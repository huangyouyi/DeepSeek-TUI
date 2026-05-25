#!/usr/bin/env python3
"""Smoke tests for mobile_evidence_plan_draft.py."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "mobile_evidence_plan_draft.py"


def run_script(*args: str) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return completed


def assert_contains(output: str, needle: str) -> None:
    if needle not in output:
        raise AssertionError(f"missing {needle!r} in output:\n{output}")


def test_completed_evidence_log() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        bundle = Path(tmpdir) / "validation" / "mobile" / "2026-05-24-ios-sim"
        bundle.mkdir(parents=True)
        (bundle / "evidence-log.md").write_text(
            textwrap.dedent(
                """\
                # Evidence Log: iOS Simulator - iPhone 15

                - Result summary:
                  | Area | Pass/Fail/Blocker | Related milestone | Next issue/task | `mobile-porting-plan.md` updated? |
                  |---|---|---|---|---|
                  | Simulator inventory/build | Pass | S7 / M13 | None | Yes |
                  | Local network rehearsal | Blocker | S7 / M9 | Capture LAN runner firewall logs | No |
                """
            ),
            encoding="utf-8",
        )

        output = run_script(str(bundle)).stdout

    assert_contains(output, "# Mobile Evidence Plan Update Draft")
    assert_contains(output, "| Platform | Pass/Fail/Blocker | Milestone | Next task | Plan updated | Source |")
    assert_contains(
        output,
        f"| iOS Simulator - iPhone 15 | Pass | S7 / M13 | Simulator inventory/build: None | Yes | {bundle / 'evidence-log.md'} |",
    )
    assert_contains(
        output,
        f"| iOS Simulator - iPhone 15 | Blocker | S7 / M9 | Local network rehearsal: Capture LAN runner firewall logs | No | {bundle / 'evidence-log.md'} |",
    )


def test_bundle_directory_discovers_results_md() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        bundle = Path(tmpdir) / "validation" / "mobile" / "2026-05-24-lan-runner"
        bundle.mkdir(parents=True)
        (bundle / "results.md").write_text(
            textwrap.dedent(
                """\
                # Evidence Log: LAN Runner

                - Result summary:
                  | Area | Pass/Fail/Blocker | Related milestone | Next issue/task | `mobile-porting-plan.md` updated? |
                  |---|---|---|---|---|
                  | Diagnose smoke | Pass | S8 / M9 | None | Yes |
                """
            ),
            encoding="utf-8",
        )
        (bundle / "notes.md").write_text(
            "# Notes\n\n| Not | A | Result | Summary | Table |\n",
            encoding="utf-8",
        )

        output = run_script(str(bundle)).stdout

    assert_contains(
        output,
        f"| LAN Runner | Pass | S8 / M9 | Diagnose smoke: None | Yes | {bundle / 'results.md'} |",
    )
    if "notes.md" in output:
        raise AssertionError(f"unexpected non-evidence markdown source in output:\n{output}")


def test_mobile_web_ssh_bundle_pass_result() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        bundle = Path(tmpdir) / "validation" / "mobile-web-ssh" / "2026-05-25-mobile-web-ssh-linux-lab"
        bundle.mkdir(parents=True)
        (bundle / "evidence-log.md").write_text(
            textwrap.dedent(
                """\
                # Evidence Log

                - Boundary: Linux/LAN Web simulator evidence only; not iOS, macOS, Windows, or real mobile-platform evidence.
                """
            ),
            encoding="utf-8",
        )
        (bundle / "results.md").write_text(
            textwrap.dedent(
                """\
                # Results

                Boundary: Linux/LAN Web simulator evidence only; not iOS, macOS, Windows, or real mobile-platform evidence.

                - mobile_web_ssh_smoke: PASS (exit 0)
                - mobile_web_ssh_flow_simulator: PASS (exit 0)
                """
            ),
            encoding="utf-8",
        )
        (bundle / "commands.log").write_text(
            "$ scripts/mobile_web_ssh_smoke.py --json\nexit_code=0\n",
            encoding="utf-8",
        )

        output = run_script(str(bundle)).stdout

    assert_contains(output, "## Linux/LAN Web SSH Simulator")
    assert_contains(
        output,
        "Linux/LAN Web simulator evidence only; not iOS, macOS, Windows, or real mobile-platform evidence.",
    )
    assert_contains(
        output,
        f"| Linux/LAN Web SSH Simulator | Pass | W5 / LF-M9 | Simulator command bundle: None; passed commands: mobile_web_ssh_smoke, mobile_web_ssh_flow_simulator | No | {bundle / 'results.md'} |",
    )


def test_mobile_web_ssh_bundle_fail_result() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        bundle = Path(tmpdir) / "validation" / "mobile-web-ssh" / "2026-05-25-mobile-web-ssh-linux-lab"
        bundle.mkdir(parents=True)
        (bundle / "evidence-log.md").write_text(
            textwrap.dedent(
                """\
                # Evidence Log

                - Boundary: Linux/LAN Web simulator evidence only; not iOS, macOS, Windows, or real mobile-platform evidence.
                """
            ),
            encoding="utf-8",
        )
        (bundle / "results.md").write_text(
            textwrap.dedent(
                """\
                # Results

                Boundary: Linux/LAN Web simulator evidence only; not iOS, macOS, Windows, or real mobile-platform evidence.

                - mobile_web_ssh_smoke: PASS (exit 0)
                - mobile_web_ssh_flow_simulator: FAIL (exit 1)
                """
            ),
            encoding="utf-8",
        )
        (bundle / "commands.log").write_text(
            "$ scripts/mobile_web_ssh_flow_simulator.py --json\nexit_code=1\n",
            encoding="utf-8",
        )

        output = run_script(str(bundle)).stdout

    assert_contains(output, "## Linux/LAN Web SSH Simulator")
    assert_contains(
        output,
        f"| Linux/LAN Web SSH Simulator | Fail | W5 / LF-M9 | Simulator command bundle: Investigate failed commands: mobile_web_ssh_flow_simulator | No | {bundle / 'results.md'} |",
    )


def test_multiple_files_keep_source_for_each_row() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir)
        ios_log = root / "ios-evidence-log.md"
        android_results = root / "android-results.md"
        ios_log.write_text(
            textwrap.dedent(
                """\
                # Evidence Log: iOS Device

                - Result summary:
                  | Area | Pass/Fail/Blocker | Related milestone | Next issue/task | `mobile-porting-plan.md` updated? |
                  |---|---|---|---|---|
                  | Device launch | Pass | S7 / M13 | None | Yes |
                """
            ),
            encoding="utf-8",
        )
        android_results.write_text(
            textwrap.dedent(
                """\
                # Evidence Log: Android Emulator

                - Result summary:
                  | Area | Pass/Fail/Blocker | Related milestone | Next issue/task | `mobile-porting-plan.md` updated? |
                  |---|---|---|---|---|
                  | Emulator launch | Blocker | S9 / M16 | Capture adb logs | No |
                """
            ),
            encoding="utf-8",
        )

        output = run_script(str(ios_log), str(android_results)).stdout

    assert_contains(
        output,
        f"| iOS Device | Pass | S7 / M13 | Device launch: None | Yes | {ios_log} |",
    )
    assert_contains(
        output,
        f"| Android Emulator | Blocker | S9 / M16 | Emulator launch: Capture adb logs | No | {android_results} |",
    )


def test_empty_results_names_checked_sources() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        bundle = Path(tmpdir) / "validation" / "mobile" / "2026-05-24-empty"
        bundle.mkdir(parents=True)
        (bundle / "evidence-log.md").write_text("# Evidence Log: Empty\n", encoding="utf-8")
        (bundle / "results.md").write_text("# Results\n", encoding="utf-8")

        completed = run_script(str(bundle))

    if completed.returncode != 1:
        raise AssertionError(f"expected exit 1, got {completed.returncode}")
    assert_contains(completed.stderr, "error: no result summary rows found in")
    assert_contains(completed.stderr, str(bundle / "evidence-log.md"))
    assert_contains(completed.stderr, str(bundle / "results.md"))


def test_checklist_templates_parse() -> None:
    output = run_script("docs/mobile-validation-checklists.md").stdout

    assert_contains(output, "| macOS Host |  | S6 / M13 | macOS environment capture:  |  | docs/mobile-validation-checklists.md |")
    assert_contains(output, "| Windows Host |  | S9 / M16 | PowerShell runner behavior:  |  | docs/mobile-validation-checklists.md |")
    assert_contains(output, "| LAN Runner |  | S8 / M9 | Capabilities and diagnose:  |  | docs/mobile-validation-checklists.md |")


def main() -> int:
    tests = [
        test_completed_evidence_log,
        test_bundle_directory_discovers_results_md,
        test_mobile_web_ssh_bundle_pass_result,
        test_mobile_web_ssh_bundle_fail_result,
        test_multiple_files_keep_source_for_each_row,
        test_empty_results_names_checked_sources,
        test_checklist_templates_parse,
    ]
    for test in tests:
        test()
        print(f"ok - {test.__name__}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
