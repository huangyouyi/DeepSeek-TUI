#!/usr/bin/env python3
"""Capture Linux/LAN mobile Web SSH simulator evidence."""

from __future__ import annotations

import argparse
import json
import platform
import re
import subprocess
import sys
from dataclasses import dataclass
from datetime import date
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SERVER = "http://127.0.0.1:8788"
DEFAULT_OUTPUT_ROOT = REPO_ROOT / "validation" / "mobile-web-ssh"
BOUNDARY = (
    "Linux/LAN Web simulator evidence only; not iOS, macOS, Windows, "
    "or real mobile-platform evidence."
)
EVIDENCE_FILES = (
    "README.md",
    "evidence-log.md",
    "environment.md",
    "commands.log",
    "results.md",
    "logs/.gitkeep",
    "screenshots/.gitkeep",
    "failures/.gitkeep",
)


@dataclass(frozen=True)
class CommandResult:
    name: str
    command: list[str]
    returncode: int
    stdout: str
    stderr: str


def host_slug(host: str) -> str:
    slug = re.sub(r"[^a-z0-9._-]+", "-", host.strip().lower())
    slug = re.sub(r"-+", "-", slug).strip("-._")
    return slug or "host"


def destination_for(args: argparse.Namespace) -> Path:
    return args.output_root / f"{args.date}-mobile-web-ssh-{host_slug(args.host)}"


def planned_paths(destination: Path) -> list[Path]:
    return [destination / relative for relative in EVIDENCE_FILES]


def planned_commands(args: argparse.Namespace) -> list[list[str]]:
    smoke = [
        sys.executable,
        str(REPO_ROOT / "scripts" / "mobile_web_ssh_smoke.py"),
        "--server",
        args.server,
        "--json",
    ]
    flow = [
        sys.executable,
        str(REPO_ROOT / "scripts" / "mobile_web_ssh_flow_simulator.py"),
        "--server",
        args.server,
        "--json",
    ]
    if args.auto_approve:
        flow.append("--auto-approve")
    if args.access_token:
        smoke.extend(["--access-token", args.access_token])
        flow.extend(["--access-token", args.access_token])
    return [smoke, flow]


def relative_command(command: list[str]) -> list[str]:
    display: list[str] = []
    redact_next = False
    for item in command:
        if redact_next:
            display.append("<redacted>")
            redact_next = False
            continue
        if item == "--access-token":
            display.append(item)
            redact_next = True
            continue
        path = Path(item)
        try:
            display.append(str(path.relative_to(REPO_ROOT)))
        except ValueError:
            display.append(item)
    return display


def ensure_linux() -> None:
    if sys.platform != "linux":
        raise SystemExit(f"this helper is Linux-only. {BOUNDARY}")


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def create_skeleton(args: argparse.Namespace, destination: Path) -> None:
    if destination.exists() and any(destination.iterdir()):
        raise SystemExit(f"destination already exists and is not empty: {destination}")

    for path in planned_paths(destination):
        path.parent.mkdir(parents=True, exist_ok=True)

    write_text(
        destination / "README.md",
        "\n".join(
            [
                "# Mobile Web SSH Simulator Evidence",
                "",
                BOUNDARY,
                "",
                f"- Date: {args.date}",
                f"- Host label: {args.host}",
                f"- Server: {args.server}",
                "",
                "This bundle captures Linux/LAN Web simulator command evidence only.",
                "",
            ]
        ),
    )
    write_text(
        destination / "evidence-log.md",
        "\n".join(
            [
                "# Evidence Log",
                "",
                f"- Created: {args.date}",
                f"- Boundary: {BOUNDARY}",
                "",
            ]
        ),
    )
    write_text(
        destination / "environment.md",
        "\n".join(
            [
                "# Environment",
                "",
                f"- Boundary: {BOUNDARY}",
                f"- Platform: {platform.platform()}",
                f"- Python: {sys.version.split()[0]}",
                f"- Repository: {REPO_ROOT}",
                f"- Server: {args.server}",
                f"- Host label: {args.host}",
                "",
            ]
        ),
    )
    write_text(destination / "commands.log", "")
    write_text(
        destination / "results.md",
        "\n".join(["# Results", "", f"Boundary: {BOUNDARY}", ""]),
    )
    for keep in ("logs/.gitkeep", "screenshots/.gitkeep", "failures/.gitkeep"):
        write_text(destination / keep, "")


