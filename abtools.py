#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import argparse
import sys
import subprocess
import time
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Tuple, List

def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)

def check_git_available() -> None:
    from shutil import which
    if which("git") is None:
        eprint("Error: 你妈的没装git。")
        sys.exit(2)

def run_cmd(cmd: List[str], cwd: Optional[Path] = None, retries: int = 2, retry_wait: float = 1.5) -> Tuple[int, str]:
    """
    带有简单重试功能的命令行执行器，防止网络踹踹包
    """
    last_out = ""
    for attempt in range(1, retries + 2):
        try:
            proc = subprocess.run(
                cmd,
                cwd=str(cwd) if cwd else None,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                check=False,
            )
            last_out = proc.stdout
            if proc.returncode == 0:
                return 0, last_out
            if attempt <= retries:
                time.sleep(retry_wait)
        except Exception as ex:
            last_out = f"[exception] {ex}"
            if attempt <= retries:
                time.sleep(retry_wait)
                continue
            return 1, last_out
    return 1, last_out


ANSI_RESET = "\033[0m"
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


def color_text(text: str, color: str) -> str:
    return f"{ANSI_COLORS.get(color, '')}{text}{ANSI_RESET}" if color in ANSI_COLORS else text


STATUS_STYLES = {
    "M": (ANSI_COLORS["yellow"], "🛠️"),
    "A": (ANSI_COLORS["green"], "🆕"),
    "D": (ANSI_COLORS["red"], "🗑️"),
    "R": (ANSI_COLORS["magenta"], "🔁"),
    "C": (ANSI_COLORS["cyan"], "📋"),
    "?": (ANSI_COLORS["blue"], "❓"),
    "!": (ANSI_COLORS["red"], "⚠️"),
    "U": (ANSI_COLORS["red"], "🤝"),
}


@dataclass
class RepoEntry:
    name: str
    path: Path
    is_private: bool


def collect_repo_entries(xml_path: Path, include_private: bool = True) -> List[RepoEntry]:
    root_dir = xml_path.parent.resolve()
    xml_root = load_xml(xml_path)
    entries: List[RepoEntry] = []

    for repo in xml_root.findall("repo"):
        name = repo.get("name") or (repo.get("path") or "(unnamed)")
        path_attr = repo.get("path")
        if not path_attr:
            eprint(f"Skip: {name}, missing path attribute.")
            continue

        repo_path = (root_dir / path_attr).resolve()
        is_private = get_repo_priv_flag(repo)
        if is_private and not include_private:
            continue

        entries.append(RepoEntry(name=name, path=repo_path, is_private=is_private))

    return entries


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
        color, candidate_emoji = STATUS_STYLES.get(ch, (ANSI_COLORS["gray"], "📄"))
        colored_status_chars.append(f"{color}{ch}{ANSI_RESET}")
        if emoji is None and candidate_emoji:
            emoji = candidate_emoji

    if emoji is None:
        emoji = "📄"

    colored_status = "".join(colored_status_chars)
    return f"{emoji} {colored_status} {remainder}".rstrip()


def format_status_output(raw: str) -> str:
    lines = [_colorize_status_line(line) for line in raw.rstrip().splitlines()]
    return "\n".join(lines)


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

    ahead_out = ahead_out.strip()
    try:
        ahead = int(ahead_out)
    except ValueError:
        ahead = None

    return upstream, ahead


def ensure_commit_message(provided: Optional[str]) -> str:
    if provided and provided.strip():
        return provided.strip()

    while True:
        try:
            msg = input("Enter commit message: ")
        except KeyboardInterrupt:
            print()
            raise
        except EOFError:
            msg = ""

        msg = msg.strip()
        if msg:
            return msg
        print("Commit message must not be empty. Please try again.")


