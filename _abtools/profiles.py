# -*- coding: utf-8 -*-
"""Dev profiles: shared, named ``repo → branch`` maps.

- The catalog of profiles lives in ``abprofiles.json`` at the repo root and is
  **committed**, so the whole team shares the same set of dev configurations.
- Profiles are *sparse*: a profile only lists the repos it overrides; everything
  else stays on its default branch (from repos.xml).
- Which profile *you* currently have applied is **local** state, kept in
  ``.abtools/state.json`` (gitignored) — each developer applies independently.
"""

import json
from pathlib import Path
from typing import Dict, Optional

from . import ui

PROFILES_FILE = "abprofiles.json"
STATE_REL = Path(".abtools") / "state.json"

Profiles = Dict[str, Dict[str, str]]  # name -> {repo_key: branch}


def profiles_path(root: Path) -> Path:
    return root / PROFILES_FILE


def load_all(root: Path) -> Profiles:
    path = profiles_path(root)
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception as ex:
        ui.err(f"Failed to read {PROFILES_FILE}: {ex}")
        return {}
    if isinstance(data, dict) and isinstance(data.get("profiles"), dict):
        return {k: dict(v) for k, v in data["profiles"].items() if isinstance(v, dict)}
    return {}


def save_all(root: Path, profiles: Profiles) -> None:
    payload = {"version": 1, "profiles": {k: dict(sorted(v.items())) for k, v in sorted(profiles.items())}}
    profiles_path(root).write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def get(root: Path, name: str) -> Optional[Dict[str, str]]:
    return load_all(root).get(name)


def upsert(root: Path, name: str, mapping: Dict[str, str]) -> None:
    profiles = load_all(root)
    profiles[name] = dict(mapping)
    save_all(root, profiles)


def delete(root: Path, name: str) -> bool:
    profiles = load_all(root)
    if name not in profiles:
        return False
    del profiles[name]
    save_all(root, profiles)
    if get_active(root) == name:
        set_active(root, None)
    return True


# --- local "which profile is applied" state ------------------------------- #

def _state_path(root: Path) -> Path:
    return root / STATE_REL


def get_active(root: Path) -> Optional[str]:
    path = _state_path(root)
    if not path.exists():
        return None
    try:
        return (json.loads(path.read_text(encoding="utf-8")) or {}).get("active_profile")
    except Exception:
        return None


def set_active(root: Path, name: Optional[str]) -> None:
    path = _state_path(root)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"active_profile": name}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def active_overrides(root: Path) -> Dict[str, str]:
    """Branch overrides for the currently-applied profile (empty if none)."""
    name = get_active(root)
    if not name:
        return {}
    return get(root, name) or {}
