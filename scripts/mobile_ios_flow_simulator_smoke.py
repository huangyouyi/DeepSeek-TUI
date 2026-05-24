#!/usr/bin/env python3
"""Smoke tests for mobile_ios_flow_simulator.py."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "mobile_ios_flow_simulator.py"
PARSER = REPO_ROOT / "scripts" / "mobile_evidence_plan_draft.py"


def run_script(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=REPO_ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def assert_contains(output: str, needle: str) -> None:
    if needle not in output:
        raise AssertionError(f"missing {needle!r} in output:\n{output}")


def test_dry_run_does_not_execute_cargo() -> None:
    completed = run_script("--dry-run")

    assert_contains(completed.stdout, "Linux iOS flow simulator dry-run")
    assert_contains(completed.stdout, "No commands executed.")
    assert_contains(completed.stdout, "session/bootstrap")
    assert_contains(completed.stdout, "pasted output")
    assert_contains(completed.stdout, "runner pairing")
    assert_contains(completed.stdout, "capabilities")
    assert_contains(completed.stdout, "diagnose")
    assert_contains(completed.stdout, "high-risk approval")
    assert_contains(completed.stdout, "command lease")
    assert_contains(completed.stdout, "audit redaction")
    assert "test result:" not in completed.stdout


def test_json_dry_run_is_parseable_summary() -> None:
    completed = run_script("--dry-run", "--json")
    summary = json.loads(completed.stdout)

    assert summary["status"] == "dry_run"
    assert isinstance(summary["steps"], list)
    assert isinstance(summary["commands"], list)
    assert summary["commands"]
    assert all(command["status"] == "planned" for command in summary["commands"])
    assert any(step["id"] == "audit-redaction" for step in summary["steps"])


def test_command_plan_names_key_targeted_cargo_tests() -> None:
    completed = run_script("--dry-run", "--json")
    summary = json.loads(completed.stdout)
    planned = "\n".join(" ".join(command["argv"]) for command in summary["commands"])

    assert_contains(planned, "ios_facade_exposes_minimal_bootstrap_flow")
    assert_contains(planned, "mobile_core_client_exercises_live_runner_socket_with_bearer_auth")
    assert_contains(planned, "mobile_core_client_reads_redacted_command_lease_lifecycle_from_runner_audit")


def test_writes_evidence_bundle_to_requested_output_root() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        output_root = Path(tmpdir) / "evidence-root"
        completed = run_script(
            "--write-evidence",
            "--output-root",
            str(output_root),
            "--host",
            "linux-loopback",
            "--date",
            "2026-05-24",
        )
        bundle = output_root / "2026-05-24-linux-loopback"

        assert_contains(completed.stdout, str(bundle))
        for relative in (
            "README.md",
            "environment.md",
            "commands.log",
            "results.md",
            "evidence-log.md",
            "logs/.gitkeep",
            "screenshots/.gitkeep",
            "failures/.gitkeep",
        ):
            path = bundle / relative
            if not path.exists():
                raise AssertionError(f"missing evidence bundle path: {path}")

        results = (bundle / "results.md").read_text(encoding="utf-8")
        evidence_log = (bundle / "evidence-log.md").read_text(encoding="utf-8")
        commands = (bundle / "commands.log").read_text(encoding="utf-8")

    assert_contains(results, "- Result summary:")
    assert_contains(evidence_log, "- Result summary:")
    assert_contains(results, "| Linux iOS simulator loopback | Planned | LF-E5 / LF-E6 | Run with `--use-loopback-runner --json` and attach output | No |")
    assert_contains(evidence_log, "# Evidence Log: Linux Loopback")
    assert_contains(commands, "python3 scripts/mobile_ios_flow_simulator.py --dry-run")
    assert_contains(commands, "python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json")


def test_parser_generates_draft_from_simulator_bundle() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        output_root = Path(tmpdir) / "bundles"
        run_script(
            "--write-evidence",
            "--output-root",
            str(output_root),
            "--host",
            "linux-loopback",
            "--date",
            "2026-05-24",
        )
        bundle = output_root / "2026-05-24-linux-loopback"
        completed = subprocess.run(
            [sys.executable, str(PARSER), str(bundle)],
            cwd=REPO_ROOT,
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    assert_contains(completed.stdout, "# Mobile Evidence Plan Update Draft")
    assert_contains(completed.stdout, "Linux Loopback")
    assert_contains(completed.stdout, "LF-E5 / LF-E6")
    assert_contains(completed.stdout, "Linux iOS simulator loopback: Run with `--use-loopback-runner --json` and attach output")


def test_dry_run_with_evidence_flags_does_not_write_files() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        output_root = Path(tmpdir) / "dry-run-root"
        completed = run_script(
            "--dry-run",
            "--write-evidence",
            "--output-root",
            str(output_root),
            "--host",
            "linux-loopback",
            "--date",
            "2026-05-24",
        )

        if output_root.exists():
            raise AssertionError(f"dry-run unexpectedly wrote files under {output_root}")

    assert_contains(completed.stdout, "No files written.")


def test_no_args_exits_nonzero_with_usage() -> None:
    completed = run_script(check=False)

    if completed.returncode == 0:
        raise AssertionError("expected no-arg invocation to exit nonzero")
    assert_contains(completed.stderr, "usage:")


def main() -> int:
    tests = [
        test_dry_run_does_not_execute_cargo,
        test_json_dry_run_is_parseable_summary,
        test_command_plan_names_key_targeted_cargo_tests,
        test_writes_evidence_bundle_to_requested_output_root,
        test_parser_generates_draft_from_simulator_bundle,
        test_dry_run_with_evidence_flags_does_not_write_files,
        test_no_args_exits_nonzero_with_usage,
    ]
    for test in tests:
        test()
        print(f"ok - {test.__name__}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
