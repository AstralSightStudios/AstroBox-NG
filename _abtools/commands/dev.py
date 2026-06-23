# -*- coding: utf-8 -*-
"""``dev`` — make sure the pinned Cargo workspace is intact, then run the app.

Because ``src-tauri/Cargo.toml`` is pinned to a wildcard workspace that adapts
to whatever module dirs exist, ``dev`` no longer rewrites it per private/public
availability. It just self-heals the file if something drifted it away from the
canonical content, then launches the requested dev server.
"""

import os
from pathlib import Path

from .. import ui
from ..config import canonical_cargo_toml
from ..shell import run_cmd


def _ensure_canonical_cargo(cargo_toml: Path, verbose: bool) -> None:
    desired = canonical_cargo_toml()
    try:
        current = cargo_toml.read_text(encoding="utf-8") if cargo_toml.exists() else None
    except Exception as ex:
        ui.err(f"Warning: failed to read {cargo_toml}: {ex}")
        current = None

    if current == desired:
        if verbose:
            ui.out("Root Cargo.toml already matches the pinned wildcard workspace.")
        return

    try:
        cargo_toml.write_text(desired, encoding="utf-8")
        ui.out(ui.color("Re-pinned src-tauri/Cargo.toml to the canonical wildcard workspace.", "yellow"))
    except Exception as ex:
        ui.err(f"Warning: failed to write canonical workspace to {cargo_toml}: {ex}")


def _run(label: str, cmd: str) -> int:
    ui.out(f">>> dev: {label} ({ui.color(cmd, 'cyan')}) ...")
    code = os.system(cmd)
    return 0 if code == 0 else 1


def run_dev(xml_path: Path, verbose: bool, tauri: bool, wasm: bool) -> int:
    project_root = xml_path.parent.resolve()
    cargo_toml = project_root / "src-tauri" / "Cargo.toml"

    if not cargo_toml.exists():
        ui.err(f"Error: Cargo.toml not found -> {cargo_toml}")
        return 2

    if tauri and wasm:
        ui.err("Error: --tauri and --wasm cannot be used together.")
        return 2

    _ensure_canonical_cargo(cargo_toml, verbose)

    if not (tauri or wasm):
        ui.out("Workspace verified. Pass --tauri or --wasm to start a dev server.")
        ui.out("  python abtools.py dev --tauri   # run the Tauri app")
        ui.out("  python abtools.py dev --wasm    # run the web dev server (wasm backend)")
        return 0

    if tauri:
        return _run("starting Tauri app", "pnpm tauri:dev")

    # wasm
    ui.out(">>> dev: building wasm package (dev profile)...")
    rc, out = run_cmd(
        ["wasm-pack", "build", "src-tauri/modules/app_wasm", "--target", "web", "--dev"],
        cwd=project_root,
    )
    if rc != 0:
        ui.err("wasm-pack build failed:\n" + out)
        return rc
    if verbose:
        ui.out(out)

    return _run("starting web dev server", "pnpm dev")
