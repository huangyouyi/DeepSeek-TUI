#!/usr/bin/env python3
"""Create a LAN runner evidence bundle without starting network services."""

from __future__ import annotations

import argparse
import re
import shutil
import sys
from datetime import date
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TEMPLATE_DIR = (
    REPO_ROOT / "docs" / "mobile-evidence-bundles" / "templates" / "lan-runner"
)
DEFAULT_OUTPUT_ROOT = REPO_ROOT / "validation" / "mobile"

REQUIRED_TEMPLATE_PATHS = (
    "README.md",
    "evidence-log.md",
    "environment.md",
    "commands.log",
    "results.md",
    "logs/.gitkeep",
    "screenshots/.gitkeep",
    "failures/.gitkeep",
)


def host_slug(host: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", host.strip().lower()).strip("-")
    return slug or "host"


def validate_template() -> None:
    missing = [
        relative
        for relative in REQUIRED_TEMPLATE_PATHS
        if not (TEMPLATE_DIR / relative).exists()
    ]
    if missing:
        joined = "\n  - ".join(
            str((TEMPLATE_DIR / relative).relative_to(REPO_ROOT))
            for relative in missing
        )
        raise SystemExit(f"missing LAN runner template paths:\n  - {joined}")


def planned_files() -> list[Path]:
    validate_template()
    return sorted(path for path in TEMPLATE_DIR.rglob("*") if path.is_file())


def command_suggestions() -> str:
    return """# LAN runner loopback validation command suggestions

# This harness is intentionally passive. It creates evidence files only.
# Do not start a real LAN listener from this harness.

# Local live socket regression coverage. The test binds 127.0.0.1:0 internally.
cargo test -p kai-runner --test runner_mobile_core_live_e2e

# Linux iOS simulator evidence-loop preparation. These commands may become
# runnable once scripts/mobile_ios_flow_simulator.py is present in this branch.
python3 scripts/mobile_ios_flow_simulator.py --dry-run
python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json

# Focused loopback checks from the same live e2e test file.
cargo test -p kai-runner --test runner_mobile_core_live_e2e mobile_core_client_exercises_live_runner_socket_with_bearer_auth -- --nocapture
cargo test -p kai-runner --test runner_mobile_core_live_e2e live_runner_socket_rejects_mobile_core_client_without_bearer_auth -- --nocapture

# If a maintainer has already started a local runner manually, capture HTTP
# transcripts against loopback only. Replace <port> with that manual process port.
curl -i http://127.0.0.1:<port>/health
curl -i -H 'Authorization: Bearer <redacted-token>' http://127.0.0.1:<port>/capabilities
curl -i http://127.0.0.1:<port>/capabilities
"""


def results_notes() -> str:
    return """

## Loopback validation command suggestions

This bundle was created by `scripts/mobile_lan_runner_evidence.py`. The harness
does not start a runner or open a LAN port. Use `commands.log` as the suggested
local loopback transcript plan, preferring `127.0.0.1` and the existing live e2e
test before any real device or LAN validation.

Suggested commands:

```bash
cargo test -p kai-runner --test runner_mobile_core_live_e2e
python3 scripts/mobile_ios_flow_simulator.py --dry-run
python3 scripts/mobile_ios_flow_simulator.py --use-loopback-runner --json
curl -i http://127.0.0.1:<port>/health
curl -i -H 'Authorization: Bearer <redacted-token>' http://127.0.0.1:<port>/capabilities
curl -i http://127.0.0.1:<port>/capabilities
```
"""


def print_plan(destination: Path, files: list[Path], dry_run: bool) -> None:
    if dry_run:
        print("LAN runner evidence bundle dry-run")
    else:
        print("LAN runner evidence bundle plan")
    print(f"Template: {TEMPLATE_DIR.relative_to(REPO_ROOT)}")
    print(f"Destination: {destination}")
    print("Planned paths:")
    for file_path in files:
        print(f"- {destination / file_path.relative_to(TEMPLATE_DIR)}")
    print("Planned generated content:")
    print("- commands.log: local loopback validation command suggestions")
    print("- results.md: loopback validation notes")
    if dry_run:
        print("No files written.")


def create_bundle(destination: Path) -> None:
    if destination.exists() and any(destination.iterdir()):
        raise SystemExit(f"destination already exists and is not empty: {destination}")

    for file_path in planned_files():
        target = destination / file_path.relative_to(TEMPLATE_DIR)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(file_path, target)

    (destination / "commands.log").write_text(command_suggestions(), encoding="utf-8")
    with (destination / "results.md").open("a", encoding="utf-8") as results:
        results.write(results_notes())


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Create a LAN runner evidence bundle from the template without "
            "starting real network services."
        )
    )
    parser.add_argument(
        "--host",
        default="host",
        help="runner host slug source used in the output folder name",
    )
    parser.add_argument(
        "--date",
        default=date.today().isoformat(),
        help="run date for the output folder name, YYYY-MM-DD",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help="root directory for generated LAN runner evidence bundles",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print the bundle plan without writing files",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    destination = args.output_root / f"{args.date}-lan-runner-{host_slug(args.host)}"
    files = planned_files()
    print_plan(destination, files, args.dry_run)
    if args.dry_run:
        return 0

    create_bundle(destination)
    print(f"Created LAN runner evidence bundle: {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
