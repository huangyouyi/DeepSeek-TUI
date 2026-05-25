#!/usr/bin/env python3
"""Smoke tests for mobile Web SSH simulator scripts.

The tests use a local fake HTTP server. They do not contact the real SSH target.
"""

from __future__ import annotations

import json
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
FLOW_SCRIPT = REPO_ROOT / "scripts" / "mobile_web_ssh_flow_simulator.py"
SMOKE_SCRIPT = REPO_ROOT / "scripts" / "mobile_web_ssh_smoke.py"
DEV_SCRIPT = REPO_ROOT / "scripts" / "mobile_web_dev.py"


class FakeState:
    def __init__(self) -> None:
        self.session_count = 0
        self.approval_count = 0
        self.diagnostics: list[dict[str, Any]] = []
        self.prepared: list[dict[str, Any]] = []
        self.responses: list[dict[str, Any]] = []
        self.target = {"host": "192.168.30.244", "user": "root", "port": 22, "key_present": False}
        self.audit: list[dict[str, Any]] = []

    def next_session(self) -> dict[str, Any]:
        self.session_count += 1
        return {
            "id": f"session-{self.session_count}",
            "title": "mobile web smoke",
            "created_at_ms": 1,
            "updated_at_ms": 1,
        }

    def next_approval(self, body: dict[str, Any]) -> dict[str, Any]:
        self.approval_count += 1
        approval = {
            "id": f"approval-{self.approval_count}",
            "session_id": body["session_id"],
            "command": body["command"],
            "cwd": body.get("cwd"),
            "created_at_ms": self.approval_count,
            "status": "pending",
        }
        self.prepared.append(approval)
        self.audit.append(
            {
                "id": f"audit-prepare-{self.approval_count}",
                "session_id": body["session_id"],
                "kind": "approval.prepared",
                "created_at_ms": self.approval_count,
                "summary": "advanced command prepared",
                "metadata": {"approval_id": approval["id"]},
            }
        )
        return approval


class FakeHandler(BaseHTTPRequestHandler):
    server_version = "MobileWebFake/1"

    def log_message(self, format: str, *args: Any) -> None:
        return

    @property
    def state(self) -> FakeState:
        return self.server.state  # type: ignore[attr-defined]

    def _read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length == 0:
            return {}
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def _send_json(self, payload: Any, status: int = 200) -> None:
        data = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self) -> None:
        if self.path == "/health":
            self._send_json(
                {
                    "status": "ok",
                    "service": "deepseek-mobile-web-server",
                    "protocol": "mobile-web-v1",
                    "model": "mock",
                }
            )
            return
        if self.path == "/event":
            body = b"event: connection.updated\ndata: {\"status\":\"connected\"}\n\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path == "/api/ssh/target":
            self._send_json(self.state.target)
            return
        if self.path == "/api/audit/recent":
            self._send_json({"entries": self.state.audit})
            return
        if self.path.endswith("/messages"):
            self._send_json({"messages": []})
            return
        if self.path == "/api/sessions":
            self._send_json({"sessions": []})
            return
        self._send_json({"code": "not_found", "message": self.path}, status=404)

    def do_PUT(self) -> None:
        if self.path == "/api/ssh/target":
            body = self._read_json()
            self.state.target = {
                "host": body.get("host", self.state.target["host"]),
                "user": body.get("user", self.state.target["user"]),
                "port": int(body.get("port", self.state.target["port"])),
                "key_present": False,
            }
            self._send_json(self.state.target)
            return
        self._send_json({"code": "not_found", "message": self.path}, status=404)

    def do_POST(self) -> None:
        body = self._read_json()
        if self.path == "/api/ssh/check":
            self._send_json(
                {
                    "status": "reachable",
                    "target": self.state.target,
                    "check_id": "ssh-check-fake",
                    "command": "true",
                    "requires_approval": False,
                    "exit_code": 0,
                    "duration_ms": 1,
                    "timed_out": False,
                }
            )
            return
        if self.path == "/api/sessions":
            self._send_json(self.state.next_session(), status=201)
            return
        if self.path == "/api/diagnostics/run":
            self.state.diagnostics.append(body)
            entry = {
                "id": f"audit-diagnostic-{len(self.state.diagnostics)}",
                "session_id": body["session_id"],
                "kind": "diagnostic.completed",
                "created_at_ms": 10,
                "summary": f"{body['diagnostic']} completed",
                "metadata": {"exit_code": 0},
            }
            self.state.audit.append(entry)
            self._send_json(
                {
                    "session_id": body["session_id"],
                    "diagnostic": body["diagnostic"],
                    "status": "completed",
                    "result": {"exit_code": 0, "stdout": "Linux fake\n", "stderr": ""},
                }
            )
            return
        if self.path == "/api/commands/prepare":
            approval = self.state.next_approval(body)
            self._send_json({"approval": approval, "status": "pending"}, status=201)
            return
        if self.path.startswith("/api/approvals/") and self.path.endswith("/respond"):
            approval_id = self.path.split("/")[3]
            response = body["response"]
            self.state.responses.append({"approval_id": approval_id, "response": response})
            status = "rejected" if response == "reject" else "approved"
            entry = {
                "id": f"audit-response-{len(self.state.responses)}",
                "session_id": "session-1",
                "kind": f"approval.{status}",
                "created_at_ms": 20,
                "summary": f"advanced command {status}",
                "metadata": {"approval_id": approval_id, "exit_code": 0},
            }
            self.state.audit.append(entry)
            self._send_json(
                {
                    "approval": {
                        "id": approval_id,
                        "session_id": "session-1",
                        "command": "fake",
                        "created_at_ms": 20,
                        "status": status,
                    },
                    "status": status,
                    "result": {"exit_code": 0, "stdout": "approved\n", "stderr": ""},
                }
            )
            return
        self._send_json({"code": "not_found", "message": self.path}, status=404)


