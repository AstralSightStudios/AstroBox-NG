# -*- coding: utf-8 -*-
"""Branded, colored output for abtools.

Every line abtools prints gets a ``[✨ABTools]`` tag so its output never gets
confused with git/cargo/pnpm noise. Color is auto-disabled when stdout is not a
TTY or when ``NO_COLOR`` is set, so logs and pipes stay clean.
"""

import os
import sys

PREFIX_LABEL = "✨ABTools"

RESET = "\033[0m"
# Bright magenta for the tag — sparkly, and distinct from the colors used for
# git status / repo names below.
_TAG_ANSI = "\033[95m"

ANSI_COLORS = {
    "yellow": "\033[33m",
    "green": "\033[32m",
    "red": "\033[31m",
    "magenta": "\033[35m",
    "cyan": "\033[36m",
    "blue": "\033[34m",
    "gray": "\033[90m",
    "white": "\033[37m",
}

# git status short-code -> (color name, emoji)
STATUS_STYLES = {
    "M": ("yellow", "🛠️"),
    "A": ("green", "🆕"),
    "D": ("red", "🗑️"),
    "R": ("magenta", "🔁"),
    "C": ("cyan", "📋"),
    "?": ("blue", "❓"),
    "!": ("red", "⚠️"),
    "U": ("red", "🤝"),
}


def _color_enabled() -> bool:
    if os.environ.get("NO_COLOR") is not None:
        return False
    if os.environ.get("ABTOOLS_NO_COLOR") is not None:
        return False
    return sys.stdout.isatty()


# Resolved once at import; cheap and predictable for a short-lived CLI.
COLOR = _color_enabled()


def color(text: str, name: str) -> str:
    """Wrap ``text`` in an ANSI color (no-op when color is disabled)."""
    code = ANSI_COLORS.get(name)
    if not code or not COLOR:
        return text
    return f"{code}{text}{RESET}"


def _tag() -> str:
    if COLOR:
        return f"{_TAG_ANSI}[{PREFIX_LABEL}]{RESET}"
    return f"[{PREFIX_LABEL}]"


def _emit(stream, parts, sep: str) -> None:
    text = sep.join(str(p) for p in parts)
    tag = _tag()
    rendered = []
    for line in text.split("\n"):
        # Preserve blank lines as real blanks so spacing still reads nicely.
        rendered.append(f"{tag} {line}" if line else "")
    print("\n".join(rendered), file=stream)


def out(*parts, sep: str = " ") -> None:
    """Print a (possibly multi-line) message to stdout, tagged per line."""
    _emit(sys.stdout, parts, sep)


def err(*parts, sep: str = " ") -> None:
    """Print a (possibly multi-line) message to stderr, tagged per line."""
    _emit(sys.stderr, parts, sep)


def _colorize_status_line(line: str) -> str:
    if not line.strip():
        return line

    status_part = line[:2]
    remainder = line[3:] if len(line) > 3 else ""

    colored_status_chars = []
    emoji = None
    for ch in status_part:
        if ch == " ":
            colored_status_chars.append(" ")
            continue
        name, candidate_emoji = STATUS_STYLES.get(ch, ("gray", "📄"))
        colored_status_chars.append(color(ch, name))
        if emoji is None and candidate_emoji:
            emoji = candidate_emoji

    if emoji is None:
        emoji = "📄"

    colored_status = "".join(colored_status_chars)
    return f"{emoji} {colored_status} {remainder}".rstrip()


def format_status(raw: str) -> str:
    """Colorize a ``git status --short`` (or diff-derived) block."""
    lines = [_colorize_status_line(line) for line in raw.rstrip().splitlines()]
    return "\n".join(lines)
