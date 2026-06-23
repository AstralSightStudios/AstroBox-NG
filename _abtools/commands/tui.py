# -*- coding: utf-8 -*-
"""``tui`` — a tiny curses dashboard + action launcher.

Uses only the standard-library ``curses`` module (no new dependencies). The
dashboard shows the live git status of every repo; the action menu runs the
existing commands. Running an action suspends curses, lets the command use the
real terminal (so ``$EDITOR`` for commits and interactive ``pnpm tauri:dev``
work normally), then restores the dashboard.

``curses`` isn't available on stock Windows Python; in that case the command
degrades gracefully with a clear message instead of crashing.
"""

import os
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, List, Optional

from .. import ui
from ..config import RepoEntry, collect_repo_entries
from ..gitutil import get_upstream_and_ahead, is_git_repo
from ..shell import check_git_available, run_cmd

from .build import run_build
from .commit import run_commit
from .dev import run_dev
from .init import run_init
from .push import run_push
from .sync import run_sync


# --------------------------------------------------------------------------- #
# Status scanning
# --------------------------------------------------------------------------- #

@dataclass
class RepoStatus:
    name: str
    exists: bool
    is_repo: bool
    branch: str
    changes: int
    ahead: int
    private: bool


def _scan_one(entry: RepoEntry) -> RepoStatus:
    if not entry.path.exists():
        return RepoStatus(entry.name, False, False, "", 0, 0, entry.is_private)
    if not is_git_repo(entry.path):
        return RepoStatus(entry.name, True, False, "", 0, 0, entry.is_private)

    rc, branch_out = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=entry.path)
    branch = branch_out.strip() if rc == 0 else "?"

    rc, status_out = run_cmd(["git", "status", "--porcelain"], cwd=entry.path)
    changes = len([ln for ln in status_out.splitlines() if ln.strip()]) if rc == 0 else 0

    _, ahead = get_upstream_and_ahead(entry.path)

    return RepoStatus(entry.name, True, True, branch, changes, ahead or 0, entry.is_private)


def _scan(xml_path: Path) -> List[RepoStatus]:
    entries: List[RepoEntry] = [
        RepoEntry(name="Root repository", path=xml_path.parent.resolve(), is_private=False)
    ]
    entries.extend(collect_repo_entries(xml_path))
    workers = min(8, max(1, (os.cpu_count() or 4)))
    with ThreadPoolExecutor(max_workers=workers) as executor:
        return list(executor.map(_scan_one, entries))


# --------------------------------------------------------------------------- #
# Actions
# --------------------------------------------------------------------------- #

@dataclass
class Action:
    key: str
    label: str
    run: Callable[[], int]


def _ask_yes_no(prompt: str, default: bool = False) -> bool:
    suffix = " [Y/n] " if default else " [y/N] "
    try:
        answer = input(prompt + suffix).strip().lower()
    except (EOFError, KeyboardInterrupt):
        return default
    if not answer:
        return default
    return answer in ("y", "yes")


def _build_actions(xml_path: Path, verbose: bool) -> List[Action]:
    def _init() -> int:
        private = _ask_yes_no("[✨ABTools] Include private repos?", default=True)
        return run_init(xml_path, include_private=private, verbose=verbose)

    def _build() -> int:
        try:
            target = input("[✨ABTools] Build target (android/ios/wasm): ").strip()
        except (EOFError, KeyboardInterrupt):
            return 0
        if not target:
            ui.out("No target given; build skipped.")
            return 0
        return run_build(target, [])

    return [
        Action("s", "Sync (public)", lambda: run_sync(xml_path, False, verbose)),
        Action("S", "Sync (incl. private)", lambda: run_sync(xml_path, True, verbose)),
        Action("c", "Commit  (opens $EDITOR)", lambda: run_commit(xml_path, None, verbose)),
        Action("p", "Push", lambda: run_push(xml_path, verbose)),
        Action("d", "Dev — Tauri app", lambda: run_dev(xml_path, verbose, True, False)),
        Action("w", "Dev — Web / WASM", lambda: run_dev(xml_path, verbose, False, True)),
        Action("i", "Init  (sync + deps)", _init),
        Action("b", "Build…", _build),
    ]


# --------------------------------------------------------------------------- #
# Curses rendering
# --------------------------------------------------------------------------- #

