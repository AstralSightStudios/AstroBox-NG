#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import argparse
import sys
import subprocess
import time
import xml.etree.ElementTree as ET
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Tuple, List, Dict

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


@dataclass
class WorkspaceRewriteSummary:
    included_modules: List[str]
    included_plugins: List[str]
    skipped_private_modules: List[str]
    skipped_private_plugins: List[str]
    missing_modules: List[str]
    missing_plugins: List[str]
    members_entries: List[str]


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


def get_head_commit(repo_path: Path) -> Optional[str]:
    rc, out = run_cmd(["git", "rev-parse", "HEAD"], cwd=repo_path)
    if rc != 0:
        return None
    commit = out.strip()
    return commit or None


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


def collect_pull_change_details(repo_path: Path, old_head: str, new_head: str) -> Tuple[Optional[str], Optional[str]]:
    status_block: Optional[str] = None
    log_block: Optional[str] = None

    rc, diff_out = run_cmd(
        ["git", "diff", "--name-status", f"{old_head}..{new_head}"],
        cwd=repo_path,
    )
    if rc == 0 and diff_out.strip():
        status_lines = _diff_to_status_lines(diff_out)
        if status_lines:
            status_block = format_status_output("\n".join(status_lines))

    rc, log_out = run_cmd(
        ["git", "log", "--oneline", f"{old_head}..{new_head}"],
        cwd=repo_path,
    )
    if rc == 0 and log_out.strip():
        log_block = log_out.strip()

    return status_block, log_block


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


def ensure_commit_message(provided: Optional[str], repo_name: Optional[str] = None) -> str:
    if provided and provided.strip():
        return provided.strip()

    prompt = "Enter commit message"
    if repo_name:
        prompt += f" [{repo_name}]"
    prompt += ": "

    while True:
        try:
            msg = input(prompt)
        except KeyboardInterrupt:
            print()
            raise
        except EOFError:
            msg = ""

        msg = msg.strip()
        if msg:
            return msg
        print("Commit message must not be empty. Please try again.")


def run_commit(
    xml_path: Path,
    message: Optional[str],
    verbose: bool,
    reset_workspace: bool,
    restore_workspace: bool,
) -> int:
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

    cargo_toml = repo_root / "src-tauri" / "Cargo.toml"

    for entry, _ in repos_with_changes:
        try:
            commit_message = ensure_commit_message(message, repo_name=entry.name)
        except KeyboardInterrupt:
            eprint(color_text("Commit flow cancelled.", "red"))
            return 130

        print(f"📝 {color_text('Committing', 'magenta')} {color_text(entry.name, 'white')} ...")

        guard_enabled = reset_workspace and entry.path == repo_root
        with CargoWorkspaceGuard(
            cargo_toml=cargo_toml,
            enabled=guard_enabled,
            restore_after=restore_workspace,
            verbose=verbose,
        ):
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


def collect_workspace_flags(xml_root: ET.Element) -> Tuple[Dict[str, bool], Dict[str, bool]]:
    """Return visibility flags for modules and plugins based on repos.xml."""

    def extract_component(path_parts: Tuple[str, ...], marker: str) -> Optional[str]:
        for idx, part in enumerate(path_parts):
            if part == marker and idx + 1 < len(path_parts):
                return path_parts[idx + 1]
        return None

    modules: Dict[str, bool] = {}
    plugins: Dict[str, bool] = {}

    for repo in xml_root.findall("repo"):
        path_attr = repo.get("path")
        if not path_attr:
            continue

        parts = Path(path_attr).parts

        module_name = extract_component(parts, "modules")
        if module_name:
            modules[module_name] = get_repo_priv_flag(repo)
            continue

        plugin_name = extract_component(parts, "plugins")
        if plugin_name:
            plugins[plugin_name] = get_repo_priv_flag(repo)

    return modules, plugins


def format_toml_array(items: List[str]) -> str:
    if not items:
        return "[]"

    lines = ["["]
    for item in items:
        lines.append(f'    "{item}",')
    lines.append("]")
    return "\n".join(lines)


def ensure_trailing_newline(text: str) -> str:
    return text if text.endswith("\n") else text + "\n"


def generate_default_workspace_content() -> str:
    lines = [
        "[workspace]",
        "members = [",
        '    "modules/*",',
        '    "plugins/*"',
        "]",
        'resolver = "3"',
        "",
        'default-members = ["modules/app"]',
        "",
    ]
    return ensure_trailing_newline("\n".join(lines))


