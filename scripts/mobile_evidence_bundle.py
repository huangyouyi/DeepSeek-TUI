#!/usr/bin/env python3
"""Create or inspect mobile real-platform evidence bundle skeletons."""

from __future__ import annotations

import argparse
import shutil
import sys
from datetime import date
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TEMPLATE_ROOT = REPO_ROOT / "docs" / "mobile-evidence-bundles" / "templates"
DEFAULT_OUTPUT_ROOT = REPO_ROOT / "validation" / "mobile"

PLATFORMS = (
    "macos-host",
    "windows-host",
    "ios-simulator",
    "ios-device",
    "lan-runner",
)


def platform_dirs() -> list[Path]:
    return [TEMPLATE_ROOT / platform for platform in PLATFORMS]


def validate_templates() -> None:
    missing: list[str] = []
    for template_dir in platform_dirs():
        for relative in (
            "README.md",
            "evidence-log.md",
            "environment.md",
            "results.md",
            "commands.log",
            "logs/.gitkeep",
            "screenshots/.gitkeep",
            "failures/.gitkeep",
        ):
            path = template_dir / relative
            if not path.exists():
                missing.append(str(path.relative_to(REPO_ROOT)))
    if missing:
        joined = "\n  - ".join(missing)
        raise SystemExit(f"missing template paths:\n  - {joined}")


def list_templates() -> int:
    validate_templates()
    print("Available mobile evidence bundle templates:")
    for template_dir in platform_dirs():
        files = sorted(
            path.relative_to(template_dir)
            for path in template_dir.rglob("*")
            if path.is_file()
        )
        print(f"- {template_dir.name}")
        for relative in files:
            print(f"  - {relative}")
    return 0


def copy_template(platform: str, destination: Path, dry_run: bool) -> int:
    validate_templates()
    source = TEMPLATE_ROOT / platform
    if platform not in PLATFORMS:
        known = ", ".join(PLATFORMS)
        raise SystemExit(f"unknown platform {platform!r}; expected one of: {known}")

    files = sorted(path for path in source.rglob("*") if path.is_file())
    print(f"Template: {source.relative_to(REPO_ROOT)}")
    print(f"Destination: {destination}")
    print("Planned paths:")
    for file_path in files:
        target = destination / file_path.relative_to(source)
        print(f"- {target}")

    if dry_run:
        print("Dry-run only; no files written.")
        return 0

    if destination.exists() and any(destination.iterdir()):
        raise SystemExit(f"destination already exists and is not empty: {destination}")

    for file_path in files:
        target = destination / file_path.relative_to(source)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(file_path, target)
    print("Evidence bundle skeleton created.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Create or inspect mobile validation evidence bundle skeletons."
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="list available platform templates and their tracked paths",
    )
    parser.add_argument(
        "--platform",
        choices=PLATFORMS,
        help="platform template to instantiate",
    )
    parser.add_argument(
        "--host",
        default="host",
        help="host/device slug used in the default output folder name",
    )
    parser.add_argument(
        "--date",
        default=date.today().isoformat(),
        help="run date for the default output folder name, YYYY-MM-DD",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help="root directory for generated bundles",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print planned paths without writing files",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.list:
        return list_templates()

    if not args.platform:
        parser.error("--platform is required unless --list is used")

    host_slug = args.host.strip().replace(" ", "-").lower()
    destination = args.output_root / f"{args.date}-{args.platform}-{host_slug}"
    return copy_template(args.platform, destination, args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