def _init_colors() -> None:
    import curses

    if not curses.has_colors():
        return
    curses.start_color()
    try:
        curses.use_default_colors()
        bg = -1
    except curses.error:
        bg = curses.COLOR_BLACK
    curses.init_pair(1, curses.COLOR_MAGENTA, bg)  # brand
    curses.init_pair(2, curses.COLOR_GREEN, bg)    # clean
    curses.init_pair(3, curses.COLOR_YELLOW, bg)   # dirty
    curses.init_pair(4, curses.COLOR_CYAN, bg)     # accent
    curses.init_pair(5, curses.COLOR_RED, bg)      # error
    curses.init_pair(6, curses.COLOR_WHITE, bg)    # name


def _cp(n: int) -> int:
    import curses

    try:
        return curses.color_pair(n)
    except curses.error:
        return 0


def _add(win, y: int, x: int, text: str, attr: int = 0) -> int:
    """Bounds-clipped addstr; returns the new x cursor."""
    import curses

    h, w = win.getmaxyx()
    if y < 0 or y >= h or x >= w:
        return x
    avail = w - x - 1
    if avail <= 0:
        return x
    chunk = text[:avail]
    try:
        win.addstr(y, x, chunk, attr)
    except curses.error:
        pass
    return x + len(chunk)


def _status_segments(st: RepoStatus):
    """(dot_color_pair, status_text, status_color_pair)."""
    if not st.exists:
        return 5, "not cloned", 5
    if not st.is_repo:
        return 5, "not a git repo", 5
    if st.changes:
        return 3, f"✚ {st.changes} changed", 3
    return 2, "✓ clean", 2


def _hline(win, y: int, x: int, length: int, attr: int = 0) -> None:
    _add(win, y, x, "─" * max(0, length), attr)


def _section(win, y: int, label: str, w: int) -> None:
    """A section header: ' LABEL ─────────────…' across the row."""
    import curses

    x = _add(win, y, 1, f" {label} ", curses.A_BOLD | _cp(4))
    _hline(win, y, x, w - x - 1, curses.A_DIM)


def _header_box(win, w: int, summary) -> None:
    """Rounded title box: brand on the left, summary stats on the right."""
    import curses

    border = _cp(1) | curses.A_DIM
    _add(win, 0, 0, "╭" + "─" * max(0, w - 2) + "╮", border)
    _add(win, 1, 0, "│", border)
    _add(win, 1, w - 1, "│", border)
    _add(win, 2, 0, "╰" + "─" * max(0, w - 2) + "╯", border)

    # ✨ is a double-width glyph; start the wordmark at a fixed column so the
    # gap after it survives on terminals that render the emoji as 2 cells.
    _add(win, 1, 2, "✨", curses.A_BOLD | _cp(1))
    x = _add(win, 1, 5, "ABTools", curses.A_BOLD | _cp(1))
    _add(win, 1, x, "  ·  Multi-repository management tool for AstroBox-NG", curses.A_DIM)

    total = sum(len(text) for text, _ in summary)
    sx = max(x + 2, w - 2 - total)
    for text, attr in summary:
        sx = _add(win, 1, sx, text, attr)


def _draw(stdscr, statuses, actions, sel, repo_scroll, message) -> int:
    import curses

    stdscr.erase()
    h, w = stdscr.getmaxyx()

    dirty = sum(1 for s in statuses if s.is_repo and s.changes)
    missing = sum(1 for s in statuses if not s.exists)
    summary = [
        (f"{len(statuses)} repos", curses.A_DIM),
        ("  ·  ", curses.A_DIM),
        (f"{dirty} dirty", _cp(3) | curses.A_BOLD if dirty else _cp(2)),
        ("  ·  ", curses.A_DIM),
        (f"{missing} absent", _cp(5) | curses.A_BOLD if missing else curses.A_DIM),
    ]
    _header_box(stdscr, w, summary)

    footer_row = h - 1
    actions_start = footer_row - len(actions)
    actions_header = actions_start - 1
    repo_header_row = 3
    repo_top = 4
    repo_bottom = actions_header - 2
    repo_visible = max(0, repo_bottom - repo_top + 1)

    max_scroll = max(0, len(statuses) - repo_visible)
    repo_scroll = max(0, min(repo_scroll, max_scroll))

    _section(stdscr, repo_header_row, "REPOSITORIES", w)
    if repo_scroll > 0:
        _add(stdscr, repo_header_row, w - 9, " ↑ more ", curses.A_DIM)

    name_w = max(16, min(40, w - 36))
    for row, st in enumerate(statuses[repo_scroll:repo_scroll + repo_visible]):
        y = repo_top + row
        dot_cp, status_text, status_cp = _status_segments(st)
        x = _add(stdscr, y, 2, "● ", _cp(dot_cp) | curses.A_BOLD)
        label = st.name + ("  (private)" if st.private else "")
        x = _add(stdscr, y, x, label[:name_w].ljust(name_w), _cp(6))
        x += 1
        x = _add(stdscr, y, x, (st.branch or "-")[:14].ljust(15), _cp(4) | curses.A_DIM)
        x = _add(stdscr, y, x, status_text, _cp(status_cp))
        if st.ahead:
            _add(stdscr, y, x + 1, f"↑{st.ahead}", _cp(4) | curses.A_BOLD)

    if repo_scroll + repo_visible < len(statuses):
        _add(stdscr, repo_bottom + 1, w - 9, " ↓ more ", curses.A_DIM)

    _section(stdscr, actions_header, "ACTIONS", w)
    for i, action in enumerate(actions):
        y = actions_start + i
        if i == sel:
            _add(stdscr, y, 1, "▌", _cp(1) | curses.A_BOLD)
            xx = _add(stdscr, y, 3, f"[{action.key}]", _cp(4) | curses.A_BOLD)
            _add(stdscr, y, xx + 1, action.label, _cp(6) | curses.A_BOLD)
        else:
            xx = _add(stdscr, y, 3, f"[{action.key}]", curses.A_DIM)
            _add(stdscr, y, xx + 1, action.label, 0)

    if message:
        _add(stdscr, footer_row, 1, message[: w - 2], _cp(4) | curses.A_BOLD)
    else:
        hint = "↑↓ move   ⏎ run   PgUp/PgDn scroll   r refresh   q quit"
        _add(stdscr, footer_row, 1, hint, curses.A_DIM)

    stdscr.refresh()
    return repo_scroll