def run_commit(xml_path: Path, message: Optional[str], verbose: bool) -> int:
    check_git_available()

    repo_root = xml_path.parent.resolve()
    entries: List[RepoEntry] = [RepoEntry(name="Root repository", path=repo_root, is_private=False)]
    entries.extend(collect_repo_entries(xml_path))

    print(color_text(">>> Checking repository status...", "cyan"))

    repos_with_changes: List[Tuple[RepoEntry, str]] = []
    overall_rc = 0

    for entry in entries:
        if not entry.path.exists():
            overall_rc = 1
            eprint(f"⚠️ Skipping {color_text(entry.name, 'yellow')}: path not found -> {entry.path}")
            continue

        if not is_git_repo(entry.path):
            overall_rc = 1
            eprint(f"⚠️ Skipping {color_text(entry.name, 'yellow')}: {entry.path} is not a git repository")
            continue

        rc, status_out = run_cmd(["git", "status", "--short"], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            eprint(f"⚠️ Unable to get status for {color_text(entry.name, 'yellow')}:\n{status_out}")
            continue

        if status_out.strip():
            repos_with_changes.append((entry, status_out))
            print(f"📂 {color_text(entry.name, 'white')} ({color_text(str(entry.path), 'gray')})")
            print(format_status_output(status_out))
            print("")
        elif verbose:
            print(f"😴 {color_text(entry.name, 'gray')} has no pending changes.")

    if not repos_with_changes:
        print(color_text("No repositories require commits. Nothing to do.", "yellow"))
        return overall_rc

    try:
        commit_message = ensure_commit_message(message)
    except KeyboardInterrupt:
        eprint(color_text("Commit flow cancelled.", "red"))
        return 130

    for entry, _ in repos_with_changes:
        print(f"📝 {color_text('Committing', 'magenta')} {color_text(entry.name, 'white')} ...")

        rc, out = run_cmd(["git", "add", "-A"], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            eprint(f"❌ {color_text(entry.name, 'red')} git add failed:\n{out}")
            continue

        rc, out = run_cmd(["git", "commit", "-m", commit_message], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            eprint(f"❌ {color_text(entry.name, 'red')} commit failed:\n{out}")
            continue

        print(f"✅ {color_text(entry.name, 'green')} commit finished.")

    return overall_rc


def run_push(xml_path: Path, verbose: bool) -> int:
    check_git_available()

    repo_root = xml_path.parent.resolve()
    entries: List[RepoEntry] = [RepoEntry(name="Root repository", path=repo_root, is_private=False)]
    entries.extend(collect_repo_entries(xml_path))

    overall_rc = 0

    for entry in entries:
        if not entry.path.exists():
            overall_rc = 1
            eprint(f"⚠️ Skipping {color_text(entry.name, 'yellow')}: path not found -> {entry.path}")
            continue

        if not is_git_repo(entry.path):
            overall_rc = 1
            eprint(f"⚠️ Skipping {color_text(entry.name, 'yellow')}: {entry.path} is not a git repository")
            continue

        rc, status_out = run_cmd(["git", "status", "--short"], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            eprint(f"⚠️ Unable to get status for {color_text(entry.name, 'yellow')}:\n{status_out}")
            continue

        if status_out.strip():
            overall_rc = 1
            eprint(f"⚠️ {color_text(entry.name, 'yellow')} still has uncommitted changes. Push skipped.")
            if verbose:
                print(format_status_output(status_out))
            continue

        rc, branch_out = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=entry.path)
        branch = branch_out.strip() if rc == 0 else "(unknown)"

        upstream, ahead = get_upstream_and_ahead(entry.path)
        if ahead is not None and ahead == 0:
            if verbose:
                target = upstream or "upstream"
                print(f"ℹ️ {color_text(entry.name, 'gray')} already matches {color_text(target, 'cyan')}. Skipping push.")
            continue

        rc, out = run_cmd(["git", "push"], cwd=entry.path)
        if rc != 0:
            overall_rc = 1
            eprint(f"❌ {color_text(entry.name, 'red')} push failed (current branch {color_text(branch, 'yellow')}):\n{out}")
            continue

        if ahead is not None and upstream:
            print(
                f"🚀 {color_text('Pushed', 'green')} {color_text(entry.name, 'white')} "
                f"({color_text(branch, 'cyan')}) -> {color_text(upstream, 'yellow')} "
                f"({color_text(str(ahead), 'green')} commit{'s' if ahead != 1 else ''})."
            )
        else:
            print(f"🚀 {color_text('Pushed', 'green')} {color_text(entry.name, 'white')} ({color_text(branch, 'cyan')}).")

    return overall_rc

def parse_bool(val: Optional[str]) -> bool:
    if val is None:
        return False
    return val.strip().lower() in ("1", "true", "yes", "y")

def get_repo_priv_flag(elem: ET.Element) -> bool:
    """同时支持 visibility='public|private' 和 private='true|false'"""
    vis = (elem.get("visibility") or "").strip().lower()
    if vis:
        if vis not in ("public", "private"):
            eprint(f"Warning: visibility='{vis}' is invalid; treat as public.")
            return False
        return vis == "private"
    return parse_bool(elem.get("private"))

def load_xml(xml_path: Path) -> ET.Element:
    if not xml_path.exists():
        eprint(f"Error: XML not found: {xml_path}")
        sys.exit(2)
    try:
        tree = ET.parse(str(xml_path))
        return tree.getroot()
    except ET.ParseError as ex:
        eprint(f"Error: XML parse failed: {ex}")
        sys.exit(2)

def ensure_dir(path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)

def is_git_repo(path: Path) -> bool:
    return (path / ".git").is_dir()

def safe_branch(elem: ET.Element) -> str:
    return (elem.get("branch") or "main").strip()

def sync_repos(xml_path: Path, include_private: bool, verbose: bool = False) -> int:
    """
    （默认仅处理公开仓；加 --private 才处理私有仓）
    """
    check_git_available()
    root = load_xml(xml_path)

    repos = root.findall("repo")
    if not repos:
        print("Note: no <repo> nodes found in XML.")
        return 0

    overall_rc = 0

    for repo in repos:
        name = repo.get("name") or "(unnamed)"
        url = repo.get("url")
        path_attr = repo.get("path")
        if not url or not path_attr:
            eprint(f"Skip: {name}, missing url or path.")
            overall_rc = overall_rc or 1
            continue

        target = Path(path_attr).resolve()
        branch = safe_branch(repo)
        is_private = get_repo_priv_flag(repo)

        if is_private and not include_private:
            if verbose:
                print(f"[skip private] {name} ({url}) -> {target}")
            continue

        print(f"📦 Syncing Repo: {name} ({branch}) ({url}) ({'private' if is_private else 'public'})")

        try:
            if target.exists() and is_git_repo(target):
                # fetch -> checkout -> pull (ff-only)
                rc, out = run_cmd(["git", "fetch", "--all", "--prune"], cwd=target)
                if rc != 0:
                    overall_rc = 1
                    eprint(f"[{name}] git fetch failed:\n{out}")
                    continue

                rc, out = run_cmd(["git", "checkout", branch], cwd=target)
                if rc != 0:
                    overall_rc = 1
                    eprint(f"[{name}] git checkout {branch} failed:\n{out}")
                    continue

                rc, out = run_cmd(["git", "pull", "--ff-only"], cwd=target)
                if rc != 0:
                    overall_rc = 1
                    eprint(f"[{name}] git pull failed (conflict/manual fix needed?):\n{out}")
                    continue

                print(f"✅ [{name}] Updated to latest.")
            else:
                # Fresh clone
                ensure_dir(target)
                rc, out = run_cmd(
                    ["git", "clone", "--branch", branch, "--single-branch", url, str(target.parent / target.name)]
                )
                if rc != 0:
                    overall_rc = 1
                    eprint(f"[{name}] git clone failed:\n{out}")
                    continue
                print(f"✅ [{name}] Successfully cloned to {target}.")

        except KeyboardInterrupt:
            eprint("\nOperation interrupted.")
            return 130
        except Exception as ex:
            overall_rc = 1
            eprint(f"[{name}] unexpected error: {ex}")

    if include_private:
        try:
            Path("./__PRIV_CLONED").write_text("ok\n", encoding="utf-8")
        except Exception as ex:
            eprint(f"[{name}] write __PRIV_CLONED failed: {ex}")

    return overall_rc

def run_build(target: Optional[str], extra: List[str]) -> int:
    if not target:
        eprint("Error: missing build target. Example: python abtools.py build windows")
        return 2

    print(f"Build target: {target}")
    if extra:
        print(f"Extra args  : {' '.join(extra)}")

    try:
        if target.lower() == "android":
            # TODO
            return 0

        elif target.lower() == "ios":
            # TODO
            return 0

        else:
            # TODO
            return 0
    except KeyboardInterrupt:
        eprint("\nBuild interrupted.")
        return 130
    except Exception as ex:
        eprint(f"Build exception: {ex}")
        return 1

def run_init_extras():
    print(">> init: installing node dependencies...")
    run_cmd(["pnpm", "i"])
    print("If needs, Please select all packages and confirm to build them.")
    run_cmd(["pnpm", "approve-builds"])

def run_init(xml_path: Path, include_private: bool, verbose: bool) -> int:
    print(">>> init: run sync first ...")
    rc = sync_repos(xml_path, include_private=include_private, verbose=verbose)
    if rc != 0:
        eprint("init: sync finished with errors (continued as much as possible).")
    print(">>> init: run extras ...")
    try:
        run_init_extras()
    except Exception as ex:
        eprint(f"init extras error: {ex}")
        rc = rc or 1
    return rc

def print_help_and_exit(parser: argparse.ArgumentParser):
    print(parser.format_help())
    sys.exit(0)

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="abtools",
        description="Lightweight CLI to sync multiple repos and run builds for AstroBox.",
        add_help=False,  # we provide a unified 'help' subcommand
    )

    parser.add_argument("-h", "--help", action="store_true", help="Show help")
    parser.add_argument("-f", "--file", default="repos.xml", help="Path to repos.xml (default: repos.xml)")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose logging")

    subparsers = parser.add_subparsers(dest="command")

    # init
    p_init = subparsers.add_parser("init", help="Run sync, then custom extras")
    p_init.add_argument("--private", action="store_true", help="Include private repos (requires configured git creds)")

    # sync
    p_sync = subparsers.add_parser("sync", help="Clone/update repos from repos.xml")
    p_sync.add_argument("--private", action="store_true", help="Include private repos (requires configured git creds)")

    # build
    p_build = subparsers.add_parser("build", help="Run build by target (no XML, no --cmd)")
    p_build.add_argument("target", nargs="?", help="Build target, e.g. android / ios / web / backend")
    p_build.add_argument("extra", nargs=argparse.REMAINDER, help="Extra args passed to target branch")

    # commit
    p_commit = subparsers.add_parser("commit", help="Commit root repo and all sub repos")
    p_commit.add_argument("-m", "--message", help="Commit message (prompted if omitted)")

    # push
    subparsers.add_parser("push", help="Push root repo and all sub repos")

    # help
    subparsers.add_parser("help", help="Print command help")

    return parser

def main():
    parser = build_parser()
    args = parser.parse_args()

    if args.help or not args.command:
        print_help_and_exit(parser)

    xml_path = Path(args.file).resolve()

    if args.command == "init":
        rc = run_init(xml_path, include_private=args.private, verbose=args.verbose)
        print("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "sync":
        rc = sync_repos(xml_path, include_private=args.private, verbose=args.verbose)
        print("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "build":
        rc = run_build(args.target, args.extra)
        print("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "commit":
        rc = run_commit(xml_path, args.message, args.verbose)
        print("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "push":
        rc = run_push(xml_path, args.verbose)
        print("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "help":
        print_help_and_exit(parser)

    else:
        eprint(f"Unknown command: {args.command}")
        print_help_and_exit(parser)

if __name__ == "__main__":
    main()
