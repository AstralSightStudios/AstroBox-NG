# -*- coding: utf-8 -*-
"""Smart pull / conflict-resolution engine shared by ``sync`` and ``push``.

The goal: integrate upstream changes without ever bouncing to GitHub Desktop,
while never silently throwing away work. Layers, from safest to most involved:

1. ``rerere`` — git records your manual conflict resolutions and replays them
   automatically next time. Always enabled.
2. ``--autostash`` — uncommitted edits are stashed before the rebase/merge and
   re-applied after. No "your local changes would be overwritten" walls.
3. Clean 3-way auto-merge of everything that doesn't actually conflict.
4. Lockfiles (Cargo.lock / pnpm-lock.yaml / …) auto-resolved to the upstream
   copy — they regenerate on the next build, so picking a side by hand is noise.
5. Whatever is *still* conflicting goes to the chosen policy: an interactive
   per-file resolver (default), bulk "keep mine"/"take upstream", or abort.

Rebase note: during a rebase the meaning of ``--ours``/``--theirs`` is inverted
(``ours`` is the branch you replay onto = upstream). :func:`_checkout_flags`
centralizes that so the rest of the code can think in plain local/remote terms.
"""

from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Tuple

from . import editor, ui
from .shell import run_cmd

# Generated artifacts: a hand-merge is pointless, the next build rewrites them.
LOCKFILES = {"Cargo.lock", "pnpm-lock.yaml", "package-lock.json", "yarn.lock"}

# Stop git from popping an editor mid-rebase/merge (commit messages are reused).
_NONINTERACTIVE_GIT_ENV = {"GIT_EDITOR": "true", "GIT_SEQUENCE_EDITOR": "true"}

_TAG = f"[{ui.PREFIX_LABEL}]"


@dataclass
class Outcome:
    status: str  # up_to_date | fast_forwarded | rebased | merged | aborted | unresolved | error
    detail: str = ""

    @property
    def ok(self) -> bool:
        return self.status in ("up_to_date", "fast_forwarded", "rebased", "merged")


# --------------------------------------------------------------------------- #
# Repo state helpers
# --------------------------------------------------------------------------- #

def _rev(repo: Path, ref: str) -> Optional[str]:
    rc, out = run_cmd(["git", "rev-parse", "--verify", "--quiet", ref], cwd=repo)
    return out.strip() if rc == 0 and out.strip() else None


def _merge_base(repo: Path, a: str, b: str) -> Optional[str]:
    rc, out = run_cmd(["git", "merge-base", a, b], cwd=repo)
    return out.strip() if rc == 0 else None


def is_dirty(repo: Path) -> bool:
    rc, out = run_cmd(["git", "status", "--porcelain"], cwd=repo)
    return bool(out.strip()) if rc == 0 else False


def classify(repo: Path, upstream: str) -> str:
    """Relationship of HEAD to ``upstream``: up_to_date / behind / ahead / diverged / no_upstream."""
    head = _rev(repo, "HEAD")
    up = _rev(repo, upstream)
    if not up:
        return "no_upstream"
    if head == up:
        return "up_to_date"
    base = _merge_base(repo, "HEAD", upstream)
    if base == head:
        return "behind"
    if base == up:
        return "ahead"
    return "diverged"


def _unmerged(repo: Path) -> List[str]:
    rc, out = run_cmd(["git", "diff", "--name-only", "--diff-filter=U"], cwd=repo)
    if rc != 0:
        return []
    return [ln.strip() for ln in out.splitlines() if ln.strip()]


def _rebase_in_progress(repo: Path) -> bool:
    return (repo / ".git" / "rebase-merge").exists() or (repo / ".git" / "rebase-apply").exists()


def _merge_in_progress(repo: Path) -> bool:
    return (repo / ".git" / "MERGE_HEAD").exists()


def _has_markers(repo: Path, rel: str) -> bool:
    try:
        text = (repo / rel).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return False
    return "<<<<<<<" in text and ">>>>>>>" in text


# --------------------------------------------------------------------------- #
# ours/theirs mapping (handles the rebase inversion)
# --------------------------------------------------------------------------- #

def _checkout_flags(op: str) -> Tuple[str, str]:
    """Return (local_flag, remote_flag) for ``git checkout`` during ``op``."""
    if op == "rebase":
        return "--theirs", "--ours"
    return "--ours", "--theirs"


def _x_option(op: str, policy: str) -> Optional[str]:
    if policy == "local":
        return "-Xtheirs" if op == "rebase" else "-Xours"
    if policy == "remote":
        return "-Xours" if op == "rebase" else "-Xtheirs"
    return None


def _take(repo: Path, rel: str, flag: str) -> None:
    run_cmd(["git", "checkout", flag, "--", rel], cwd=repo)
    run_cmd(["git", "add", "--", rel], cwd=repo)


# --------------------------------------------------------------------------- #
# Resolution rounds
# --------------------------------------------------------------------------- #

def _auto_resolve_lockfiles(repo: Path, op: str, verbose: bool) -> None:
    _, remote_flag = _checkout_flags(op)
    for rel in _unmerged(repo):
        if Path(rel).name in LOCKFILES:
            _take(repo, rel, remote_flag)
            ui.out(f"   🔒 auto-resolved {ui.color(rel, 'cyan')} → upstream (regenerate on next build)")


def _bulk_resolve(repo: Path, op: str, policy: str) -> None:
    local_flag, remote_flag = _checkout_flags(op)
    flag = local_flag if policy == "local" else remote_flag
    for rel in _unmerged(repo):
        _take(repo, rel, flag)


