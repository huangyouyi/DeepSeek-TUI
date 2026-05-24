#!/usr/bin/env python3
"""Run Linux-safe mobile porting validation checks."""

from __future__ import annotations

import argparse
import shlex
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def build_commands() -> list[list[str]]:
    python = "python3"
    return [
        [python, "scripts/mobile_secret_scan_smoke.py"],
        [python, "scripts/mobile_evidence_plan_draft_test.py"],
        [python, "scripts/mobile_evidence_bundle_smoke.py"],
        [python, "scripts/mobile_evidence_bundle.py", "--list"],
        [
            python,
            "scripts/mobile_evidence_bundle.py",
            "--platform",
            "lan-runner",
            "--host",
            "lab-runner",
            "--date",
            "2026-05-24",
            "--dry-run",
        ],
        [python, "scripts/mobile_ios_flow_simulator.py", "--dry-run"],
        [python, "scripts/mobile_ios_flow_simulator_smoke.py"],
        ["sh", "-n", "crates/mobile-agent-core/scripts/uniffi-dry-run"],
        ["sh", "-n", "ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos"],
        ["sh", "-n", "ios/DeepSeekMobileDemo/Scripts/verify-macos"],
        ["crates/mobile-agent-core/scripts/uniffi-dry-run", "--check-plan"],
        ["ios/DeepSeekMobileDemo/Scripts/generate-uniffi-macos", "--check-plan"],
    ]


def format_command(command: list[str]) -> str:
    return " ".join(shlex.quote(part) for part in command)


def run_commands(commands: list[list[str]], dry_run: bool) -> int:
    for command in commands:
        print(f"$ {format_command(command)}", flush=True)
        if dry_run:
            continue

        completed = subprocess.run(command, cwd=REPO_ROOT)
        if completed.returncode != 0:
            return completed.returncode
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run Linux-safe mobile validation checks without cargo, Rust edits, or iOS edits."
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print validation commands without executing them",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return run_commands(build_commands(), args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
