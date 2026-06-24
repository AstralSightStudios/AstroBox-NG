# -*- coding: utf-8 -*-
"""``profile`` — manage and apply shared dev configurations (repo → branch).

A profile is a sparse map of repo → branch, committed in ``abprofiles.json`` and
shared by everyone. Applying a profile switches every repo to its profile branch
(or its default branch when not listed), records it as locally-active, and syncs.
Clearing reverts every repo to its default branch and de-activates the profile.
"""

from pathlib import Path
from typing import List, Optional, Tuple

from .. import profiles, ui
from ..config import WorkspaceRepo, list_workspace
from ..gitutil import (
    checkout_or_create_branch,
    current_branch,
    default_branch,
    ensure_full_fetch_refspec,
    is_git_repo,
)
from ..shell import check_git_available, run_cmd
from .sync import run_sync


def _targets(xml_path: Path) -> Tuple[Path, List[WorkspaceRepo]]:
    root = xml_path.parent.resolve()
    # The root's "default" must be its canonical branch (origin HEAD), not the
    # branch currently checked out — otherwise clear/apply can't revert it.
    return root, list_workspace(xml_path, root, default_branch(root))


def _present(target: WorkspaceRepo) -> bool:
    return target.path.exists() and is_git_repo(target.path)


def _include_private(root: Path) -> bool:
    return (root / "__PRIV_CLONED").exists()


# --------------------------------------------------------------------------- #
# Read-only views
# --------------------------------------------------------------------------- #

def list_profiles(xml_path: Path) -> int:
    root = xml_path.parent.resolve()
    catalog = profiles.load_all(root)
    active = profiles.get_active(root)
    if not catalog:
        ui.out("No dev profiles yet. Create one with: python abtools.py profile save <name>")
        return 0
    ui.out(ui.color("Dev profiles:", "cyan"))
    for name in sorted(catalog):
        marker = ui.color(" ● active", "green") if name == active else ""
        ui.out(f"  {ui.color(name, 'white')}  ({len(catalog[name])} override(s)){marker}")
    return 0


def show_profile(xml_path: Path, name: str) -> int:
    root = xml_path.parent.resolve()
    mapping = profiles.get(root, name)
    if mapping is None:
        ui.err(f"Profile not found: {name}")
        return 1
    ui.out(ui.color(f"Profile '{name}':", "cyan"))
    if not mapping:
        ui.out("  (no overrides — every repo on its default branch)")
    for key in sorted(mapping):
        ui.out(f"  {ui.color(key, 'white')} → {ui.color(mapping[key], 'cyan')}")
    return 0


def current_profile(xml_path: Path) -> int:
    root, targets = _targets(xml_path)
    active = profiles.get_active(root)
    overrides = profiles.active_overrides(root)
    ui.out(f"Active profile: {ui.color(active or '— (defaults)', 'cyan')}")
    for t in targets:
        if not _present(t):
            continue
        want = overrides.get(t.key, t.branch)
        cur = current_branch(t.path)
        mark = ui.color("✓", "green") if cur == want else ui.color("✗", "yellow")
        line = f"  {mark} {t.name}: on {ui.color(cur, 'cyan')}"
        if cur != want:
            line += f" (want {ui.color(want, 'yellow')})"
        ui.out(line)
    return 0


# --------------------------------------------------------------------------- #
# Mutations
# --------------------------------------------------------------------------- #

def save_profile(xml_path: Path, name: str, sets: List[str], from_current: bool) -> int:
    root, targets = _targets(xml_path)
    mapping = {}

    if from_current:
        for t in targets:
            if not _present(t):
                continue
            cur = current_branch(t.path)
            if cur and cur not in ("(unknown)",) and cur != t.branch:
                mapping[t.key] = cur

    for pair in sets:
        if "=" not in pair:
            ui.err(f"Ignoring --set '{pair}' (expected REPO=BRANCH).")
            continue
        key, branch = pair.split("=", 1)
        mapping[key.strip()] = branch.strip()

    profiles.upsert(root, name, mapping)
    ui.out(f"💾 Saved profile {ui.color(name, 'white')} with {len(mapping)} override(s).")
    if mapping:
        for key in sorted(mapping):
            ui.out(f"    {key} → {mapping[key]}")
    ui.out(f"   (commit {ui.color('abprofiles.json', 'cyan')} to share it.)")
    return 0


def delete_profile(xml_path: Path, name: str) -> int:
    root = xml_path.parent.resolve()
    if profiles.delete(root, name):
        ui.out(f"🗑️  Deleted profile {ui.color(name, 'white')}.")
        return 0
    ui.err(f"Profile not found: {name}")
    return 1


def _switch_all(targets: List[WorkspaceRepo], branch_for, verbose: bool) -> int:
    """Checkout (creating from default if needed) the wanted branch in every repo."""
    rc_overall = 0
    for t in targets:
        if not _present(t):
            if verbose:
                ui.out(f"  [skip] {t.name} not present")
            continue
        want = branch_for(t)
        ensure_full_fetch_refspec(t.path, verbose=verbose)
        run_cmd(["git", "fetch", "origin", "--prune"], cwd=t.path)
        if current_branch(t.path) == want:
            continue
        rc, out = checkout_or_create_branch(t.path, want, base=t.branch)
        if rc != 0:
            rc_overall = 1
            ui.err(f"  ❌ {t.name}: cannot switch to {want}: {out.strip()}")
        else:
            ui.out(f"  ↪ {t.name} → {ui.color(want, 'cyan')}")
    return rc_overall


def apply_profile(
    xml_path: Path,
    name: str,
    *,
    do_sync: bool,
    policy: str,
    merge_mode: str,
    auto_lockfiles: bool,
    verbose: bool,
) -> int:
    check_git_available()
    root, targets = _targets(xml_path)
    mapping = profiles.get(root, name)
    if mapping is None:
        ui.err(f"Profile not found: {name}")
        return 1

    ui.out(ui.color(f">>> Applying profile '{name}' ...", "cyan"))
    rc = _switch_all(targets, lambda t: mapping.get(t.key, t.branch), verbose)
    profiles.set_active(root, name)
    ui.out(f"✅ Profile {ui.color(name, 'white')} is now active.")

    if do_sync:
        rc = run_sync(
            xml_path,
            include_private=_include_private(root),
            verbose=verbose,
            policy=policy,
            merge_mode=merge_mode,
            auto_lockfiles=auto_lockfiles,
        ) or rc
    return rc


def clear_profile(
    xml_path: Path,
    *,
    do_sync: bool,
    policy: str,
    merge_mode: str,
    auto_lockfiles: bool,
    verbose: bool,
) -> int:
    check_git_available()
    root, targets = _targets(xml_path)
    ui.out(ui.color(">>> Clearing active profile — back to default branches ...", "cyan"))
    rc = _switch_all(targets, lambda t: t.branch, verbose)
    profiles.set_active(root, None)
    ui.out("✅ No profile active; repos on their default branches.")

    if do_sync:
        rc = run_sync(
            xml_path,
            include_private=_include_private(root),
            verbose=verbose,
            policy=policy,
            merge_mode=merge_mode,
            auto_lockfiles=auto_lockfiles,
        ) or rc
    return rc
