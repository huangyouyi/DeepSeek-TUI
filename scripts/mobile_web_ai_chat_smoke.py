#!/usr/bin/env python3
"""Smoke-check the mobile Web AI chat agent-turn flow."""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
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
    timeout: float = 20.0,
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


def pending_approvals(payload: Any) -> list[dict[str, Any]]:
    if not isinstance(payload, dict):
        return []
    approvals = payload.get("pending_approvals", [])
    return [approval for approval in approvals if isinstance(approval, dict)]


def extract_approval_id(approval: dict[str, Any]) -> str:
    approval_id = approval.get("id")
    if isinstance(approval_id, str) and approval_id:
        return approval_id
    raise ScriptError("agent turn returned a pending approval without an id")


def assistant_text(payload: Any) -> str:
    if isinstance(payload, dict):
        text = payload.get("assistant_text")
        if isinstance(text, str):
            return text.strip()
        answer = payload.get("assistant_answer")
        if isinstance(answer, str):
            return answer.strip()
    return ""


def approval_summary(payload: Any) -> str | None:
    if not isinstance(payload, dict):
        return None
    text = payload.get("assistant_text")
    if isinstance(text, str) and text.strip():
        return text.strip()
    result = payload.get("result")
    if isinstance(result, dict):
        result_text = result.get("assistant_text") or result.get("summary")
        if isinstance(result_text, str) and result_text.strip():
            return result_text.strip()
    return None


def build_turn_payload(args: argparse.Namespace) -> dict[str, Any]:
    payload = {
        "message": args.message,
        "model_mode": args.model_mode,
    }
    if args.model_config:
        payload["model_config_present"] = True
    return payload


def validate_assistant_answer(text: str) -> None:
    if not text:
        raise ScriptError("agent turn did not return assistant text")
    if "not found" in text.lower():
        raise ScriptError("agent turn assistant text contained not found")


def run(args: argparse.Namespace) -> dict[str, Any]:
    if args.model_config and not args.model_config.exists():
        raise ScriptError(f"model config does not exist: {args.model_config}")

    health = request_json(args.server, "GET", "/health", access_token=args.access_token).json()
    if health.get("status") != "ok":
        raise ScriptError("GET /health did not return status ok")

    session = request_json(
        args.server,
        "POST",
        "/api/sessions",
        {"title": "mobile web ai chat smoke"},
        access_token=args.access_token,
    ).json()
    session_id = extract_session_id(session)
    turn = request_json(
        args.server,
        "POST",
        f"/api/sessions/{session_id}/agent-turn",
        build_turn_payload(args),
        timeout=args.timeout,
        access_token=args.access_token,
    ).json()
    answer = assistant_text(turn)
    validate_assistant_answer(answer)
    approvals = pending_approvals(turn)

    approval_response = None
    summary = None
    if args.auto_approve and approvals:
        approval_id = extract_approval_id(approvals[0])
        approval_response = request_json(
            args.server,
            "POST",
            f"/api/approvals/{approval_id}/respond",
            {"response": "approve_once"},
            timeout=args.timeout,
            access_token=args.access_token,
        ).json()
        summary = approval_summary(approval_response)
        if not summary:
            raise ScriptError("approval response did not include an assistant summary")

    result = {
        "status": "ok",
        "server": args.server,
        "session_id": session_id,
        "model_mode": args.model_mode,
        "model_config_present": bool(args.model_config),
        "assistant_answer": answer,
        "turn": {
            "turn_id": turn.get("turn_id") if isinstance(turn, dict) else None,
            "status": turn.get("status") if isinstance(turn, dict) else None,
            "executed_tools": turn.get("executed_tools", []) if isinstance(turn, dict) else [],
        },
        "pending_approvals": approvals,
        "approval_summary": summary,
    }
    assert_no_secret_text(result)
    if approval_response is not None:
        assert_no_secret_text(approval_response)
    return result


def print_text(summary: dict[str, Any]) -> None:
    print("Mobile Web AI chat smoke: ok")
    print(f"Server: {summary['server']}")
    print(f"Session: {summary['session_id']}")
    print(f"Model mode: {summary['model_mode']}")
    print(f"Turn status: {summary['turn']['status']}")
    print(f"Assistant: {summary['assistant_answer']}")
    print(f"Pending approvals: {len(summary['pending_approvals'])}")
    if summary["approval_summary"]:
        print(f"Approval summary: {summary['approval_summary']}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Smoke-check mobile Web AI chat agent-turn flow.")
    parser.add_argument("--server", default=DEFAULT_SERVER, help="base server URL")
    parser.add_argument("--access-token", help="optional mobile web access token sent as an HTTP header")
    parser.add_argument("--message", required=True, help="message to send to the remote Linux agent")
    parser.add_argument("--auto-approve", action="store_true", help="approve the first pending approval once")
    parser.add_argument(
        "--model-mode",
        choices=("auto", "mock", "deepseek"),
        default="auto",
        help="model mode requested for the agent turn",
    )
    parser.add_argument(
        "--model-config",
        type=Path,
        help="optional model config path marker; the helper does not read ~/.deepseek/config.toml",
    )
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON")
    parser.add_argument("--timeout", type=float, default=20.0, help="HTTP timeout for agent turn routes")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        summary = run(args)
        if args.json:
            print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
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