class CargoWorkspaceGuard:
    def __init__(self, cargo_toml: Path, enabled: bool, restore_after: bool, verbose: bool):
        self.cargo_toml = cargo_toml
        self.enabled = enabled
        self.restore_after = restore_after
        self.verbose = verbose
        self.original_text: Optional[str] = None
        self.changed = False

    def __enter__(self):
        if not self.enabled or not self.cargo_toml.exists():
            return self

        try:
            self.original_text = self.cargo_toml.read_text(encoding="utf-8")
        except Exception as ex:
            eprint(f"Warning: failed to read {self.cargo_toml}: {ex}")
            return self

        default_content = generate_default_workspace_content()
        if self.original_text != default_content:
            if self.verbose:
                print(f"[commit] Resetting {self.cargo_toml} to default workspace before staging.")
            try:
                self.cargo_toml.write_text(default_content, encoding="utf-8")
                self.changed = True
            except Exception as ex:
                eprint(f"Warning: failed to write default workspace to {self.cargo_toml}: {ex}")

        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.changed and self.restore_after and self.original_text is not None:
            try:
                self.cargo_toml.write_text(self.original_text, encoding="utf-8")
                if self.verbose:
                    print(f"[commit] Restored {self.cargo_toml} after commit.")
            except Exception as ex:
                eprint(f"Warning: failed to restore {self.cargo_toml}: {ex}")
        return False


def prepare_workspace_entries(
    base_dir: Path,
    flags: Dict[str, bool],
    include_private: bool,
    verbose: bool,
    prefix: str,
) -> Tuple[List[str], List[str], List[str]]:
    """Build inclusion/skip lists for a workspace subdirectory."""

    available: Dict[str, bool] = dict(flags)

    if base_dir.exists():
        for entry in base_dir.iterdir():
            if entry.is_dir():
                available.setdefault(entry.name, False)

    included: List[str] = []
    skipped_private: List[str] = []
    missing: List[str] = []

    for name in sorted(available.keys()):
        entry_path = base_dir / name
        is_private = available[name]
        path_label = f"{prefix}/{name}"

        if is_private and not include_private:
            skipped_private.append(path_label)
            if verbose:
                print(f"[skip private] {path_label}")
            continue

        if not entry_path.exists():
            missing.append(path_label)
            if verbose:
                print(f"[skip missing] {path_label} (directory missing)")
            continue

        included.append(path_label)

    return included, skipped_private, missing


def rewrite_cargo_workspace(
    cargo_toml: Path,
    modules_dir: Path,
    plugins_dir: Path,
    module_flags: Dict[str, bool],
    plugin_flags: Dict[str, bool],
    include_private: bool,
    verbose: bool,
    dry_run: bool = False,
) -> WorkspaceRewriteSummary:
    """Build workspace members/default-members for modules and plugins respecting visibility."""

    included_modules, skipped_modules, missing_modules = prepare_workspace_entries(
        modules_dir, module_flags, include_private, verbose, "modules"
    )
    included_plugins, skipped_plugins, missing_plugins = prepare_workspace_entries(
        plugins_dir, plugin_flags, include_private, verbose, "plugins"
    )

    members_entries = included_modules + included_plugins

    default_member = None
    preferred = "modules/app"
    if preferred in included_modules:
        default_member = preferred
    elif included_modules:
        default_member = included_modules[0]
    elif members_entries:
        default_member = members_entries[0]

    members_block = format_toml_array(members_entries)
    default_block = format_toml_array([default_member] if default_member else [])

    if not dry_run:
        lines = [
            "[workspace]",
            f"members = {members_block}",
            'resolver = "3"',
            "",
            f"default-members = {default_block}",
            "",
        ]

        cargo_toml.write_text("\n".join(lines), encoding="utf-8")

    return WorkspaceRewriteSummary(
        included_modules=included_modules,
        included_plugins=included_plugins,
        skipped_private_modules=skipped_modules,
        skipped_private_plugins=skipped_plugins,
        missing_modules=missing_modules,
        missing_plugins=missing_plugins,
        members_entries=members_entries,
    )

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
                old_head = get_head_commit(target)

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

                new_head = get_head_commit(target)

                if old_head and new_head and old_head != new_head:
                    short_from = old_head[:7]
                    short_to = new_head[:7]
                    print(
                        f"✅ {color_text(name, 'white')} "
                        f"{color_text(short_from, 'cyan')} -> {color_text(short_to, 'cyan')}"
                    )

                    status_block, log_block = collect_pull_change_details(target, old_head, new_head)

                    if status_block:
                        print(status_block)

                    if log_block:
                        print(color_text("📜 Commit log:", "gray"))
                        for line in log_block.splitlines():
                            print(f"    {line}")

                    print("")
                elif verbose:
                    print(f"😴 {color_text(name, 'gray')} already up to date.")

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


