#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""Thin launcher for the AstroBox multi-repo helper.

The real implementation lives in the ``_abtools`` package next to this file.
Keeping this shim means ``python abtools.py <command>`` keeps working exactly
as before while the logic is split into small, readable modules.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from _abtools.cli import main

if __name__ == "__main__":
    main()
