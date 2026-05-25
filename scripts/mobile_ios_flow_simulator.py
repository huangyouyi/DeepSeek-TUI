#!/usr/bin/env python3
"""Linux-only iOS execution flow simulator.

This command does not claim real iOS execution. It either prints the Linux
simulation plan or delegates loopback verification to existing targeted cargo
tests that own the actual mobile-core and kai-runner contracts.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
TEMPLATE_RELATIVE_PATHS = (
    "README.md",
    "environment.md",
    "commands.log",
    "results.md",
    "evidence-log.md",
    "logs/.gitkeep",
    "screenshots/.gitkeep",
    "failures/.gitkeep",
)
SUMMARY_HEADER = (
    "Area",
    "Pass/Fail/Blocker",
    "Related milestone",
    "Next issue/task",
    "`mobile-porting-plan.md` updated?",
)


@dataclass(frozen=True)
class Step:
    id: str
    title: str
    verifies: str


@dataclass(frozen=True)
class PlannedCommand:
    id: str
    covers: tuple[str, ...]
    argv: tuple[str, ...]


STEPS: tuple[Step, ...] = (
    Step(
        "session-bootstrap",
        "Create a mobile session through the Swift-facing JSON facade contract",
        "mobile-core creates the session and exposes bootstrap commands to Swift-friendly APIs",
    ),
    Step(
        "bootstrap-command-card",
        "Generate Homebrew/Xcode diagnostic bootstrap command-card output",
        "bootstrap flow contains diagnostics before installer or repair steps",
    ),
    Step(
        "pasted-output",
        "Import pasted terminal output and record a mobile audit/event entry",
        "bootstrap output submission advances the mobile session event stream",
    ),
    Step(
        "runner-pairing",
        "Pair with the Linux loopback kai-runner through the existing live test",
        "runner socket uses bearer authentication on 127.0.0.1 without custom services",
    ),
    Step(
        "capabilities",
        "Read runner capabilities from the authenticated loopback runner",
        "mobile-core discovers runner capability mode and tool list",
    ),
    Step(
        "diagnose",
        "Execute low-risk remote.diagnose.system through the loopback runner",
        "diagnose returns structured system and capability output",
    ),
    Step(
        "high-risk-approval",
        "Request high-risk shell approval metadata before execution",
        "shell execution is represented as an approval-gated command lease",
    ),
    Step(
        "command-lease",
        "Verify command lease accepted, consumed, and replay-rejected lifecycle",
        "runner audit records command lease lifecycle without exposing secrets",
    ),
    Step(
        "audit-redaction",
        "Read recent audit output and verify iOS timeline-safe redaction",
        "bearer tokens, nonces, lease ids, idempotency keys, commands, and env secrets are absent",
    ),
)


COMMANDS: tuple[PlannedCommand, ...] = (
    PlannedCommand(
        "mobile-core-bootstrap-facade",
        ("session-bootstrap", "bootstrap-command-card", "pasted-output"),
        (
            "cargo",
            "test",
            "-p",
            "deepseek-mobile-agent-core",
            "--test",
            "bootstrap_session",
            "ios_facade_exposes_minimal_bootstrap_flow",
        ),
    ),
    PlannedCommand(
        "mobile-core-bootstrap-install-json-facade",
        ("session-bootstrap", "bootstrap-command-card"),
        (
            "cargo",
            "test",
            "-p",
            "deepseek-mobile-agent-core",
            "--test",
            "uniffi_api",
            "bootstrap_install_steps_json_returns_swift_friendly_install_steps",
        ),
    ),
    PlannedCommand(
        "kai-runner-bearer-capabilities-diagnose-live",
        ("runner-pairing", "capabilities", "diagnose", "audit-redaction"),
        (
            "cargo",
            "test",
            "-p",
            "kai-runner",
            "--test",
            "runner_mobile_core_live_e2e",
            "mobile_core_client_exercises_live_runner_socket_with_bearer_auth",
            "--",
            "--nocapture",
        ),
    ),
    PlannedCommand(
        "kai-runner-command-lease-audit-lifecycle-live",
        ("high-risk-approval", "command-lease", "audit-redaction"),
        (
            "cargo",
            "test",
            "-p",
            "kai-runner",
            "--test",
            "runner_mobile_core_live_e2e",
            "mobile_core_client_reads_redacted_command_lease_lifecycle_from_runner_audit",
            "--",
            "--nocapture",
        ),
    ),
)


def step_dict(step: Step) -> dict[str, str]:
    return {
        "id": step.id,
        "title": step.title,
        "verifies": step.verifies,
    }


def command_dict(
    command: PlannedCommand,
    status: str,
    exit_code: int | None = None,
    stdout_tail: str | None = None,
    stderr_tail: str | None = None,
) -> dict[str, Any]:
    item: dict[str, Any] = {
        "id": command.id,
        "covers": list(command.covers),
        "argv": list(command.argv),
        "status": status,
    }
    if exit_code is not None:
        item["exit_code"] = exit_code
    if stdout_tail:
        item["stdout_tail"] = stdout_tail
    if stderr_tail:
        item["stderr_tail"] = stderr_tail
    return item


def print_text_plan() -> None:
    print("Linux iOS flow simulator dry-run")
    print(
        "Coverage: session/bootstrap, pasted output, runner pairing, "
        "capabilities, diagnose, high-risk approval, command lease, audit redaction"
    )
    print()
    print("Planned LF-D steps:")
    for index, step in enumerate(STEPS, start=1):
        print(f"{index}. {step.title}")
        print(f"   id: {step.id}")
        print(f"   verifies: {step.verifies}")
    print()
    print("Planned verification commands:")
    for command in COMMANDS:
        print(f"- {command.id}: {' '.join(command.argv)}")
    print()
    print("No commands executed.")
    print("No files written.")


def dry_run_summary() -> dict[str, Any]:
    return {
        "status": "dry_run",
        "steps": [step_dict(step) for step in STEPS],
        "commands": [command_dict(command, "planned") for command in COMMANDS],
    }


def host_title(host: str) -> str:
    return " ".join(part for part in host.replace("_", "-").split("-") if part).title()


def validate_host_slug(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", value):
        raise argparse.ArgumentTypeError(
            "host must be a filesystem-safe slug containing letters, numbers, dots, underscores, or hyphens"
        )
    return value


def validate_iso_date(value: str) -> str:
    try:
        date.fromisoformat(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("date must use YYYY-MM-DD") from error
    return value


def command_status(summary: dict[str, Any], command_id: str) -> str:
    for command in summary["commands"]:
        if command["id"] == command_id:
            return command["status"]
    return "planned"


def evidence_result_status(summary: dict[str, Any]) -> str:
    if summary["status"] == "passed":
        return "Pass"
    if summary["status"] == "failed":
        return "Fail"
    return "Planned"


def summary_rows(summary: dict[str, Any]) -> list[tuple[str, str, str, str, str]]:
    status = evidence_result_status(summary)
    if status == "Pass":
        next_task = "None"
        plan_updated = "Yes"
    elif status == "Fail":
        next_task = "Inspect failing command output and attach logs under failures/"
        plan_updated = "No"
    else:
        next_task = "Run with `--use-loopback-runner --json` and attach output"
        plan_updated = "No"

    return [
        (
            "Linux iOS simulator loopback",
            status,
            "LF-E5 / LF-E6",
            next_task,
            plan_updated,
        )
    ]


def render_summary_table(summary: dict[str, Any], indent: str = "") -> str:
    lines = [
        f"{indent}| {' | '.join(SUMMARY_HEADER)} |",
        f"{indent}|---|---|---|---|---|",
    ]
    for row in summary_rows(summary):
        lines.append(f"{indent}| {' | '.join(row)} |")
    return "\n".join(lines)


def git_output(*args: str) -> str:
    completed = subprocess.run(
        ("git", *args),
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if completed.returncode != 0:
        return "unknown"
    return completed.stdout.strip() or "unknown"


def worktree_status() -> str:
    status = git_output("status", "--short")
    if status == "unknown":
        return "unknown"
    return "dirty" if status else "clean"


def ensure_bundle_structure(bundle: Path) -> None:
    for relative in TEMPLATE_RELATIVE_PATHS:
        path = bundle / relative
        if path.suffix:
            path.parent.mkdir(parents=True, exist_ok=True)
            if relative.endswith(".gitkeep"):
                path.touch()
        else:
            path.mkdir(parents=True, exist_ok=True)


def render_readme() -> str:
    return """# LAN Runner Evidence Bundle

