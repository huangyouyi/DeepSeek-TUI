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
                "--write-plan-draft",
                "--access-token",
                "dev-secret-token",
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
        if payload.get("plan_draft") != str(destination / "plan-update-draft.md"):
            raise AssertionError(payload)
        if destination.exists():
            raise AssertionError(f"dry-run wrote destination: {destination}")

        planned_paths = "\n".join(payload["planned_paths"])
        assert_contains(planned_paths, "README.md")
        assert_contains(planned_paths, "logs/.gitkeep")
        assert_contains(planned_paths, "plan-update-draft.md")
        planned_commands = "\n".join(" ".join(command) for command in payload["planned_commands"])
        assert_contains(planned_commands, "scripts/mobile_web_ssh_smoke.py")
        assert_contains(planned_commands, "scripts/mobile_web_ssh_flow_simulator.py")
        assert_contains(planned_commands, "--auto-approve")
        assert_contains(planned_commands, "--access-token <redacted>")
        if "dev-secret-token" in result.stdout:
            raise AssertionError(f"dry-run leaked access token:\n{result.stdout}")
        assert_contains(payload["boundary"], "Linux/LAN Web simulator evidence only")

        live_root = output_root / "live"
        live_result = run_helper(
            [
                "--server",
                "http://127.0.0.1:9",
                "--host",
                "Failing Lab",
                "--date",
                "2026-05-25",
                "--output-root",
                str(live_root),
                "--auto-approve",
                "--json",
                "--write-plan-draft",
                "--access-token",
                "dev-secret-token",
            ]
        )
        if live_result.returncode != 1:
            raise AssertionError(
                "expected live helper to report child command failure\n"
                f"exit: {live_result.returncode}\n"
                f"stdout:\n{live_result.stdout}\n"
                f"stderr:\n{live_result.stderr}"
            )

        live_payload = json.loads(live_result.stdout)
        live_destination = live_root / "2026-05-25-mobile-web-ssh-failing-lab"
        if live_payload["destination"] != str(live_destination):
            raise AssertionError(live_payload)
        if live_payload.get("plan_draft") != str(live_destination / "plan-update-draft.md"):
            raise AssertionError(live_payload)

        commands_log = (live_destination / "commands.log").read_text(encoding="utf-8")
        assert_contains(commands_log, "--access-token <redacted>")
        if "dev-secret-token" in commands_log:
            raise AssertionError(f"commands log leaked access token:\n{commands_log}")

        results_md = (live_destination / "results.md").read_text(encoding="utf-8")
        assert_contains(results_md, "- mobile_web_ssh_smoke: FAIL (exit 1)")
        assert_contains(results_md, "- mobile_web_ssh_flow_simulator: FAIL (exit 1)")

        draft = (live_destination / "plan-update-draft.md").read_text(encoding="utf-8")
        assert_contains(draft, "# Mobile Evidence Plan Update Draft")
        assert_contains(draft, "Investigate failed commands: mobile_web_ssh_smoke, mobile_web_ssh_flow_simulator")

    print("mobile_web_ssh_evidence_smoke: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
