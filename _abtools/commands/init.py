# -*- coding: utf-8 -*-
"""``init`` — sync everything, then run the project bootstrap extras."""

from pathlib import Path

from .. import ui
from ..shell import run_cmd
from .sync import run_sync


def run_init_extras() -> None:
    ui.out(">> init: installing node dependencies...")
    run_cmd(["pnpm", "i"])
    ui.out("If needs, Please select all packages and confirm to build them.")
    run_cmd(["pnpm", "approve-builds"])


def run_init(xml_path: Path, include_private: bool, verbose: bool) -> int:
    ui.out(">>> init: run sync first ...")
    rc = run_sync(xml_path, include_private=include_private, verbose=verbose)
    if rc != 0:
        ui.err("init: sync finished with errors (continued as much as possible).")

    ui.out(">>> init: run extras ...")
    try:
        run_init_extras()
    except Exception as ex:
        ui.err(f"init extras error: {ex}")
        rc = rc or 1

    return rc
