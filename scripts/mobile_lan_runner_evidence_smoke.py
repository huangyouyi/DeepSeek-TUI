#!/usr/bin/env python3
"""Smoke tests for mobile_lan_runner_evidence.py."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "mobile_lan_runner_evidence.py"


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


def test_dry_run_does_not_write_bundle() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        output_root = Path(tmpdir) / "validation" / "mobile"
        output = run_script(
            "--host",
            "Linux Runner",
            "--date",
            "2026-05-24",
            "--output-root",
            str(output_root),
            "--dry-run",
        )

        planned = output_root / "2026-05-24-lan-runner-linux-runner"
        assert_contains(output, "LAN runner evidence bundle dry-run")
        assert_contains(output, f"Destination: {planned}")
        assert_contains(output, "commands.log")
        assert_contains(output, "results.md")
        assert_contains(output, "No files written.")
        if planned.exists():
            raise AssertionError(f"dry-run unexpectedly created {planned}")


def test_non_dry_run_writes_bundle_to_tempdir() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        output_root = Path(tmpdir) / "validation" / "mobile"
        output = run_script(
            "--host",
            "Loopback Host",
            "--date",
            "2026-05-24",
            "--output-root",
            str(output_root),
        )

        bundle = output_root / "2026-05-24-lan-runner-loopback-host"
        assert_contains(output, f"Created LAN runner evidence bundle: {bundle}")
        for relative in (
            "README.md",
            "evidence-log.md",
            "environment.md",
            "commands.log",
            "results.md",
            "logs/.gitkeep",
            "screenshots/.gitkeep",
            "failures/.gitkeep",
        ):
            path = bundle / relative
            if not path.exists():
                raise AssertionError(f"expected bundle path missing: {path}")

        commands = (bundle / "commands.log").read_text(encoding="utf-8")
        results = (bundle / "results.md").read_text(encoding="utf-8")
        assert_contains(commands, "cargo test -p kai-runner --test runner_mobile_core_live_e2e")
        assert_contains(commands, "python3 scripts/mobile_ios_flow_simulator.py --dry-run")
        assert_contains(
            commands,
            "python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json",
        )
        assert_contains(commands, "curl -i http://127.0.0.1:<port>/health")
        assert_contains(commands, "Do not start a real LAN listener from this harness.")
        assert_contains(results, "Loopback validation command suggestions")
        assert_contains(results, "cargo test -p kai-runner --test runner_mobile_core_live_e2e")
        assert_contains(results, "python3 scripts/mobile_ios_flow_simulator.py --dry-run")
        assert_contains(
            results,
            "python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json",
        )
        assert_contains(results, "curl -i http://127.0.0.1:<port>/health")
        assert_contains(results, "127.0.0.1")


def main() -> int:
    tests = [
        test_dry_run_does_not_write_bundle,
        test_non_dry_run_writes_bundle_to_tempdir,
    ]
    for test in tests:
        test()
        print(f"ok - {test.__name__}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
