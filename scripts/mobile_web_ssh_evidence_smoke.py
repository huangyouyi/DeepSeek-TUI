#!/usr/bin/env python3
"""Smoke-test the mobile Web SSH evidence helper without real SSH."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
HELPER = REPO_ROOT / "scripts" / "mobile_web_ssh_evidence.py"


def run_helper(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(HELPER), *args],
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def assert_contains(haystack: str, needle: str) -> None:
    if needle not in haystack:
        raise AssertionError(f"expected {needle!r} in output:\n{haystack}")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="mobile-web-ssh-evidence-smoke-") as tmp:
        output_root = Path(tmp)
        result = run_helper(
            [
                "--server",
                "http://127.0.0.1:8788",
                "--host",
                "Lab Phone",
                "--date",
                "2026-05-25",
                "--output-root",
                str(output_root),
                "--auto-approve",
                "--json",
                "--dry-run",
            ]
        )
        if result.returncode != 0:
            raise AssertionError(
                "dry-run helper failed\n"
                f"stdout:\n{result.stdout}\n"
                f"stderr:\n{result.stderr}"
            )

        payload = json.loads(result.stdout)
        destination = output_root / "2026-05-25-mobile-web-ssh-lab-phone"
        if payload["destination"] != str(destination):
            raise AssertionError(payload)
        if destination.exists():
            raise AssertionError(f"dry-run wrote destination: {destination}")

        planned_paths = "\n".join(payload["planned_paths"])
        assert_contains(planned_paths, "README.md")
        assert_contains(planned_paths, "logs/.gitkeep")
        planned_commands = "\n".join(" ".join(command) for command in payload["planned_commands"])
        assert_contains(planned_commands, "scripts/mobile_web_ssh_smoke.py")
        assert_contains(planned_commands, "scripts/mobile_web_ssh_flow_simulator.py")
        assert_contains(planned_commands, "--auto-approve")
        assert_contains(payload["boundary"], "Linux/LAN Web simulator evidence only")

    print("mobile_web_ssh_evidence_smoke: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
