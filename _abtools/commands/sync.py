# -*- coding: utf-8 -*-
"""``sync`` — clone/update every repo in repos.xml (plus the root).

Two phases:

1. **Parallel** — fetch all repos, clone the missing ones, and fast-forward the
   ones that are cleanly behind. This phase never touches a dirty tree and never
   needs to resolve anything, so it's safe to run concurrently.
2. **Serial** — repos that diverged, or that are behind *and* have local edits,
   go through :func:`_abtools.conflict.smart_pull` one at a time (rebase +
   autostash + rerere + lockfile auto-resolve + the chosen conflict policy).
   Serial because the interactive resolver reads from stdin.
"""

import os
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Tuple

from .. import conflict, ui
from ..config import get_repo_priv_flag, load_xml, safe_branch
from ..gitutil import (
    collect_pull_change_details,
    ensure_branch_checked_out,
    ensure_dir,
    ensure_full_fetch_refspec,
    get_head_commit,
    get_upstream_and_ahead,
    is_git_repo,
)
from ..shell import check_git_available, run_cmd


@dataclass
class SyncTarget:
    name: str
    path: Path
    url: Optional[str]  # None for the root repo (never cloned)
    branch: str
    is_private: bool


@dataclass
class IntegrateJob:
    target: SyncTarget
    old_head: Optional[str]


def _build_targets(xml_path: Path, project_root: Path) -> List[SyncTarget]:
    rc, branch_out = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=project_root)
    root_branch = branch_out.strip() if rc == 0 else "main"

    targets: List[SyncTarget] = [
        SyncTarget("Root repository", project_root, None, root_branch, False)
    ]

    xml_root = load_xml(xml_path)
    for repo in xml_root.findall("repo"):
        name = repo.get("name") or "(unnamed)"
        url = repo.get("url")
        path_attr = repo.get("path")
        if not url or not path_attr:
            ui.err(f"Skip: {name}, missing url or path.")
            continue
        target = SyncTarget(
            name=name,
            path=(project_root / path_attr).resolve(),
            url=url,
            branch=safe_branch(repo),
            is_private=get_repo_priv_flag(repo),
        )
        targets.append(target)
    return targets


def _report_change(name: str, path: Path, old_head: Optional[str]) -> List[str]:
    new_head = get_head_commit(path)
    if not (old_head and new_head and old_head != new_head):
        return []
    lines = [
        f"✅ {ui.color(name, 'white')} "
        f"{ui.color(old_head[:7], 'cyan')} -> {ui.color(new_head[:7], 'cyan')}"
    ]
    status_block, log_block = collect_pull_change_details(path, old_head, new_head)
    if status_block:
        lines.append(status_block)
    if log_block:
        lines.append(ui.color("📜 Commit log:", "gray"))
        for line in log_block.splitlines():
            lines.append(f"    {line}")
    lines.append("")
    return lines


def _clone(target: SyncTarget) -> Tuple[int, str]:
    lines = [
        f"📦 Cloning {target.name} ({target.branch}) ({target.url}) "
        f"({'private' if target.is_private else 'public'})"
    ]
    ensure_dir(target.path)
    rc, out = run_cmd(
        ["git", "clone", "--branch", target.branch, target.url, str(target.path)]
    )
    if rc != 0:
        return 1, "\n".join(lines + [f"[{target.name}] git clone failed:\n{out}"])

    if ensure_full_fetch_refspec(target.path) != 0:
        return 1, "\n".join(lines + [f"[{target.name}] clone ok but fetch refspec normalization failed."])
    run_cmd(["git", "fetch", "origin", "--prune"], cwd=target.path)
    lines.append(f"✅ [{target.name}] cloned to {target.path}.")
    return 0, "\n".join(lines)


