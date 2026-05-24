#!/usr/bin/env python3
"""Smoke tests for mobile_evidence_bundle.py."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "mobile_evidence_bundle.py"


def run_script(*args: str) -> str:
    completed = subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=REPO_ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return completed.stdout


def assert_contains(output: str, needle: str) -> None:
    if needle not in output:
        raise AssertionError(f"missing {needle!r} in output:\n{output}")


def test_list_templates() -> None:
    output = run_script("--list")

    assert_contains(output, "Available mobile evidence bundle templates:")
    assert_contains(output, "- macos-host")
    assert_contains(output, "  - evidence-log.md")
    assert_contains(output, "- lan-runner")
    assert_contains(output, "  - screenshots/.gitkeep")


def test_dry_run_does_not_write_bundle() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        output_root = Path(tmpdir) / "validation" / "mobile"
        output = run_script(
            "--platform",
            "ios-simulator",
            "--host",
            "iPhone 15",
            "--date",
            "2026-05-24",
            "--output-root",
            str(output_root),
            "--dry-run",
        )

        planned = output_root / "2026-05-24-ios-simulator-iphone-15"
        assert_contains(output, f"Destination: {planned}")
        assert_contains(output, "evidence-log.md")
        assert_contains(output, "Dry-run only; no files written.")
        if planned.exists():
            raise AssertionError(f"dry-run unexpectedly created {planned}")


def main() -> int:
    tests = [test_list_templates, test_dry_run_does_not_write_bundle]
    for test in tests:
        test()
        print(f"ok - {test.__name__}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
