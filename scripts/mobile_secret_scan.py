#!/usr/bin/env python3
"""Fail if mobile user-visible output contains known secret-like sentinels."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


REDACTED_PLACEHOLDER = "<redacted-token>"


@dataclass(frozen=True)
class Rule:
    name: str
    pattern: re.Pattern[str]


@dataclass(frozen=True)
class Finding:
    source: str
    line_number: int
    rule_name: str
    evidence: str


RULES = [
    Rule(
        "bearer token",
        re.compile(r"\bBearer\s+(?!<redacted-token>\b)[A-Za-z0-9._~+/=-]{8,}"),
    ),
    Rule(
        "approval nonce",
        re.compile(
            r"\bapproval[-_\s]?nonce\b\s*[:=]\s*(?!<redacted-token>\b)[A-Za-z0-9._:-]{8,}",
            re.IGNORECASE,
        ),
    ),
    Rule(
        "command lease id",
        re.compile(
            r"\bcommand[-_\s]?lease[-_\s]?id\b\s*[:=]\s*(?!<redacted-token>\b)[A-Za-z0-9._:-]{8,}",
            re.IGNORECASE,
        ),
    ),
    Rule(
        "idempotency key",
        re.compile(
            r"\bidempotency[-_\s]?key\b\s*[:=]\s*(?!<redacted-token>\b)[A-Za-z0-9._:-]{8,}",
            re.IGNORECASE,
        ),
    ),
    Rule(
        "sensitive env sentinel",
        re.compile(
            r"\b(?:SECRET_ENV_VALUE|SENSITIVE_ENV_SENTINEL)\b"
            r"(?!\s*[:=]\s*<redacted-token>)"
            r"(?:\s*[:=]\s*\S+)?"
        ),
    ),
    Rule(
        "raw shell command sentinel",
        re.compile(r"\bMOBILE_SECRET_SCAN_RAW_COMMAND_SENTINEL\b"),
    ),
]


def iter_files(targets: Iterable[str]) -> Iterable[tuple[str, str]]:
    for target in targets:
        if target == "-":
            yield "<stdin>", sys.stdin.read()
            continue

        path = Path(target)
        if path.is_dir():
            for child in sorted(p for p in path.rglob("*") if p.is_file()):
                yield str(child), read_text(child)
        else:
            yield str(path), read_text(path)


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def scan_text(source: str, text: str) -> list[Finding]:
    findings: list[Finding] = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        for rule in RULES:
            match = rule.pattern.search(line)
            if match:
                findings.append(
                    Finding(
                        source=source,
                        line_number=line_number,
                        rule_name=rule.name,
                        evidence=match.group(0),
                    )
                )
    return findings


def scan_targets(targets: Iterable[str]) -> list[Finding]:
    findings: list[Finding] = []
    for source, text in iter_files(targets):
        findings.extend(scan_text(source, text))
    return findings


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Scan files, directories, or stdin for known mobile secret-like "
            "sentinels in user-visible output."
        )
    )
    parser.add_argument(
        "targets",
        nargs="+",
        help="files/directories to scan, or '-' to read stdin",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    findings = scan_targets(args.targets)

    if findings:
        print("mobile secret scan failed:")
        for finding in findings:
            print(
                f"- {finding.source}:{finding.line_number}: "
                f"{finding.rule_name}: {finding.evidence}"
            )
        return 1

    print("mobile secret scan passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