def run_script(script: Path, *args: str) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        [sys.executable, str(script), *args],
        cwd=REPO_ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"{script.name} exited {completed.returncode}\nstdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def with_fake_server(callback: Any) -> None:
    server = ThreadingHTTPServer(("127.0.0.1", 0), FakeHandler)
    server.state = FakeState()  # type: ignore[attr-defined]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        callback(f"http://127.0.0.1:{server.server_port}", server.state)  # type: ignore[attr-defined]
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def assert_no_secret_output(completed: subprocess.CompletedProcess[str]) -> None:
    combined = completed.stdout + completed.stderr
    forbidden = ("sk-", "deepseek_token", "DEEPSEEK_API_KEY", "approval_nonce", "lease_secret")
    for needle in forbidden:
        if needle in combined:
            raise AssertionError(f"script output leaked secret-like text containing {needle!r}")


def test_flow_simulator_uses_fake_server_and_auto_approval() -> None:
    def run(base_url: str, state: FakeState) -> None:
        completed = run_script(
            FLOW_SCRIPT,
            "--server",
            base_url,
            "--ssh-host",
            "127.0.0.2",
            "--ssh-user",
            "tester",
            "--ssh-port",
            "2222",
            "--auto-approve",
            "--json",
        )
        assert_no_secret_output(completed)
        summary = json.loads(completed.stdout)
        if summary["status"] != "ok":
            raise AssertionError(completed.stdout)
        if state.target["host"] != "127.0.0.2" or state.target["user"] != "tester" or state.target["port"] != 2222:
            raise AssertionError(f"target override not applied: {state.target}")
        if len(state.diagnostics) != 1:
            raise AssertionError(f"expected one diagnostic, got {state.diagnostics}")
        if [item["response"] for item in state.responses] != ["reject", "approve_once"]:
            raise AssertionError(f"unexpected approval responses: {state.responses}")

    with_fake_server(run)


def test_smoke_script_checks_core_endpoints() -> None:
    def run(base_url: str, state: FakeState) -> None:
        completed = run_script(
            SMOKE_SCRIPT,
            "--server",
            base_url,
            "--diagnostic",
            "system_info",
            "--json",
        )
        assert_no_secret_output(completed)
        summary = json.loads(completed.stdout)
        if summary["status"] != "ok":
            raise AssertionError(completed.stdout)
        if len(state.diagnostics) != 1:
            raise AssertionError(f"expected one diagnostic, got {state.diagnostics}")

    with_fake_server(run)


def test_dev_helper_prints_redacted_access_token() -> None:
    completed = run_script(
        DEV_SCRIPT,
        "--print",
        "--access-token",
        "dev-secret-token",
        "--no-web",
    )
    combined = completed.stdout + completed.stderr
    if "dev-secret-token" in combined:
        raise AssertionError(f"dev helper leaked access token:\n{combined}")
    if "--access-token REDACTED" not in combined:
        raise AssertionError(f"dev helper did not print redacted token placeholder:\n{combined}")
    if "scripts/mobile_web_ssh_smoke.py --server http://127.0.0.1:8788 --access-token REDACTED" not in combined:
        raise AssertionError(f"smoke helper command did not redact access token:\n{combined}")


def main() -> int:
    tests = [
        test_flow_simulator_uses_fake_server_and_auto_approval,
        test_smoke_script_checks_core_endpoints,
        test_dev_helper_prints_redacted_access_token,
    ]
    for test in tests:
        test()
        print(f"ok - {test.__name__}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
