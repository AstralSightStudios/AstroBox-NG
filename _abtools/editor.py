# -*- coding: utf-8 -*-
"""IME-friendly commit message entry.

Typing a commit message inline with ``input()`` is hostile to Chinese IMEs:
pressing Enter/Space to pick a candidate often submits the line early. So we do
what ``git commit`` does — open ``$EDITOR`` on a temp file. The user composes in
a real editor (full IME support, multi-line), saves, and we read it back.
"""

import os
import shlex
import shutil
import subprocess
import tempfile
from typing import Optional

from . import ui


class CommitAborted(Exception):
    """Raised when the user supplies no usable commit message."""


def _pick_editor() -> Optional[str]:
    for var in ("GIT_EDITOR", "VISUAL", "EDITOR"):
        value = os.environ.get(var)
        if value and value.strip():
            return value.strip()
    for candidate in ("nano", "vim", "vi"):
        if shutil.which(candidate):
            return candidate
    return None


def _strip_comments(raw: str) -> str:
    kept = [ln for ln in raw.splitlines() if not ln.lstrip().startswith("#")]
    return "\n".join(kept).strip()


def _build_template(repo_name: str, status_raw: Optional[str]) -> str:
    lines = [
        "",
        "",
        f"# Please enter the commit message for: {repo_name}",
        "# Lines starting with '#' are ignored; an empty message aborts this repo's commit.",
        "#",
    ]
    if status_raw and status_raw.strip():
        lines.append("# Changes to be committed:")
        for line in status_raw.rstrip().splitlines():
            lines.append(f"#   {line}")
    return "\n".join(lines) + "\n"


def _inline_fallback(repo_name: str) -> str:
    """Last resort when no editor exists at all."""
    ui.err(ui.color("No $EDITOR/VISUAL and no nano/vim/vi found; falling back to inline input.", "yellow"))
    prompt = f"[✨ABTools] Commit message [{repo_name}]: "
    while True:
        try:
            msg = input(prompt)
        except EOFError:
            msg = ""
        msg = msg.strip()
        if msg:
            return msg
        ui.out("Commit message must not be empty. Please try again.")


def open_file(path) -> bool:
    """Open an existing file in the user's editor (used for manual conflict edits)."""
    editor = _pick_editor()
    if not editor:
        ui.err("No $EDITOR/VISUAL and no nano/vim/vi found; cannot open the file.")
        return False
    try:
        rc = subprocess.call(shlex.split(editor) + [str(path)])
    except FileNotFoundError:
        ui.err(f"Editor not found: {editor}")
        return False
    return rc == 0


def message_from_editor(repo_name: str, status_raw: Optional[str] = None) -> str:
    """Open the user's editor and return the composed (non-empty) message."""
    editor = _pick_editor()
    if not editor:
        return _inline_fallback(repo_name)

    fd, path = tempfile.mkstemp(prefix="ABTOOLS_COMMIT_", suffix=".txt")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(_build_template(repo_name, status_raw))

        ui.out(f"Opening {ui.color(editor, 'cyan')} to compose the commit message for "
               f"{ui.color(repo_name, 'white')} ...")
        cmd = shlex.split(editor) + [path]
        try:
            rc = subprocess.call(cmd)
        except FileNotFoundError:
            return _inline_fallback(repo_name)
        if rc != 0:
            raise CommitAborted(f"editor exited with status {rc}")

        with open(path, encoding="utf-8") as handle:
            raw = handle.read()
    finally:
        try:
            os.unlink(path)
        except OSError:
            pass

    message = _strip_comments(raw)
    if not message:
        raise CommitAborted("empty commit message")
    return message
