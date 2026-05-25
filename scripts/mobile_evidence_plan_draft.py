#!/usr/bin/env python3
"""Generate a mobile-porting plan update draft from evidence summary tables.

The script intentionally writes only to stdout. It does not modify
docs/mobile-porting-plan.md.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


SUMMARY_HEADER = [
    "area",
    "pass/fail/blocker",
    "related milestone",
    "next issue/task",
    "`mobile-porting-plan.md` updated?",
]
EVIDENCE_BUNDLE_FILENAMES = {"evidence-log.md", "results.md"}
MOBILE_WEB_SSH_PLATFORM = "Linux/LAN Web SSH Simulator"
MOBILE_WEB_SSH_MILESTONE = "W5 / LF-M9"
MOBILE_WEB_SSH_BOUNDARY = (
    "Linux/LAN Web simulator evidence only; not iOS, macOS, Windows, "
    "or real mobile-platform evidence."
)
MOBILE_WEB_SSH_RESULT_RE = re.compile(
    r"^-\s+(mobile_web_ssh[\w_]*):\s+(PASS|FAIL)\s+\(exit\s+(-?\d+)\)\s*$",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class SummaryRow:
    platform: str
    status: str
    milestone: str
    area: str
    next_task: str
    plan_updated: str
    source: Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Parse mobile evidence result summary tables and print a "
            "mobile-porting-plan.md update draft."
        )
    )
    parser.add_argument(
        "paths",
        nargs="+",
        help=(
            "Evidence log markdown files, evidence bundle directories, or "
            "docs/mobile-validation-checklists.md."
        ),
    )
    parser.add_argument(
        "--source",
        action="store_true",
        help="Deprecated compatibility flag; source file paths are always included.",
    )
    return parser.parse_args()


def evidence_files(paths: list[str]) -> list[Path]:
    files: list[Path] = []
    for raw_path in paths:
        path = Path(raw_path)
        if path.is_dir():
            files.extend(
                sorted(
                    p
                    for p in path.rglob("*.md")
                    if p.name in EVIDENCE_BUNDLE_FILENAMES
                )
            )
        elif path.is_file():
            files.append(path)
        else:
            raise FileNotFoundError(f"No such file or directory: {path}")
    return dedupe_paths(files)


def dedupe_paths(paths: list[Path]) -> list[Path]:
    seen: set[Path] = set()
    unique: list[Path] = []
    for path in paths:
        resolved = path.resolve()
        if resolved not in seen:
            seen.add(resolved)
            unique.append(path)
    return unique


def parse_file(path: Path) -> list[SummaryRow]:
    lines = path.read_text(encoding="utf-8").splitlines()
    mobile_web_ssh_rows = parse_mobile_web_ssh_results(path, lines)
    if mobile_web_ssh_rows:
        return mobile_web_ssh_rows

    rows: list[SummaryRow] = []
    platform = infer_platform(path, lines, 0)
    index = 0

    while index < len(lines):
        line = lines[index]
        heading_platform = platform_from_heading(line)
        if heading_platform:
            platform = heading_platform

        if "result summary:" not in line.lower():
            index += 1
            continue

        table_platform = infer_platform(path, lines, index) or platform
        table_rows, next_index = parse_summary_table(lines, index + 1)
        for cells in table_rows:
            area, status, milestone, next_task, plan_updated = cells
            rows.append(
                SummaryRow(
                    platform=table_platform,
                    status=status,
                    milestone=milestone,
                    area=area,
                    next_task=next_task,
                    plan_updated=plan_updated,
                    source=path,
                )
            )
        index = next_index

    return rows


def parse_mobile_web_ssh_results(path: Path, lines: list[str]) -> list[SummaryRow]:
    if path.name != "results.md":
        return []

    bundle = path.parent
    if not (bundle / "evidence-log.md").is_file() or not (bundle / "commands.log").is_file():
        return []

    content = "\n".join(lines)
    if MOBILE_WEB_SSH_BOUNDARY not in content or "mobile_web_ssh_" not in content:
        return []

    command_results: list[tuple[str, str, str]] = []
    for line in lines:
        match = MOBILE_WEB_SSH_RESULT_RE.match(line.strip())
        if match:
            name, status, exit_code = match.groups()
            command_results.append((name, status.upper(), exit_code))

    if not command_results:
        return []

    failed = [name for name, status, _ in command_results if status != "PASS"]
    passed = [name for name, status, _ in command_results if status == "PASS"]
    if failed:
        status = "Fail"
        next_task = "Investigate failed commands: " + ", ".join(failed)
    else:
        status = "Pass"
        next_task = "None; passed commands: " + ", ".join(passed)

    return [
        SummaryRow(
            platform=MOBILE_WEB_SSH_PLATFORM,
            status=status,
            milestone=MOBILE_WEB_SSH_MILESTONE,
            area="Simulator command bundle",
            next_task=next_task,
            plan_updated="No",
            source=path,
        )
    ]


def parse_summary_table(lines: list[str], start: int) -> tuple[list[list[str]], int]:
    index = start
    while index < len(lines) and not is_table_row(lines[index]):
        index += 1

    if index >= len(lines):
        return [], index

    header = split_table_row(lines[index])
    if normalize_cells(header) != SUMMARY_HEADER:
        return [], index + 1

    index += 1
    if index < len(lines) and is_separator_row(lines[index]):
        index += 1

    rows: list[list[str]] = []
    while index < len(lines) and is_table_row(lines[index]):
        cells = split_table_row(lines[index])
        if len(cells) >= 5 and any(cell.strip() for cell in cells):
            rows.append(cells[:5])
        index += 1
    return rows, index


def infer_platform(path: Path, lines: list[str], before_index: int) -> str:
    for index in range(before_index, -1, -1):
        if 0 <= index < len(lines):
            platform = platform_from_heading(lines[index])
            if platform:
                return platform

    for line in lines:
        platform = platform_from_heading(line)
        if platform:
            return platform

    return platform_from_path(path)


def platform_from_heading(line: str) -> str:
    stripped = line.strip()
    if stripped.startswith("# Evidence Log:"):
        return stripped.removeprefix("# Evidence Log:").strip()
    if stripped.startswith("## ") and stripped.endswith(" Checklist"):
        return stripped.removeprefix("## ").removesuffix(" Checklist").strip()
    return ""


def platform_from_path(path: Path) -> str:
    name = path.parent.name if path.name == "evidence-log.md" else path.stem
    parts = name.split("-")
    if len(parts) >= 4 and all(part.isdigit() for part in parts[:3]):
        return " ".join(part for part in parts[3:] if part).title()
    return name.replace("-", " ").replace("_", " ").title()


def is_table_row(line: str) -> bool:
    return line.strip().startswith("|") and line.strip().endswith("|")


def is_separator_row(line: str) -> bool:
    cells = split_table_row(line)
    return bool(cells) and all(set(cell.replace(":", "").strip()) <= {"-"} for cell in cells)


def split_table_row(line: str) -> list[str]:
    stripped = line.strip()
    if stripped.startswith("|"):
        stripped = stripped[1:]
    if stripped.endswith("|"):
        stripped = stripped[:-1]
    return [cell.strip() for cell in stripped.split("|")]


def normalize_cells(cells: list[str]) -> list[str]:
    return [" ".join(cell.strip().lower().split()) for cell in cells]


def markdown_escape(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def next_task_text(row: SummaryRow) -> str:
    return f"{row.area}: {row.next_task}"


def render_draft(rows: list[SummaryRow], include_source: bool = True) -> str:
    lines = [
        "# Mobile Evidence Plan Update Draft",
        "",
        "Generated from result summary tables. Review and copy relevant updates into `docs/mobile-porting-plan.md`; this draft does not edit the plan automatically.",
        "",
    ]

    if any(row.platform == MOBILE_WEB_SSH_PLATFORM for row in rows):
        lines.extend(
            [
                "## Linux/LAN Web SSH Simulator",
                "",
                MOBILE_WEB_SSH_BOUNDARY,
                "",
            ]
        )

    if include_source:
        lines.append("| Platform | Pass/Fail/Blocker | Milestone | Next task | Plan updated | Source |")
        lines.append("|---|---|---|---|---|---|")
    else:
        lines.append("| Platform | Pass/Fail/Blocker | Milestone | Next task | Plan updated |")
        lines.append("|---|---|---|---|---|")

    for row in rows:
        cells = [
            row.platform,
            row.status,
            row.milestone,
            next_task_text(row),
            row.plan_updated,
        ]
        if include_source:
            cells.append(str(row.source))
        lines.append("| " + " | ".join(markdown_escape(cell) for cell in cells) + " |")

    return "\n".join(lines) + "\n"


def main() -> int:
    args = parse_args()
    try:
        files = evidence_files(args.paths)
        rows: list[SummaryRow] = []
        for path in files:
            rows.extend(parse_file(path))
    except OSError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    if files:
        checked = ", ".join(str(path) for path in files)
    else:
        checked = ", ".join(args.paths)

    if not rows:
        print(f"error: no result summary rows found in {checked}", file=sys.stderr)
        return 1

    print(render_draft(rows, include_source=True), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