def _suspend_and_run(stdscr, fn: Callable[[], int]) -> Optional[int]:
    import curses

    curses.def_prog_mode()
    curses.endwin()
    rc: Optional[int] = None
    try:
        print()
        rc = fn()
    except KeyboardInterrupt:
        ui.err("Interrupted.")
    finally:
        try:
            input("\n[✨ABTools] Press Enter to return to the menu...")
        except (EOFError, KeyboardInterrupt):
            pass
        curses.reset_prog_mode()
        stdscr.refresh()
    return rc


def _main_loop(stdscr, xml_path: Path, verbose: bool) -> None:
    import curses

    curses.curs_set(0)
    stdscr.keypad(True)
    _init_colors()

    actions = _build_actions(xml_path, verbose)
    sel = 0
    repo_scroll = 0
    message = "Scanning repositories…"
    repo_scroll = _draw(stdscr, [], actions, sel, repo_scroll, message)
    statuses = _scan(xml_path)
    message = None

    while True:
        repo_scroll = _draw(stdscr, statuses, actions, sel, repo_scroll, message)
        message = None
        ch = stdscr.getch()

        if ch in (ord("q"), ord("Q"), 27):  # q / ESC
            break
        elif ch in (curses.KEY_UP, ord("k")):
            sel = (sel - 1) % len(actions)
        elif ch in (curses.KEY_DOWN, ord("j")):
            sel = (sel + 1) % len(actions)
        elif ch == curses.KEY_NPAGE:
            repo_scroll += 5
        elif ch == curses.KEY_PPAGE:
            repo_scroll -= 5
        elif ch in (ord("r"), ord("R")):
            _draw(stdscr, statuses, actions, sel, repo_scroll, "Scanning repositories…")
            statuses = _scan(xml_path)
        elif ch in (curses.KEY_ENTER, 10, 13):
            rc = _suspend_and_run(stdscr, actions[sel].run)
            statuses = _scan(xml_path)
            if rc is not None:
                message = f"{actions[sel].label} → exit {rc}"
        elif ch == curses.KEY_RESIZE:
            continue
        else:
            for i, action in enumerate(actions):
                if ch == ord(action.key):
                    sel = i
                    rc = _suspend_and_run(stdscr, action.run)
                    statuses = _scan(xml_path)
                    if rc is not None:
                        message = f"{action.label} → exit {rc}"
                    break


def run_tui(xml_path: Path, verbose: bool) -> int:
    try:
        import curses  # noqa: F401
    except Exception:
        ui.err("TUI requires the standard-library 'curses' module, which isn't available here.")
        ui.err("Use the regular subcommands instead (sync / commit / push / dev …).")
        return 2

    if not xml_path.exists():
        ui.err(f"Error: XML not found: {xml_path}")
        return 2

    check_git_available()

    try:
        curses.wrapper(_main_loop, xml_path, verbose)
    except KeyboardInterrupt:
        pass
    return 0