def _prepare(
    target: SyncTarget, include_private: bool, verbose: bool, project_root: Path
) -> Tuple[int, str, Optional[IntegrateJob]]:
    """Parallel phase for one repo. May defer heavy integration to the serial phase."""
    if target.is_private and not include_private:
        return (0, f"[skip private] {target.name}" if verbose else "", None)

    if target.url and target.path == project_root:
        return (0, f"[skip duplicate] {target.name} == root" if verbose else "", None)

    # Clone when missing.
    if not target.path.exists() or not is_git_repo(target.path):
        if not target.url:
            return 1, f"[{target.name}] {target.path} is not a git repository", None
        return (*_clone(target), None)

    # Existing repo: fetch + classify.
    if ensure_full_fetch_refspec(target.path, verbose=verbose) != 0:
        return 1, f"[{target.name}] failed to normalize fetch refspec.", None

    rc, out = run_cmd(["git", "fetch", "origin", "--prune"], cwd=target.path)
    if rc != 0:
        return 1, f"[{target.name}] git fetch failed:\n{out}", None

    if target.url:  # subrepos: make sure we're on the configured branch
        rc, out = run_cmd(["git", "rev-parse", "--verify", "--quiet", target.branch], cwd=target.path)
        on_branch = rc == 0
        if not on_branch or not conflict.is_dirty(target.path):
            rc, out = ensure_branch_checked_out(target.path, target.branch)
            if rc != 0:
                return 1, f"[{target.name}] git checkout {target.branch} failed:\n{out}", None

    upstream = f"origin/{target.branch}"
    state = conflict.classify(target.path, upstream)
    dirty = conflict.is_dirty(target.path)
    old_head = get_head_commit(target.path)

    if state == "no_upstream":
        return 0, (f"😴 {ui.color(target.name, 'gray')} no upstream tracking; skipped." if verbose else ""), None

    if state == "up_to_date":
        msg = f"😴 {ui.color(target.name, 'gray')} already up to date."
        return 0, (msg if verbose else ""), None

    if state == "ahead":
        msg = f"⬆️  {ui.color(target.name, 'white')} is ahead of upstream — run push."
        return 0, msg, None

    if state == "behind" and not dirty:
        rc, out = run_cmd(["git", "merge", "--ff-only", upstream], cwd=target.path)
        if rc != 0:
            return 1, f"[{target.name}] fast-forward failed:\n{out}", None
        text = "\n".join([f"📦 {target.name} ({target.branch})"] + _report_change(target.name, target.path, old_head))
        return 0, text.rstrip(), None

    # behind+dirty or diverged -> serial smart integration
    return 0, "", IntegrateJob(target, old_head)


def _integrate(job: IntegrateJob, policy: str, merge_mode: str, auto_lockfiles: bool, verbose: bool) -> int:
    target = job.target
    ui.out(f"🔧 {ui.color('Integrating', 'magenta')} {ui.color(target.name, 'white')} "
           f"({target.branch}) — {'merge' if merge_mode == 'merge' else 'rebase'} ...")

    outcome = conflict.smart_pull(
        target.path,
        target.branch,
        op=merge_mode,
        policy=policy,
        auto_lockfiles=auto_lockfiles,
        verbose=verbose,
        interactive_ok=sys.stdin.isatty(),
    )

    if outcome.ok:
        lines = _report_change(target.name, target.path, job.old_head)
        if lines:
            ui.out("\n".join(lines).rstrip())
        else:
            ui.out(f"✅ {ui.color(target.name, 'green')} integrated (no new commits).")
        return 0

    if outcome.status == "aborted":
        ui.err(f"🛑 {ui.color(target.name, 'yellow')} {outcome.detail}")
        return 1

    if outcome.status == "unresolved":
        ui.err(f"⚠️ {ui.color(target.name, 'yellow')} {outcome.detail}")
        return 1

    ui.err(f"❌ {ui.color(target.name, 'red')} integration failed: {outcome.detail}")
    return 1


def run_sync(
    xml_path: Path,
    include_private: bool,
    verbose: bool = False,
    *,
    policy: str = "interactive",
    merge_mode: str = "rebase",
    auto_lockfiles: bool = True,
) -> int:
    check_git_available()

    project_root = xml_path.parent.resolve()
    targets = _build_targets(xml_path, project_root)

    overall_rc = 0
    jobs: List[IntegrateJob] = []

    max_workers = min(8, max(1, (os.cpu_count() or 4)))
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = [
            executor.submit(_prepare, t, include_private, verbose, project_root)
            for t in targets
        ]
        for future in as_completed(futures):
            try:
                rc, text, job = future.result()
            except KeyboardInterrupt:
                ui.err("\nOperation interrupted.")
                return 130
            except Exception as ex:
                overall_rc = 1
                ui.err(f"unexpected worker error: {ex}")
                continue
            if text:
                ui.out(text)
            if rc != 0:
                overall_rc = 1
            if job:
                jobs.append(job)

    if jobs:
        ui.out(ui.color(f">>> {len(jobs)} repo(s) need integration (local edits / diverged history)...", "cyan"))
        for job in jobs:
            try:
                if _integrate(job, policy, merge_mode, auto_lockfiles, verbose) != 0:
                    overall_rc = 1
            except KeyboardInterrupt:
                ui.err("\nIntegration interrupted.")
                return 130

    if include_private:
        try:
            (project_root / "__PRIV_CLONED").write_text("ok\n", encoding="utf-8")
        except Exception as ex:
            ui.err(f"write __PRIV_CLONED failed: {ex}")

    return overall_rc