def _resolve_interactive(repo: Path, op: str) -> str:
    """Walk the user through each conflicted file. Returns resolved | leave | abort."""
    local_flag, remote_flag = _checkout_flags(op)

    while True:
        files = _unmerged(repo)
        if not files:
            return "resolved"

        ui.out(ui.color(f"⚔️  {len(files)} conflicted file(s):", "yellow"))
        for rel in files:
            ui.out(f"     {rel}")

        for rel in files:
            while True:
                try:
                    choice = input(
                        f"{_TAG} {rel}  →  [l]ocal  [r]emote  [e]dit  [d]iff  [s]kip  [A]bort > "
                    ).strip().lower()
                except EOFError:
                    return "leave"
                except KeyboardInterrupt:
                    return "abort"

                if choice in ("l", "local"):
                    _take(repo, rel, local_flag)
                    break
                if choice in ("r", "remote"):
                    _take(repo, rel, remote_flag)
                    break
                if choice in ("e", "edit"):
                    if editor.open_file(repo / rel):
                        if _has_markers(repo, rel):
                            ui.err("   Still contains conflict markers; not staged.")
                            continue
                        run_cmd(["git", "add", "--", rel], cwd=repo)
                    break
                if choice in ("d", "diff"):
                    _, diff = run_cmd(["git", "diff", "--", rel], cwd=repo)
                    ui.out(diff.rstrip() or "(no diff)")
                    continue
                if choice in ("s", "skip"):
                    break
                if choice in ("a", "abort"):
                    return "abort"
                ui.out("   Please choose l / r / e / d / s / A.")

        remaining = _unmerged(repo)
        if not remaining:
            return "resolved"

        try:
            cont = input(
                f"{_TAG} {len(remaining)} file(s) still conflicted — "
                f"[c]ontinue resolving  [k]eep as-is & stop  [A]bort > "
            ).strip().lower()
        except (EOFError, KeyboardInterrupt):
            return "leave"
        if cont in ("k", "keep"):
            return "leave"
        if cont in ("a", "abort"):
            return "abort"


def _abort(repo: Path) -> None:
    if _rebase_in_progress(repo):
        run_cmd(["git", "rebase", "--abort"], cwd=repo)
    elif _merge_in_progress(repo):
        run_cmd(["git", "merge", "--abort"], cwd=repo)


def _conflict_hint(repo: Path) -> str:
    files = _unmerged(repo)
    listing = "\n".join(f"     {f}" for f in files)
    finish = "git rebase --continue" if _rebase_in_progress(repo) else "git commit"
    return (
        "left in a conflicted state; finish manually:\n"
        f"{listing}\n"
        f"   (resolve the files, `git add`, then `{finish}`)"
    )


# --------------------------------------------------------------------------- #
# Entry point
# --------------------------------------------------------------------------- #

def smart_pull(
    repo_path: Path,
    branch: str,
    *,
    op: str = "rebase",
    policy: str = "interactive",
    auto_lockfiles: bool = True,
    verbose: bool = False,
    interactive_ok: bool = True,
) -> Outcome:
    """Integrate ``origin/<branch>`` into the current branch, resolving conflicts."""
    upstream = f"origin/{branch}"
    run_cmd(["git", "config", "rerere.enabled", "true"], cwd=repo_path)

    if policy == "interactive" and not interactive_ok:
        # No TTY to prompt on — fall back to the safe option.
        policy = "abort"

    cmd = ["git", "-c", "rerere.enabled=true", op, "--autostash"]
    strat = _x_option(op, policy)
    if strat:
        cmd.append(strat)
    cmd.append(upstream)

    rc, out = run_cmd(cmd, cwd=repo_path, env=_NONINTERACTIVE_GIT_ENV)

    guard = 0
    while True:
        guard += 1
        if guard > 200:
            return Outcome("error", "resolution loop exceeded; please finish manually.")

        unmerged = _unmerged(repo_path)
        if rc == 0 and not unmerged:
            break
        if not unmerged:
            # Non-conflict failure (bad branch, stash failure, etc.)
            return Outcome("error", out.strip())

        in_rebase = _rebase_in_progress(repo_path)
        in_merge = _merge_in_progress(repo_path)

        if auto_lockfiles:
            _auto_resolve_lockfiles(repo_path, op, verbose)
            unmerged = _unmerged(repo_path)

        if unmerged:
            if policy == "abort":
                _abort(repo_path)
                return Outcome("aborted", f"{len(unmerged)} conflicted file(s); aborted, repo unchanged.")
            if policy in ("local", "remote"):
                _bulk_resolve(repo_path, op, policy)
            else:
                result = _resolve_interactive(repo_path, op)
                if result == "abort":
                    _abort(repo_path)
                    return Outcome("aborted", "aborted; repo restored to pre-pull state.")
                if result == "leave":
                    return Outcome("unresolved", _conflict_hint(repo_path))

        if in_rebase:
            rc, out = run_cmd(["git", "rebase", "--continue"], cwd=repo_path, env=_NONINTERACTIVE_GIT_ENV)
        elif in_merge:
            rc, out = run_cmd(["git", "commit", "--no-edit"], cwd=repo_path, env=_NONINTERACTIVE_GIT_ENV)
        else:
            # autostash re-apply conflicts: files resolved in place, nothing to continue.
            rc = 0

    return Outcome("rebased" if op == "rebase" else "merged")
