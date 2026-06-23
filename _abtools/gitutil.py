# -*- coding: utf-8 -*-
"""Small git query/format helpers built on top of :func:`shell.run_cmd`."""

from pathlib import Path
from typing import List, Optional, Tuple

from . import ui
from .shell import run_cmd


def is_git_repo(path: Path) -> bool:
    return (path / ".git").is_dir()


def ensure_dir(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def get_head_commit(repo_path: Path) -> Optional[str]:
    rc, out = run_cmd(["git", "rev-parse", "HEAD"], cwd=repo_path)
    if rc != 0:
        return None
    return out.strip() or None


def get_upstream_and_ahead(repo_path: Path) -> Tuple[Optional[str], Optional[int]]:
    rc, upstream_out = run_cmd(
        ["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        cwd=repo_path,
    )
    if rc != 0:
        return None, None

    upstream = upstream_out.strip()
    if not upstream:
        return None, None

    rc, ahead_out = run_cmd(["git", "rev-list", "--count", "@{u}..HEAD"], cwd=repo_path)
    if rc != 0:
        return upstream, None

    try:
        ahead = int(ahead_out.strip())
    except ValueError:
        ahead = None

    return upstream, ahead


def ensure_full_fetch_refspec(repo_path: Path, verbose: bool = False) -> int:
    desired = "+refs/heads/*:refs/remotes/origin/*"

    rc, out = run_cmd(
        ["git", "config", "--get-all", "remote.origin.fetch"],
        cwd=repo_path,
    )
    existing = [line.strip() for line in out.splitlines() if line.strip()] if rc == 0 else []

    if existing == [desired]:
        return 0

    rc, out = run_cmd(
        ["git", "config", "--unset-all", "remote.origin.fetch"],
        cwd=repo_path,
    )
    if rc != 0 and "No such section or key" not in out:
        if verbose:
            ui.err(f"[fetch-config] unset remote.origin.fetch failed in {repo_path}:\n{out}")

    rc, out = run_cmd(
        ["git", "config", "--add", "remote.origin.fetch", desired],
        cwd=repo_path,
    )
    if rc != 0:
        ui.err(f"[fetch-config] set remote.origin.fetch failed in {repo_path}:\n{out}")
        return 1

    if verbose:
        ui.out(f"[fetch-config] normalized remote.origin.fetch in {repo_path}")

    return 0


def ensure_branch_checked_out(repo_path: Path, branch: str) -> Tuple[int, str]:
    rc, out = run_cmd(["git", "rev-parse", "--verify", branch], cwd=repo_path)
    if rc == 0:
        return run_cmd(["git", "checkout", branch], cwd=repo_path)

    rc, out = run_cmd(
        ["git", "show-ref", "--verify", f"refs/remotes/origin/{branch}"],
        cwd=repo_path,
    )
    if rc == 0:
        return run_cmd(
            ["git", "checkout", "-B", branch, "--track", f"origin/{branch}"],
            cwd=repo_path,
        )

    return 1, f"Remote branch origin/{branch} not found."


def current_branch(repo_path: Path) -> str:
    rc, out = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=repo_path)
    return out.strip() if rc == 0 else "(unknown)"


def local_branch_exists(repo_path: Path, name: str) -> bool:
    rc, _ = run_cmd(["git", "rev-parse", "--verify", "--quiet", f"refs/heads/{name}"], cwd=repo_path)
    return rc == 0


def remote_branch_exists(repo_path: Path, name: str) -> bool:
    rc, _ = run_cmd(
        ["git", "show-ref", "--verify", "--quiet", f"refs/remotes/origin/{name}"], cwd=repo_path
    )
    return rc == 0


def checkout_or_create_branch(repo_path: Path, name: str, base: str) -> Tuple[int, str]:
    """Switch to ``name``, creating it from ``base`` (or origin/base) if needed.

    Resolution order: existing local branch → existing origin branch (tracked) →
    create new from local ``base`` → create new from ``origin/base``.
    """
    if local_branch_exists(repo_path, name):
        return run_cmd(["git", "checkout", name], cwd=repo_path)

    if remote_branch_exists(repo_path, name):
        return run_cmd(
            ["git", "checkout", "-B", name, "--track", f"origin/{name}"], cwd=repo_path
        )

    if local_branch_exists(repo_path, base):
        base_ref = base
    elif remote_branch_exists(repo_path, base):
        base_ref = f"origin/{base}"
    else:
        return 1, f"base branch '{base}' not found (locally or on origin)."

    return run_cmd(["git", "checkout", "-b", name, base_ref], cwd=repo_path)


def ensure_upstream_for_current_branch(
    repo_path: Path,
) -> Tuple[int, Optional[str], Optional[str]]:
    rc, branch_out = run_cmd(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=repo_path,
    )
    if rc != 0:
        return 1, None, branch_out

    branch = branch_out.strip()
    if not branch:
        return 1, None, "Cannot determine current branch."

    upstream, _ = get_upstream_and_ahead(repo_path)
    if upstream:
        return 0, branch, upstream

    rc, out = run_cmd(["git", "push", "-u", "origin", branch], cwd=repo_path)
    if rc != 0:
        return rc, branch, out

    return 0, branch, f"origin/{branch}"


def _diff_to_status_lines(diff_out: str) -> List[str]:
    lines: List[str] = []
    for raw in diff_out.strip().splitlines():
        raw = raw.strip()
        if not raw:
            continue
        parts = raw.split("\t")
        if not parts:
            continue

        status_token = parts[0]
        primary = status_token[0] if status_token else "?"

        if primary in ("R", "C") and len(parts) >= 3:
            desc = f"{parts[1]} -> {parts[2]}"
        elif len(parts) >= 2:
            desc = parts[1]
        else:
            desc = raw.replace("\t", " ")

        lines.append(f"{primary}  {desc}")

    return lines


def collect_pull_change_details(
    repo_path: Path, old_head: str, new_head: str
) -> Tuple[Optional[str], Optional[str]]:
    status_block: Optional[str] = None
    log_block: Optional[str] = None

    rc, diff_out = run_cmd(
        ["git", "diff", "--name-status", f"{old_head}..{new_head}"],
        cwd=repo_path,
    )
    if rc == 0 and diff_out.strip():
        status_lines = _diff_to_status_lines(diff_out)
        if status_lines:
            status_block = ui.format_status("\n".join(status_lines))

    rc, log_out = run_cmd(
        ["git", "log", "--oneline", f"{old_head}..{new_head}"],
        cwd=repo_path,
    )
    if rc == 0 and log_out.strip():
        log_block = log_out.strip()

    return status_block, log_block
