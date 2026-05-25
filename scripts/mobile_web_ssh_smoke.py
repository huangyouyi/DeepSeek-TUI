#!/usr/bin/env python3
"""Smoke-check the mobile Web SSH simulator HTTP API."""

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
    timeout: float = 5.0,
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
        body = error.read()
        assert_no_secret_text(body.decode("utf-8", errors="replace"))
        raise ScriptError(f"{method} {path} failed with HTTP {error.code}") from error
    except urllib.error.URLError as error:
        raise ScriptError(f"{method} {path} failed: {error.reason}") from error
    assert_no_secret_text(result.body.decode("utf-8", errors="replace"))
    return result


def connect_event(server: str, access_token: str | None = None) -> dict[str, Any]:
    headers = {"Accept": "text/event-stream"}
    if access_token:
        headers["X-Mobile-Web-Token"] = access_token
    request = urllib.request.Request(
        server.rstrip("/") + "/event",
        headers=headers,
        method="GET",
    )
    try:
        with urllib.request.urlopen(request, timeout=5.0) as response:
            content_type = response.headers.get("Content-Type", "")
            lines: list[bytes] = []
            for _ in range(4):
                line = response.readline()
                if not line:
                    break
                lines.append(line)
                if line == b"\n" or line == b"\r\n":
                    break
            chunk = b"".join(lines)
    except urllib.error.URLError as error:
        raise ScriptError(f"GET /event failed: {error.reason}") from error
    assert_no_secret_text(chunk.decode("utf-8", errors="replace"))
    if "text/event-stream" not in content_type:
        raise ScriptError(f"GET /event returned unexpected content type: {content_type}")
    return {"connected": True, "content_type": content_type}


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


def run(args: argparse.Namespace) -> dict[str, Any]:
    updated_target = maybe_update_target(args)
    health = request_json(args.server, "GET", "/health").json()
    if health.get("status") != "ok":
        raise ScriptError("GET /health did not return status ok")

    event = connect_event(args.server, args.access_token)
    target = request_json(args.server, "GET", "/api/ssh/target", access_token=args.access_token).json()
    ssh_check = request_json(
        args.server,
        "POST",
        "/api/ssh/check",
        access_token=args.access_token,
        timeout=args.timeout,
    ).json()
    session = request_json(
        args.server,
        "POST",
        "/api/sessions",
        {"title": "mobile web smoke"},
        access_token=args.access_token,
    ).json()
    session_id = extract_session_id(session)
    diagnostic = request_json(
        args.server,
        "POST",
        "/api/diagnostics/run",
        {"session_id": session_id, "diagnostic": args.diagnostic},
        timeout=args.timeout,
        access_token=args.access_token,
    ).json()
    audit_payload = request_json(args.server, "GET", "/api/audit/recent", access_token=args.access_token).json()
    entries = audit_entries(audit_payload)

    if ssh_check.get("status") not in {"reachable", "unreachable", "timed_out", "error"}:
        raise ScriptError("POST /api/ssh/check returned an unknown status")

    assert_no_secret_text(
        {
            "health": health,
            "target": target,
            "ssh_check": ssh_check,
            "diagnostic": diagnostic,
            "audit": entries,
        }
    )
    return {
        "status": "ok",
        "server": args.server,
        "checks": {
            "health": True,
            "event": event["connected"],
            "ssh_target": bool(target),
            "ssh_check": True,
            "diagnostic": True,
            "audit_recent": True,
        },
        "session_id": session_id,
        "diagnostic": args.diagnostic,
        "audit_count": len(entries),
        "target": target,
        "ssh_check": ssh_check,
        "updated_target": updated_target,
    }


def print_text(summary: dict[str, Any]) -> None:
    print("Mobile Web SSH smoke: ok")
    print(f"Server: {summary['server']}")
    print(f"Session: {summary['session_id']}")
    print(f"Diagnostic: {summary['diagnostic']}")
    target = summary["target"]
    print(f"SSH target: {target.get('user', '?')}@{target.get('host', '?')}:{target.get('port', '?')}")
    print(f"SSH check: {summary['ssh_check'].get('status', '?')}")
    print(f"Audit entries: {summary['audit_count']}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Smoke-check mobile Web SSH simulator API routes.")
    parser.add_argument("--server", default=DEFAULT_SERVER, help="base server URL")
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON")
    parser.add_argument("--diagnostic", default="system_info", help="preset diagnostic id to run")
    parser.add_argument("--ssh-host", help="override SSH target host before smoke checks")
    parser.add_argument("--ssh-user", help="override SSH target user before smoke checks")
    parser.add_argument("--ssh-port", type=int, help="override SSH target port before smoke checks")
    parser.add_argument("--access-token", help="optional mobile web access token sent as an HTTP header")
    parser.add_argument("--timeout", type=float, default=15.0, help="HTTP timeout for command routes")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        summary = run(args)
        assert_no_secret_text(summary)
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
