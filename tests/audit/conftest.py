"""Pytest config for F4 audit tests.

Makes `scripts/audit/data-content-check.py` importable as a module so the
unit tests can directly exercise its handlers and the `data_content_check`
driver without going through the CLI. The checker lives outside the
conventional Python-package tree, so we manipulate `sys.path` here rather
than moving the script.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "audit" / "data-content-check.py"


def _load_checker_module():
    """Dynamically import the checker script as a module (`data_content_check`).

    The file name `data-content-check.py` contains hyphens which are invalid in
    Python module identifiers, so a plain `import` cannot work. importlib's
    spec_from_file_location handles this cleanly.
    """
    spec = importlib.util.spec_from_file_location(
        "data_content_check", SCRIPT_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"could not load spec for {SCRIPT_PATH}")
    module = importlib.util.module_from_spec(spec)
    sys.modules["data_content_check"] = module
    spec.loader.exec_module(module)
    return module


# Load once at collection so tests can import the module via
# `import data_content_check` (or via the fixture below).
_load_checker_module()


import pytest  # noqa: E402


@pytest.fixture
def checker():
    """Returns the loaded data_content_check module."""
    import data_content_check  # noqa: F401
    return sys.modules["data_content_check"]


@pytest.fixture
def fixtures_dir() -> Path:
    """Absolute path to tests/audit/fixtures/."""
    return Path(__file__).resolve().parent / "fixtures"


@pytest.fixture
def rules_path() -> Path:
    """Absolute path to the real rule pack YAML used by the checker.

    Tests can read from this path directly since it is git-tracked and
    stable. For tests that need a *synthetic* rule set, they should build
    one inline and pass to the driver rather than mutating this file.
    """
    return REPO_ROOT / ".planning" / "functional-layer" / "data-content-rules.yaml"
