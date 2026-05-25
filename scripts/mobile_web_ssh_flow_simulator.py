#!/usr/bin/env python3
"""Exercise the mobile Web SSH simulator approval flow over HTTP."""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any


DEFAULT_SERVER = "http://127.0.0.1:8788"
SECRET_PATTERNS = (
    re.compile(r"sk-[A-Za-z0-9_-]{8,}"),
    re.compile(r"(?i)deepseek[_-]?(api[_-]?)?key"),
    re.compile(r"(?i)deepseek[_-]?token"),
    re.compile(r"(?i)bearer\s+[A-Za-z0-9._~+/=-]{8,}"),
    re.compile(r"(?i)(approval[_-]?nonce|lease[_-]?secret|idempotency[_-]?key)"),
)


class ScriptError(RuntimeError):
    pass


@dataclass(frozen=True)
class HttpResult:
    status: int
    headers: dict[str, str]
    body: bytes

    def json(self) -> Any:
        if not self.body:
            return {}
        return json.loads(self.body.decode("utf-8"))


def assert_no_secret_text(value: Any) -> None:
    text = value if isinstance(value, str) else json.dumps(value, sort_keys=True)
    for pattern in SECRET_PATTERNS:
        if pattern.search(text):
            raise ScriptError("server response contained secret-like text; refusing to print it")


