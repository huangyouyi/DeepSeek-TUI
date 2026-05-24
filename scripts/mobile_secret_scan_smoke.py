#!/usr/bin/env python3
"""Smoke tests for mobile_secret_scan.py."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "mobile_secret_scan.py"


def run_script(
    *args: str,
    check: bool = True,
    stdin: str | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=REPO_ROOT,
        check=check,
        input=stdin,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def assert_contains(output: str, needle: str) -> None:
    if needle not in output:
        raise AssertionError(f"missing {needle!r} in output:\n{output}")


def test_clean_file_passes_and_allows_redacted_placeholders() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        clean = Path(tmpdir) / "clean.log"
        clean.write_text(
            "\n".join(
                [
                    "Authorization: Bearer <redacted-token>",
                    "approval_nonce=<redacted-token>",
                    "command_lease_id=<redacted-token>",
                    "idempotency_key=<redacted-token>",
                    "env SECRET_ENV_VALUE=<redacted-token>",
                ]
            ),
            encoding="utf-8",
        )

        completed = run_script(str(clean))

    assert_contains(completed.stdout, "mobile secret scan passed")


def test_secret_file_fails_on_bearer_token() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        secret = Path(tmpdir) / "secret.log"
        secret.write_text(
            "Authorization: Bearer mobile-secret-token-123456\n",
            encoding="utf-8",
        )

        completed = run_script(str(secret), check=False)

    if completed.returncode == 0:
        raise AssertionError("expected bearer token sentinel to fail")
    assert_contains(completed.stdout, "mobile secret scan failed")
    assert_contains(completed.stdout, "bearer token")


def test_directory_scan_recurses() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        nested = Path(tmpdir) / "nested"
        nested.mkdir()
        (nested / "audit.log").write_text(
            "approval_nonce=approval-nonce-test-123456\n",
            encoding="utf-8",
        )

        completed = run_script(tmpdir, check=False)

    if completed.returncode == 0:
        raise AssertionError("expected recursive directory scan to fail")
    assert_contains(completed.stdout, "approval nonce")
    assert_contains(completed.stdout, "audit.log")


def test_stdin_scan_fails() -> None:
    completed = run_script(
        "-",
        check=False,
        stdin="command_lease_id=command-lease-test-123456\n",
    )

    if completed.returncode == 0:
        raise AssertionError("expected stdin sentinel to fail")
    assert_contains(completed.stdout, "<stdin>")
    assert_contains(completed.stdout, "command lease id")


def test_default_rules_cover_idempotency_key_and_raw_command_sentinel() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        secret = Path(tmpdir) / "raw-command.log"
        secret.write_text(
            "\n".join(
                [
                    "idempotency_key=idempotency-test-123456",
                    "MOBILE_SECRET_SCAN_RAW_COMMAND_SENTINEL",
                ]
            ),
            encoding="utf-8",
        )

        completed = run_script(str(secret), check=False)

    if completed.returncode == 0:
        raise AssertionError("expected idempotency/raw command sentinels to fail")
    assert_contains(completed.stdout, "idempotency key")
    assert_contains(completed.stdout, "raw shell command sentinel")


def main() -> int:
    tests = [
        test_clean_file_passes_and_allows_redacted_placeholders,
        test_secret_file_fails_on_bearer_token,
        test_directory_scan_recurses,
        test_stdin_scan_fails,
        test_default_rules_cover_idempotency_key_and_raw_command_sentinel,
    ]
    for test in tests:
        test()
        print(f"ok - {test.__name__}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