Use this bundle for Linux loopback iOS simulator evidence that mirrors the LAN
runner evidence bundle structure in `docs/mobile-evidence-bundles/templates/lan-runner`.

Expected paths:

- `evidence-log.md`: checklist index and result summary.
- `environment.md`: host OS, repo SHA, branch, and worktree status.
- `commands.log`: simulator dry-run and loopback command transcript.
- `results.md`: observed pass/fail/blocker state and next tasks.
- `logs/`: optional runner or cargo logs.
- `screenshots/`: optional simulator screenshots or recordings.
- `failures/`: optional failure transcripts and redacted diagnostics.

Do not store production secrets or customer data in this bundle.
"""


def render_environment(host: str, bundle_date: str) -> str:
    return f"""# Environment

- Runner host: {host}
- Hardware/architecture: unknown
- OS version: unknown
- Runner version/artifact: Linux iOS flow simulator
- Bind address/port: loopback runner cargo tests
- Firewall/network profile: loopback only
- Client device/simulator: Linux simulation of iOS flow
- Repo SHA: {git_output("rev-parse", "HEAD")}
- Branch: {git_output("branch", "--show-current")}
- Worktree status: {worktree_status()}
- Date: {bundle_date}
- Notes: Generated by `scripts/mobile_ios_flow_simulator.py --write-evidence`.
"""


def render_commands_log(summary: dict[str, Any]) -> str:
    lines = [
        "# Commands",
        "",
        "$ python3 scripts/mobile_ios_flow_simulator.py --dry-run",
        "status: planned",
        "",
        "$ python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json",
    ]
    for command in summary["commands"]:
        exit_code = command.get("exit_code")
        exit_text = "" if exit_code is None else f" exit={exit_code}"
        lines.append(f"{command['id']}: {command['status']}{exit_text}")
        if command.get("stdout_tail"):
            lines.append("stdout_tail:")
            lines.append(command["stdout_tail"])
        if command.get("stderr_tail"):
            lines.append("stderr_tail:")
            lines.append(command["stderr_tail"])
    return "\n".join(lines) + "\n"


def render_results(summary: dict[str, Any]) -> str:
    command_lines = [
        f"| {command['id']} | {command['status']} | {command_status(summary, command['id'])} |"
        for command in summary["commands"]
    ]
    return "\n".join(
        [
            "# Results",
            "",
            "- Result summary:",
            render_summary_table(summary, indent="  "),
            "",
            "| Check | Pass/Fail/Blocker | Notes/next action |",
            "|---|---|---|",
            *command_lines,
            "",
        ]
    )


def render_evidence_log(summary: dict[str, Any], host: str, bundle_date: str) -> str:
    title = host_title(host)
    return "\n".join(
        [
            f"# Evidence Log: {title}",
            "",
            f"- Date: {bundle_date}",
            f"- Device/host: {host}",
            "- OS/runtime: Linux loopback simulator",
            f"- Commit/worktree: {git_output('rev-parse', 'HEAD')} / {worktree_status()}",
            "- Checklist scope: LF-E5 / LF-E6 simulator evidence bundle generation and parser draft",
            "- Commands/actions:",
            "  - Runner startup: existing targeted cargo tests when `--use-loopback-runner` is used",
            "  - `/health`: covered by loopback runner test command plan",
            "  - Pairing/capabilities/diagnose: covered by loopback runner test command plan",
            f"- Result: {summary['status']}",
            "- Result summary:",
            render_summary_table(summary, indent="  "),
            "- Log paths: commands.log",
            "- Screenshot/recording paths: screenshots/",
            "- Blockers: None recorded by simulator" if summary["status"] != "failed" else "- Blockers: failing command output in commands.log",
            "- Next steps: Review generated draft from `scripts/mobile_evidence_plan_draft.py`",
            "",
        ]
    )


def write_evidence_bundle(output_root: Path, host: str, bundle_date: str, summary: dict[str, Any]) -> Path:
    bundle = output_root / f"{bundle_date}-{host}"
    ensure_bundle_structure(bundle)
    (bundle / "README.md").write_text(render_readme(), encoding="utf-8")
    (bundle / "environment.md").write_text(render_environment(host, bundle_date), encoding="utf-8")
    (bundle / "commands.log").write_text(render_commands_log(summary), encoding="utf-8")
    (bundle / "results.md").write_text(render_results(summary), encoding="utf-8")
    (bundle / "evidence-log.md").write_text(
        render_evidence_log(summary, host, bundle_date),
        encoding="utf-8",
    )
    return bundle


def run_loopback(json_output: bool) -> tuple[int, dict[str, Any]]:
    results: list[dict[str, Any]] = []
    overall_status = "passed"

    if not json_output:
        print("Linux iOS flow simulator loopback verification")
        print("Using existing targeted cargo tests; no custom network service is started.")
        print()

    for command in COMMANDS:
        if not json_output:
            print(f"Running {command.id}: {' '.join(command.argv)}")
            sys.stdout.flush()
        completed = subprocess.run(
            command.argv,
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE if json_output else None,
            stderr=subprocess.PIPE if json_output else None,
        )
        status = "passed" if completed.returncode == 0 else "failed"
        if completed.returncode != 0:
            overall_status = "failed"
        stdout_tail = None
        stderr_tail = None
        if json_output and completed.returncode != 0:
            stdout_tail = (completed.stdout or "")[-4000:]
            stderr_tail = (completed.stderr or "")[-4000:]
        results.append(
            command_dict(
                command,
                status,
                completed.returncode,
                stdout_tail=stdout_tail,
                stderr_tail=stderr_tail,
            )
        )
        if not json_output:
            print(f"{command.id}: {status} (exit {completed.returncode})")
            print()
        if completed.returncode != 0:
            break

    summary = {
        "status": overall_status,
        "steps": [step_dict(step) for step in STEPS],
        "commands": results
        + [
            command_dict(command, "skipped")
            for command in COMMANDS[len(results) :]
        ],
    }
    return (0 if overall_status == "passed" else 1, summary)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Simulate the iOS phone-to-runner execution flow on Linux by "
            "printing LF-D steps or running existing loopback cargo tests."
        )
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--dry-run",
        action="store_true",
        help="print planned LF-D steps and cargo tests without executing commands or writing files",
    )
    mode.add_argument(
        "--use-loopback-runner",
        action="store_true",
        help="run existing targeted cargo tests that validate the Linux loopback runner contract",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit summary JSON containing steps, commands, and status",
    )
    parser.add_argument(
        "--write-evidence",
        action="store_true",
        help="write a lan-runner-compatible evidence bundle from the simulator summary",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        help="directory that will receive the dated evidence bundle; required with --write-evidence",
    )
    parser.add_argument(
        "--host",
        type=validate_host_slug,
        default="linux-loopback",
        help="filesystem-safe host slug for the evidence bundle directory",
    )
    parser.add_argument(
        "--date",
        type=validate_iso_date,
        default=date.today().isoformat(),
        help="evidence bundle date in YYYY-MM-DD format",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.write_evidence and args.output_root is None and not args.dry_run:
        parser.error("--output-root is required with --write-evidence")

    if not args.dry_run and not args.use_loopback_runner and not args.write_evidence:
        parser.print_usage(sys.stderr)
        return 2

    if args.dry_run:
        summary = dry_run_summary()
        if args.json:
            print(json.dumps(summary, indent=2, sort_keys=True))
        else:
            print_text_plan()
        return 0

    if args.use_loopback_runner:
        exit_code, summary = run_loopback(args.json)
    else:
        exit_code = 0
        summary = dry_run_summary()

    bundle = None
    if args.write_evidence:
        bundle = write_evidence_bundle(args.output_root, args.host, args.date, summary)

    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
    elif bundle is not None:
        print(f"Evidence bundle written: {bundle}")
    elif exit_code == 0:
        print("Loopback verification passed.")
    else:
        print("Loopback verification failed.", file=sys.stderr)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