def request_json(
    server: str,
    method: str,
    path: str,
    payload: dict[str, Any] | None = None,
    timeout: float = 10.0,
    access_token: str | None = None,
) -> HttpResult:
    data = None
    headers = {"Accept": "application/json"}
    if access_token:
        headers["X-Mobile-Web-Token"] = access_token
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(
        server.rstrip("/") + path,
        data=data,
        headers=headers,
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            result = HttpResult(
                status=response.status,
                headers={key.lower(): value for key, value in response.headers.items()},
                body=response.read(),
            )
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        assert_no_secret_text(body)
        raise ScriptError(f"{method} {path} failed with HTTP {error.code}") from error
    except urllib.error.URLError as error:
        raise ScriptError(f"{method} {path} failed: {error.reason}") from error
    assert_no_secret_text(result.body.decode("utf-8", errors="replace"))
    return result


def extract_session_id(payload: Any) -> str:
    if isinstance(payload, dict):
        if isinstance(payload.get("id"), str):
            return payload["id"]
        session = payload.get("session")
        if isinstance(session, dict) and isinstance(session.get("id"), str):
            return session["id"]
        sessions = payload.get("sessions")
        if isinstance(sessions, list) and sessions and isinstance(sessions[0].get("id"), str):
            return sessions[0]["id"]
    raise ScriptError("POST /api/sessions did not return a session id")


def extract_messages(payload: Any) -> list[Any]:
    if isinstance(payload, list):
        return payload
    if isinstance(payload, dict):
        for key in ("messages", "items"):
            messages = payload.get(key)
            if isinstance(messages, list):
                return messages
    raise ScriptError("GET session messages did not return a message list")


def extract_approval_id(payload: Any) -> str:
    if isinstance(payload, dict):
        if isinstance(payload.get("id"), str):
            return payload["id"]
        approval = payload.get("approval")
        if isinstance(approval, dict) and isinstance(approval.get("id"), str):
            return approval["id"]
        pending = payload.get("pending_approval")
        if isinstance(pending, dict) and isinstance(pending.get("id"), str):
            return pending["id"]
    raise ScriptError("POST /api/commands/prepare did not return an approval id")


def audit_entries(payload: Any) -> list[Any]:
    if isinstance(payload, list):
        return payload
    if isinstance(payload, dict):
        for key in ("entries", "audit", "items"):
            entries = payload.get(key)
            if isinstance(entries, list):
                return entries
    raise ScriptError("GET /api/audit/recent did not return an audit list")


def maybe_update_target(args: argparse.Namespace) -> Any | None:
    values = {
        key: value
        for key, value in {
            "host": args.ssh_host,
            "user": args.ssh_user,
            "port": args.ssh_port,
        }.items()
        if value is not None
    }
    if not values:
        return None
    return request_json(
        args.server,
        "PUT",
        "/api/ssh/target",
        values,
        access_token=args.access_token,
    ).json()


def require_audit_contains(entries: list[Any], needle: str) -> None:
    if needle not in json.dumps(entries, sort_keys=True):
        raise ScriptError(f"audit output did not include expected marker: {needle}")


def run(args: argparse.Namespace) -> dict[str, Any]:
    updated_target = maybe_update_target(args)
    health = request_json(args.server, "GET", "/health").json()
    if health.get("status") != "ok":
        raise ScriptError("GET /health did not return status ok")

    target = request_json(args.server, "GET", "/api/ssh/target", access_token=args.access_token).json()
    session = request_json(
        args.server,
        "POST",
        "/api/sessions",
        {"title": "mobile web ssh flow simulator"},
        access_token=args.access_token,
    ).json()
    session_id = extract_session_id(session)
    messages = extract_messages(
        request_json(
            args.server,
            "GET",
            f"/api/sessions/{session_id}/messages",
            access_token=args.access_token,
        ).json()
    )

    diagnostic = request_json(
        args.server,
        "POST",
        "/api/diagnostics/run",
        {"session_id": session_id, "diagnostic": args.diagnostic},
        timeout=args.timeout,
        access_token=args.access_token,
    ).json()

    reject_prepare = request_json(
        args.server,
        "POST",
        "/api/commands/prepare",
        {"session_id": session_id, "command": args.reject_command},
        access_token=args.access_token,
    ).json()
    reject_approval_id = extract_approval_id(reject_prepare)
    reject_response = request_json(
        args.server,
        "POST",
        f"/api/approvals/{reject_approval_id}/respond",
        {"response": "reject"},
        timeout=args.timeout,
        access_token=args.access_token,
    ).json()

    approve_prepare = request_json(
        args.server,
        "POST",
        "/api/commands/prepare",
        {"session_id": session_id, "command": args.approve_command},
        access_token=args.access_token,
    ).json()
    approve_approval_id = extract_approval_id(approve_prepare)
    approve_response = None
    if args.auto_approve:
        approve_response = request_json(
            args.server,
            "POST",
            f"/api/approvals/{approve_approval_id}/respond",
            {"response": "approve_once"},
            timeout=args.timeout,
            access_token=args.access_token,
        ).json()

    entries = audit_entries(
        request_json(args.server, "GET", "/api/audit/recent", access_token=args.access_token).json()
    )
    require_audit_contains(entries, "diagnostic")
    require_audit_contains(entries, reject_approval_id)
    require_audit_contains(entries, approve_approval_id)
    if args.auto_approve:
        require_audit_contains(entries, "approved")

    summary = {
        "status": "ok",
        "server": args.server,
        "session_id": session_id,
        "message_count": len(messages),
        "target": target,
        "updated_target": updated_target,
        "diagnostic": {
            "id": args.diagnostic,
            "status": diagnostic.get("status", "unknown") if isinstance(diagnostic, dict) else "unknown",
        },
        "approvals": {
            "rejected": reject_approval_id,
            "prepared_for_auto_approve": approve_approval_id,
            "auto_approved": bool(args.auto_approve),
        },
        "audit_count": len(entries),
    }
    assert_no_secret_text(summary)
    assert_no_secret_text(
        {
            "diagnostic": diagnostic,
            "reject": reject_response,
            "approve": approve_response,
            "audit": entries,
        }
    )
    return summary


def print_text(summary: dict[str, Any]) -> None:
    print("Mobile Web SSH flow simulator: ok")
    print(f"Server: {summary['server']}")
    print(f"Session: {summary['session_id']}")
    print(f"Diagnostic: {summary['diagnostic']['id']} ({summary['diagnostic']['status']})")
    print(f"Rejected approval: {summary['approvals']['rejected']}")
    print(f"Prepared approval: {summary['approvals']['prepared_for_auto_approve']}")
    print(f"Auto-approved: {summary['approvals']['auto_approved']}")
    print(f"Audit entries: {summary['audit_count']}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Exercise mobile Web SSH simulator HTTP flows.")
    parser.add_argument("--server", default=DEFAULT_SERVER, help="base server URL")
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON")
    parser.add_argument("--auto-approve", action="store_true", help="approve the second prepared command once")
    parser.add_argument("--diagnostic", default="system_info", help="preset diagnostic id to run")
    parser.add_argument("--reject-command", default="echo rejected-by-mobile-web-flow", help="advanced command to reject")
    parser.add_argument("--approve-command", default="pwd", help="advanced command to prepare and optionally approve")
    parser.add_argument("--ssh-host", help="override SSH target host before running the flow")
    parser.add_argument("--ssh-user", help="override SSH target user before running the flow")
    parser.add_argument("--ssh-port", type=int, help="override SSH target port before running the flow")
    parser.add_argument("--access-token", help="optional mobile web access token sent as an HTTP header")
    parser.add_argument("--timeout", type=float, default=20.0, help="HTTP timeout for command routes")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        summary = run(args)
        if args.json:
            print(json.dumps(summary, sort_keys=True))
        else:
            print_text(summary)
        return 0
    except ScriptError as error:
        if args.json:
            print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        else:
            print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
