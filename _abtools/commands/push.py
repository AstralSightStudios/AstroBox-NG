# -*- coding: utf-8 -*-
"""``push`` — push the root repo and every sub-repo.

Happy path runs in parallel. If a push is rejected because upstream moved on
(non-fast-forward), that repo is handed to a serial recovery pass: fetch, run
the smart conflict-resolving pull, then retry the push — no GitHub Desktop trip.
"""

import os
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import List, Optional, Tuple

from .. import conflict, ui
from ..config import RepoEntry, collect_repo_entries
from ..gitutil import (
    ensure_full_fetch_refspec,
    ensure_upstream_for_current_branch,
    get_upstream_and_ahead,
    is_git_repo,
)
from ..shell import check_git_available, run_cmd

_REJECT_HINTS = ("rejected", "non-fast-forward", "fetch first", "failed to push")


def _current_branch(path: Path) -> str:
    rc, out = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=path)
    return out.strip() if rc == 0 else "(unknown)"


def push_single_repo(entry: RepoEntry, verbose: bool) -> Tuple[int, str, bool]:
    """Returns (rc, text, needs_recovery)."""
    if not entry.path.exists():
        return 1, f"⚠️ Skipping {ui.color(entry.name, 'yellow')}: path not found -> {entry.path}", False

    if not is_git_repo(entry.path):
        return 1, (
            f"⚠️ Skipping {ui.color(entry.name, 'yellow')}: {entry.path} is not a git repository"
        ), False

    rc, status_out = run_cmd(["git", "status", "--porcelain"], cwd=entry.path)
    if rc == 0 and status_out.strip():
        msg = f"⚠️ {ui.color(entry.name, 'yellow')} has uncommitted changes. Push skipped (commit first)."
        return 1, msg, False

    if ensure_full_fetch_refspec(entry.path, verbose=verbose) != 0:
        return 1, f"❌ {ui.color(entry.name, 'red')} failed to normalize fetch refspec.", False

    branch = _current_branch(entry.path)
    upstream, ahead = get_upstream_and_ahead(entry.path)

    if ahead is not None and ahead == 0 and upstream:
        if verbose:
            return 0, (
                f"ℹ️ {ui.color(entry.name, 'gray')} already matches {ui.color(upstream, 'cyan')}."
            ), False
        return 0, "", False

    if upstream is None:
        rc, branch_name, result = ensure_upstream_for_current_branch(entry.path)
        if rc != 0:
            return 1, (
                f"❌ {ui.color(entry.name, 'red')} push failed "
                f"(branch {ui.color(branch, 'yellow')}):\n{result}"
            ), False
        return 0, (
            f"🚀 {ui.color('Pushed', 'green')} {ui.color(entry.name, 'white')} "
            f"({ui.color(branch_name or branch, 'cyan')}) -> {ui.color(result or 'origin', 'yellow')}."
        ), False

    rc, out = run_cmd(["git", "push"], cwd=entry.path)
    if rc != 0:
        if any(hint in out.lower() for hint in _REJECT_HINTS):
            return 0, "", True  # defer to serial recovery
        return 1, (
            f"❌ {ui.color(entry.name, 'red')} push failed "
            f"(branch {ui.color(branch, 'yellow')}):\n{out}"
        ), False

    suffix = f" ({ui.color(str(ahead), 'green')} commit{'s' if ahead != 1 else ''})" if ahead else ""
    return 0, (
        f"🚀 {ui.color('Pushed', 'green')} {ui.color(entry.name, 'white')} "
        f"({ui.color(branch, 'cyan')}) -> {ui.color(upstream, 'yellow')}{suffix}."
    ), False


def _recover_and_push(
    entry: RepoEntry, policy: str, merge_mode: str, auto_lockfiles: bool, verbose: bool
) -> int:
    branch = _current_branch(entry.path)
    ui.out(f"🔄 {ui.color(entry.name, 'white')} push rejected; integrating upstream then retrying ...")

    run_cmd(["git", "fetch", "origin", "--prune"], cwd=entry.path)
    outcome = conflict.smart_pull(
        entry.path,
        branch,
        op=merge_mode,
        policy=policy,
        auto_lockfiles=auto_lockfiles,
        verbose=verbose,
        interactive_ok=sys.stdin.isatty(),
    )
    if not outcome.ok:
        ui.err(f"❌ {ui.color(entry.name, 'red')} could not integrate before push: {outcome.detail}")
        return 1

    rc, out = run_cmd(["git", "push"], cwd=entry.path)
    if rc != 0:
        ui.err(f"❌ {ui.color(entry.name, 'red')} push still failed after integrate:\n{out}")
        return 1

    ui.out(f"🚀 {ui.color('Pushed', 'green')} {ui.color(entry.name, 'white')} ({ui.color(branch, 'cyan')}) after integrate.")
    return 0


def run_push(
    xml_path: Path,
    verbose: bool,
    *,
    policy: str = "interactive",
    merge_mode: str = "rebase",
    auto_lockfiles: bool = True,
) -> int:
    check_git_available()

    repo_root = xml_path.parent.resolve()
    entries: List[RepoEntry] = [
        RepoEntry(name="Root repository", path=repo_root, is_private=False)
    ]
    entries.extend(collect_repo_entries(xml_path))

    overall_rc = 0
    recover: List[RepoEntry] = []
    max_workers = min(8, max(1, (os.cpu_count() or 4)))

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = {executor.submit(push_single_repo, entry, verbose): entry for entry in entries}
        for future in as_completed(futures):
            try:
                rc, text, needs_recovery = future.result()
            except KeyboardInterrupt:
                ui.err("\nPush interrupted.")
                return 130
            except Exception as ex:
                overall_rc = 1
                ui.err(f"unexpected worker error: {ex}")
                continue
            if text:
                ui.out(text)
            if rc != 0:
                overall_rc = 1
            if needs_recovery:
                recover.append(futures[future])

    if recover:
        ui.out(ui.color(f">>> {len(recover)} repo(s) rejected; recovering serially...", "cyan"))
        for entry in recover:
            try:
                if _recover_and_push(entry, policy, merge_mode, auto_lockfiles, verbose) != 0:
                    overall_rc = 1
            except KeyboardInterrupt:
                ui.err("\nRecovery interrupted.")
                return 130

    return overall_rc
