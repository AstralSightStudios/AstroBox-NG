# -*- coding: utf-8 -*-
"""``branch`` — create/list branches across the workspace.

- ``branch new <name> --repo <key> [--base <branch>]`` — new branch in one repo.
- ``branch new <name> --all [--base main]`` — new branch in every repo from a
  common base; also auto-saves a same-named dev profile and activates it, so the
  whole feature branch becomes an applyable, shareable config in one step.

New branches are created locally only; share them later with ``push``
(or pass ``--push``).
"""

from pathlib import Path
from typing import List, Optional, Tuple

from .. import profiles, ui
from ..config import WorkspaceRepo, list_workspace
from ..gitutil import (
    checkout_or_create_branch,
    current_branch,
    ensure_full_fetch_refspec,
    is_git_repo,
)
from ..shell import check_git_available, run_cmd


def _targets(xml_path: Path) -> Tuple[Path, List[WorkspaceRepo]]:
    root = xml_path.parent.resolve()
    rc, out = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=root)
    root_branch = out.strip() if rc == 0 else "main"
    return root, list_workspace(xml_path, root, root_branch)


def _present(t: WorkspaceRepo) -> bool:
    return t.path.exists() and is_git_repo(t.path)


def _find_repo(targets: List[WorkspaceRepo], ident: str) -> Optional[WorkspaceRepo]:
    for t in targets:
        if ident in (t.key, t.path.name) or ident == t.name:
            return t
    for t in targets:  # fuzzy fallback
        if ident.lower() in t.name.lower():
            return t
    return None


def _create_one(t: WorkspaceRepo, name: str, base: str, push: bool, verbose: bool) -> Tuple[int, bool]:
    if not _present(t):
        if verbose:
            ui.out(f"  [skip] {t.name} not present")
        return 0, False
    ensure_full_fetch_refspec(t.path, verbose=verbose)
    run_cmd(["git", "fetch", "origin", "--prune"], cwd=t.path)
    rc, out = checkout_or_create_branch(t.path, name, base=base)
    if rc != 0:
        ui.err(f"  ❌ {t.name}: {out.strip()}")
        return 1, False
    ui.out(f"  🌱 {t.name} → {ui.color(name, 'cyan')} (from {base})")
    if push:
        prc, pout = run_cmd(["git", "push", "-u", "origin", name], cwd=t.path)
        if prc != 0:
            ui.err(f"  ⚠️ {t.name}: created but push failed:\n{pout}")
            return 1, True
        ui.out(f"     🚀 pushed {name} → origin")
    return 0, True


def branch_new(
    xml_path: Path,
    name: str,
    *,
    all_repos: bool,
    repo: Optional[str],
    base: Optional[str],
    push: bool,
    verbose: bool,
) -> int:
    check_git_available()
    root, targets = _targets(xml_path)

    if all_repos:
        base = base or "main"
        ui.out(ui.color(f">>> Creating '{name}' across all repos (from {base}) ...", "cyan"))
        created: List[str] = []
        overall = 0
        for t in targets:
            rc, ok = _create_one(t, name, base, push, verbose)
            overall = overall or rc
            if ok:
                created.append(t.key)
        if created:
            profiles.upsert(root, name, {k: name for k in created})
            profiles.set_active(root, name)
            ui.out(
                f"💾 Saved & activated profile {ui.color(name, 'white')} "
                f"({len(created)} repos). Commit {ui.color('abprofiles.json', 'cyan')} to share."
            )
        return overall

    # single repo
    if not repo:
        ui.err("Specify --repo <key> for a single repo, or --all for every repo.")
        return 2
    target = _find_repo(targets, repo)
    if not target:
        ui.err(f"No repo matches '{repo}'. Known keys: " + ", ".join(t.key for t in targets))
        return 2
    base = base or target.branch
    ui.out(ui.color(f">>> Creating '{name}' in {target.name} (from {base}) ...", "cyan"))
    rc, _ = _create_one(target, name, base, push, verbose)
    return rc


def branch_list(xml_path: Path) -> int:
    _, targets = _targets(xml_path)
    ui.out(ui.color("Current branches:", "cyan"))
    for t in targets:
        if not _present(t):
            ui.out(f"  {ui.color(t.name, 'gray')}: (not present)")
            continue
        cur = current_branch(t.path)
        same = cur == t.branch
        suffix = "" if same else ui.color(f"  (default {t.branch})", "gray")
        ui.out(f"  {ui.color(t.name, 'white')}: {ui.color(cur, 'cyan')}{suffix}")
    return 0
