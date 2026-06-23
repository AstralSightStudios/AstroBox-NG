# -*- coding: utf-8 -*-
"""``commit`` — stage & commit the root repo and every sub-repo.

The old workspace reset/restore dance is gone: ``src-tauri/Cargo.toml`` is now
pinned to a static wildcard workspace (see :mod:`_abtools.config`), so it never
churns and there is nothing special to guard during a commit.
"""

from pathlib import Path
from typing import List, Optional, Tuple

from .. import editor, ui
from ..config import RepoEntry, collect_repo_entries
from ..gitutil import is_git_repo
from ..shell import check_git_available, run_cmd


def _resolve_message(provided: Optional[str], repo_name: str, status_raw: str) -> str:
    """A ``-m`` value is reused for every repo; otherwise edit one per repo."""
    if provided and provided.strip():
        return provided.strip()
    return editor.message_from_editor(repo_name, status_raw)


def run_commit(xml_path: Path, message: Optional[str], verbose: bool) -> int:
    check_git_available()

    repo_root = xml_path.parent.resolve()
    entries: List[RepoEntry] = [
        RepoEntry(name="Root repository", path=repo_root, is_private=False)
    ]
    entries.extend(collect_repo_entries(xml_path))

    ui.out(ui.color(">>> Checking repository status...", "cyan"))

    repos_with_changes: List[Tuple[RepoEntry, str]] = []
    overall_rc = 0

    for entry in entries:
        if not entry.path.exists():
            overall_rc = 1
            ui.err(f"⚠️ Skipping {ui.color(entry.name, 'yellow')}: path not found -> {entry.path}")
            continue

        if not is_git_repo(entry.path):
            overall_rc = 1
            ui.err(f"⚠️ Skipping {ui.color(entry.name, 'yellow')}: {entry.path} is not a git repository")
            continue

        rc, status_raw = run_cmd(["git", "status", "--short"], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            ui.err(f"⚠️ Unable to get status for {ui.color(entry.name, 'yellow')}:\n{status_raw}")
            continue

        if status_raw.strip():
            repos_with_changes.append((entry, status_raw))
            ui.out(f"📂 {ui.color(entry.name, 'white')} ({ui.color(str(entry.path), 'gray')})")
            ui.out(ui.format_status(status_raw))
            ui.out("")
        elif verbose:
            ui.out(f"😴 {ui.color(entry.name, 'gray')} has no pending changes.")

    if not repos_with_changes:
        ui.out(ui.color("No repositories require commits. Nothing to do.", "yellow"))
        return overall_rc

    for entry, status_raw in repos_with_changes:
        try:
            commit_message = _resolve_message(message, entry.name, status_raw)
        except editor.CommitAborted as ex:
            overall_rc = 1
            ui.err(ui.color(f"⏭️  Skipping {entry.name}: {ex}.", "yellow"))
            continue
        except KeyboardInterrupt:
            ui.err(ui.color("Commit flow cancelled.", "red"))
            return 130

        ui.out(f"📝 {ui.color('Committing', 'magenta')} {ui.color(entry.name, 'white')} ...")

        rc, out = run_cmd(["git", "add", "-A"], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            ui.err(f"❌ {ui.color(entry.name, 'red')} git add failed:\n{out}")
            continue

        rc, out = run_cmd(["git", "commit", "-m", commit_message], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            ui.err(f"❌ {ui.color(entry.name, 'red')} commit failed:\n{out}")
            continue

        ui.out(f"✅ {ui.color(entry.name, 'green')} commit finished.")

    return overall_rc