def run_command(name: str, command: list[str]) -> CommandResult:
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return CommandResult(
        name=name,
        command=relative_command(command),
        returncode=completed.returncode,
        stdout=completed.stdout,
        stderr=completed.stderr,
    )


def append_command_log(destination: Path, results: list[CommandResult]) -> None:
    lines: list[str] = []
    for result in results:
        lines.extend(
            [
                f"$ {' '.join(result.command)}",
                f"exit_code={result.returncode}",
                "[stdout]",
                result.stdout.rstrip(),
                "[stderr]",
                result.stderr.rstrip(),
                "",
            ]
        )
    write_text(destination / "commands.log", "\n".join(lines))


def append_results(destination: Path, results: list[CommandResult]) -> None:
    lines = ["# Results", "", f"Boundary: {BOUNDARY}", ""]
    for result in results:
        status = "PASS" if result.returncode == 0 else "FAIL"
        lines.append(f"- {result.name}: {status} (exit {result.returncode})")
    lines.append("")
    write_text(destination / "results.md", "\n".join(lines))


def dry_run(args: argparse.Namespace, destination: Path) -> int:
    paths = [str(path) for path in planned_paths(destination)]
    commands = [relative_command(command) for command in planned_commands(args)]
    if args.json:
        print(
            json.dumps(
                {
                    "status": "dry-run",
                    "boundary": BOUNDARY,
                    "destination": str(destination),
                    "planned_paths": paths,
                    "planned_commands": commands,
                },
                sort_keys=True,
            )
        )
    else:
        print(BOUNDARY)
        print(f"Destination: {destination}")
        print("Planned paths:")
        for path in paths:
            print(f"- {path}")
        print("Planned commands:")
        for command in commands:
            print(f"- {' '.join(command)}")
        print("Dry-run only; no files written.")
    return 0


def live_run(args: argparse.Namespace, destination: Path) -> int:
    create_skeleton(args, destination)
    command_specs = [
        ("mobile_web_ssh_smoke", planned_commands(args)[0]),
        ("mobile_web_ssh_flow_simulator", planned_commands(args)[1]),
    ]
    results = [run_command(name, command) for name, command in command_specs]
    append_command_log(destination, results)
    append_results(destination, results)
    failed = [result for result in results if result.returncode != 0]
    status = "error" if failed else "ok"
    summary = {
        "status": status,
        "boundary": BOUNDARY,
        "destination": str(destination),
        "results": [
            {
                "name": result.name,
                "command": result.command,
                "returncode": result.returncode,
            }
            for result in results
        ],
    }
    if args.json:
        print(json.dumps(summary, sort_keys=True))
    else:
        print(BOUNDARY)
        print(f"Destination: {destination}")
        for result in results:
            label = "ok" if result.returncode == 0 else "failed"
            print(f"{result.name}: {label} (exit {result.returncode})")
    return 1 if failed else 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture Linux/LAN Web SSH simulator evidence."
    )
    parser.add_argument("--server", default=DEFAULT_SERVER, help="base simulator server URL")
    parser.add_argument("--host", default="host", help="host label used in the evidence folder name")
    parser.add_argument("--date", default=date.today().isoformat(), help="evidence date, YYYY-MM-DD")
    parser.add_argument(
        "--output-root",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help="root directory for generated evidence folders",
    )
    parser.add_argument(
        "--auto-approve",
        action="store_true",
        help="pass --auto-approve to the flow simulator command",
    )
    parser.add_argument(
        "--access-token",
        help="optional mobile web access token passed to child simulator commands; redacted in logs",
    )
    parser.add_argument("--json", action="store_true", help="print machine-readable summary JSON")
    parser.add_argument("--dry-run", action="store_true", help="print planned paths and commands only")
    return parser


def main(argv: list[str] | None = None) -> int:
    ensure_linux()
    parser = build_parser()
    args = parser.parse_args(argv)
    destination = destination_for(args)
    if args.dry_run:
        return dry_run(args, destination)
    return live_run(args, destination)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
