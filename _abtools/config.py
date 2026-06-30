# -*- coding: utf-8 -*-
"""repos.xml parsing and the pinned Cargo workspace content.

The root ``src-tauri/Cargo.toml`` is *pinned* to a single, static, wildcard
workspace. Because ``members = ["modules/*", "plugins/*"]`` globs whatever is
actually present on disk, the exact same committed file works for both:

- the open-source checkout (private module dirs simply aren't there), and
- the full private checkout (every module dir is present),

so both ``cargo check`` cleanly with no per-checkout rewriting. The private vs.
public distinction is therefore expressed by *which directories exist*, not by
editing this file. ``default-members`` is intentionally omitted: the private
``modules/app`` crate doesn't exist in a public checkout, and the tauri build
(``scripts/tauri-runner.mjs``) ``cd``s into ``modules/app`` anyway, so it never
relied on it.
"""

import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional

from . import ui

# Crates that need a dedicated toolchain and must stay out of the default
# workspace (still globbed by members, so explicitly excluded).
WORKSPACE_STATIC_EXCLUDES = ["modules/app_esp32s3"]

# [patch.crates-io] pins required by the workspace.
PATCH_CRATES = [
    'rustix = { git = "https://github.com/bytecodealliance/rustix", version = "1.1.4" }',
    'swift-rs = { git = "https://github.com/Searchstars/swift-rs", branch = "main" }',
    'wasmtime-internal-fiber = { git = "https://github.com/Searchstars/wasmtime", branch = "astrobox-38.0.4-android-fiber-fix" }',
    'tao = { git = "https://github.com/Searchstars/tao", branch = "fix/ios-scene-configuration-lifetime" }',
]


@dataclass
class RepoEntry:
    name: str
    path: Path
    is_private: bool


# Stable key for the root repo inside dev profiles.
ROOT_KEY = "."


@dataclass
class WorkspaceRepo:
    name: str
    path: Path
    key: str  # ROOT_KEY for the root repo, otherwise the repos.xml ``path``
    url: Optional[str]  # None for the root repo (never cloned)
    branch: str  # default branch from repos.xml (or the root's current branch)
    is_private: bool


def list_workspace(
    xml_path: Path, project_root: Path, root_branch: str
) -> List["WorkspaceRepo"]:
    """The root repo plus every <repo> in repos.xml, as a uniform list.

    ``root_branch`` is passed in (callers know it via git) so this stays pure XML.
    """
    repos: List[WorkspaceRepo] = [
        WorkspaceRepo("Root repository", project_root, ROOT_KEY, None, root_branch, False)
    ]
    xml_root = load_xml(xml_path)
    for repo in xml_root.findall("repo"):
        url = repo.get("url")
        path_attr = repo.get("path")
        name = repo.get("name") or (path_attr or "(unnamed)")
        if not url or not path_attr:
            ui.err(f"Skip: {name}, missing url or path.")
            continue
        repos.append(
            WorkspaceRepo(
                name=name,
                path=(project_root / path_attr).resolve(),
                key=path_attr,
                url=url,
                branch=safe_branch(repo),
                is_private=get_repo_priv_flag(repo),
            )
        )
    return repos


def _toml_array(items: List[str]) -> str:
    if not items:
        return "[]"
    body = "\n".join(f'    "{item}",' for item in items)
    return "[\n" + body + "\n]"


def canonical_cargo_toml() -> str:
    """The one true content of ``src-tauri/Cargo.toml`` (with trailing newline)."""
    lines = [
        "[workspace]",
        f'members = {_toml_array(["modules/*", "plugins/*"])}',
    ]
    if WORKSPACE_STATIC_EXCLUDES:
        lines.append(f"exclude = {_toml_array(WORKSPACE_STATIC_EXCLUDES)}")
    lines += [
        'resolver = "3"',
        "",
        "[patch.crates-io]",
        *PATCH_CRATES,
        "",
    ]
    return "\n".join(lines)


def load_xml(xml_path: Path) -> ET.Element:
    if not xml_path.exists():
        ui.err(f"Error: XML not found: {xml_path}")
        sys.exit(2)
    try:
        tree = ET.parse(str(xml_path))
        return tree.getroot()
    except ET.ParseError as ex:
        ui.err(f"Error: XML parse failed: {ex}")
        sys.exit(2)


def parse_bool(val) -> bool:
    if val is None:
        return False
    return val.strip().lower() in ("1", "true", "yes", "y")


def get_repo_priv_flag(elem: ET.Element) -> bool:
    """Support both visibility='public|private' and private='true|false'."""
    vis = (elem.get("visibility") or "").strip().lower()
    if vis:
        if vis not in ("public", "private"):
            ui.err(f"Warning: visibility='{vis}' is invalid; treat as public.")
            return False
        return vis == "private"
    return parse_bool(elem.get("private"))


def safe_branch(elem: ET.Element) -> str:
    return (elem.get("branch") or "main").strip()


def collect_repo_entries(xml_path: Path, include_private: bool = True) -> List[RepoEntry]:
    root_dir = xml_path.parent.resolve()
    xml_root = load_xml(xml_path)
    entries: List[RepoEntry] = []

    for repo in xml_root.findall("repo"):
        name = repo.get("name") or (repo.get("path") or "(unnamed)")
        path_attr = repo.get("path")
        if not path_attr:
            ui.err(f"Skip: {name}, missing path attribute.")
            continue

        repo_path = (root_dir / path_attr).resolve()
        is_private = get_repo_priv_flag(repo)
        if is_private and not include_private:
            continue

        entries.append(RepoEntry(name=name, path=repo_path, is_private=is_private))

    return entries
