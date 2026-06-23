# -*- coding: utf-8 -*-
"""AstroBox multi-repo helper (``abtools``).

Split into focused modules:

- :mod:`_abtools.ui`       branded, colored output (the ``[✨ABTools]`` tag)
- :mod:`_abtools.shell`    subprocess runner + git availability check
- :mod:`_abtools.config`   repos.xml parsing + the pinned Cargo workspace content
- :mod:`_abtools.gitutil`  small git query/format helpers
- :mod:`_abtools.editor`   IME-friendly commit message editing via ``$EDITOR``
- :mod:`_abtools.commands` one module per subcommand
- :mod:`_abtools.cli`      argparse wiring + dispatch
"""
