# -*- coding: utf-8 -*-
"""Subprocess helpers shared by every command."""

import os
import subprocess
import sys
import time
from pathlib import Path
from shutil import which
from typing import Dict, List, Optional, Tuple

from . import ui


def check_git_available() -> None:
    if which("git") is None:
        ui.err("Error: 你妈的没装git。")
        sys.exit(2)


def run_cmd(
    cmd: List[str],
    cwd: Optional[Path] = None,
    retries: int = 2,
    retry_wait: float = 1.5,
    env: Optional[Dict[str, str]] = None,
) -> Tuple[int, str]:
    """Run a command with a small retry loop (network ops can flap).

    ``env`` entries are merged on top of the current environment.
    """
    full_env = {**os.environ, **env} if env else None
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
                env=full_env,
            )
            last_out = proc.stdout
            if proc.returncode == 0:
                return 0, last_out
            if attempt <= retries:
                time.sleep(retry_wait)
        except Exception as ex:  # pragma: no cover - defensive
            last_out = f"[exception] {ex}"
            if attempt <= retries:
                time.sleep(retry_wait)
                continue
            return 1, last_out
    return 1, last_out