def run_dev(xml_path: Path, verbose: bool, dry_run: bool, tauri: bool) -> int:
    project_root = xml_path.parent.resolve()
    cargo_toml = project_root / "src-tauri" / "Cargo.toml"

    if not cargo_toml.exists():
        eprint(f"Error: Cargo.toml not found -> {cargo_toml}")
        return 2

    xml_root = load_xml(xml_path)
    module_flags, plugin_flags = collect_workspace_flags(xml_root)
    modules_dir = cargo_toml.parent / "modules"
    plugins_dir = cargo_toml.parent / "plugins"

    if not modules_dir.exists():
        eprint(
            f"Warning: {modules_dir} is missing; the Cargo workspace may still fail to compile after dev."
        )

    include_private = (project_root / "__PRIV_CLONED").exists()

    print(">>> dev: refreshing Cargo workspace based on __PRIV_CLONED ...")
    print(f"    - include_private = {'yes' if include_private else 'no'}")
    if dry_run:
        print("    - dry_run = yes (preview only, no write)")

    summary = rewrite_cargo_workspace(
        cargo_toml,
        modules_dir,
        plugins_dir,
        module_flags,
        plugin_flags,
        include_private,
        verbose,
        dry_run=dry_run,
    )

    if verbose:
        if summary.included_modules:
            print("    - Included modules:")
            for item in summary.included_modules:
                print(f"        * {item}")
        if summary.included_plugins:
            print("    - Included plugins:")
            for item in summary.included_plugins:
                print(f"        * {item}")
        if summary.skipped_private_modules or summary.skipped_private_plugins:
            print("    - Skipped private entries:")
            for item in summary.skipped_private_modules + summary.skipped_private_plugins:
                print(f"        * {item}")
        if summary.missing_modules or summary.missing_plugins:
            print("    - Skipped missing directories:")
            for item in summary.missing_modules + summary.missing_plugins:
                print(f"        * {item}")

        if summary.members_entries:
            print("    - Final workspace members:")
            for item in summary.members_entries:
                print(f"        * {item}")

    if not include_private and (
        summary.skipped_private_modules or summary.skipped_private_plugins
    ):
        print(
            "Tip: __PRIV_CLONED not detected, so private modules/plugins were excluded. "
            "After cloning private repos, run `touch __PRIV_CLONED` and rerun."
        )

    if summary.missing_modules or summary.missing_plugins:
        print(
            "Warning: Some workspace directories are missing; Cargo may still fail to build. "
            "Run `python abtools.py sync --private` to fetch them."
        )

    if dry_run:
        print("ℹ️ dry-run mode did not modify Cargo.toml.")
    else:
        print("✅ dev refreshed Cargo workspace. This change is not committed automatically; keep or revert it as needed.")

    if tauri:
        os.system("pnpm tauri:dev")

    return 0

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
    p_build = subparsers.add_parser("build", help="Run build by target")
    p_build.add_argument("target", nargs="?", help="Build target, e.g. android / ios / wasm")
    p_build.add_argument("extra", nargs=argparse.REMAINDER, help="Extra args passed to target branch")

    # dev
    p_dev = subparsers.add_parser(
        "dev",
        help="Rewrite Cargo workspace members based on private-module availability",
    )
    p_dev.add_argument("--dry-run", action="store_true", help="Preview changes without touching files")
    p_dev.add_argument("--tauri", action="store_true", help="Run tauri app development")

    # commit
    p_commit = subparsers.add_parser("commit", help="Commit root repo and all sub repos")
    p_commit.add_argument("-m", "--message", help="Commit message (prompted if omitted)")
    p_commit.add_argument(
        "--no-restore-workspace",
        dest="restore_workspace",
        action="store_false",
        help="Keep the default workspace after commit instead of restoring the pre-commit content",
    )
    p_commit.set_defaults(restore_workspace=True)

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

    elif args.command == "dev":
        rc = run_dev(xml_path, verbose=args.verbose, dry_run=args.dry_run, tauri=args.tauri)
        if rc == 0 and args.dry_run:
            print("✅ All tasks have been completed (dry-run)")
        else:
            print("✅ All tasks have been completed")
        sys.exit(rc)

    elif args.command == "commit":
        rc = run_commit(
            xml_path,
            args.message,
            args.verbose,
            True,
            args.restore_workspace,
        )
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
