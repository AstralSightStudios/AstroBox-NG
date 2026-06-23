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
from typing import List

from . import ui

# Crates that need a dedicated toolchain and must stay out of the default
# workspace (still globbed by members, so explicitly excluded).
WORKSPACE_STATIC_EXCLUDES = ["modules/app_esp32s3"]

# [patch.crates-io] pins required by the workspace.
PATCH_CRATES = [
    'rustix = { git = "https://github.com/bytecodealliance/rustix", version = "1.1.4" }',
    'swift-rs = { git = "https://github.com/Searchstars/swift-rs", branch = "main" }',
    'wasmtime-internal-fiber = { git = "https://github.com/Searchstars/wasmtime", branch = "astrobox-38.0.4-android-fiber-fix" }',
]


@dataclass
class RepoEntry:
    name: str
    path: Path
    is_private: bool


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
