# -*- coding: utf-8 -*-
"""``build`` — run a platform build by target."""

from typing import List, Optional

from .. import ui
from ..shell import run_cmd


def run_build(target: Optional[str], extra: List[str]) -> int:
    if not target:
        ui.err("Error: missing build target. Example: python abtools.py build windows")
        return 2

    ui.out(f"Build target: {target}")
    if extra:
        ui.out(f"Extra args  : {' '.join(extra)}")

    try:
        lowered = target.lower()
        if lowered == "android":
            # TODO
            return 0

        if lowered == "ios":
            # TODO
            return 0

        if lowered == "wasm":
            wasm_cmd = ["wasm-pack", "build", "src-tauri/modules/app_wasm", "--target", "web"]
            if extra:
                wasm_cmd.extend(extra)
            rc, out = run_cmd(wasm_cmd)
            if rc != 0:
                ui.err("wasm-pack build failed:\n" + out)
                return rc
            ui.out(out)

            rc, out = run_cmd(["pnpm", "build"])
            if rc != 0:
                ui.err("pnpm build failed:\n" + out)
                return rc
            ui.out(out)
            return 0

        # TODO: other targets
        return 0
    except KeyboardInterrupt:
        ui.err("\nBuild interrupted.")
        return 130
    except Exception as ex:
        ui.err(f"Build exception: {ex}")
        return 1
