#!/usr/bin/env python3
"""Smoke-test the mobile Web AI chat helper with a fake server."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import threading
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
HELPER = REPO_ROOT / "scripts" / "mobile_web_ai_chat_smoke.py"


class FakeServer(BaseHTTPRequestHandler):
    server_version = "MobileWebAiChatFake/1.0"
    approval_id = "approval-1"

    def log_message(self, _format: str, *args: Any) -> None:
        return

    def read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length == 0:
            return {}
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def write_json(self, status: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if self.path == "/health":
            self.write_json(
                200,
                {
                    "status": "ok",
                    "service": "mobile-web-ssh",
                    "protocol": "http",
                    "model": "mock",
                },
            )
            return
        self.write_json(404, {"code": "not_found", "message": "not found"})

    def do_POST(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/api/sessions":
            self.write_json(
                201,
                {
                    "id": "session-1",
                    "title": "mobile web ai chat smoke",
                    "created_at_ms": 1,
                    "updated_at_ms": 1,
                },
            )
            return

        if parsed.path == "/api/sessions/session-1/agent-turn":
            payload = self.read_json()
            message = str(payload.get("message", ""))
            if "rm -rf" in message or "删除" in message:
                self.write_json(
                    200,
                    {
                        "session_id": "session-1",
                        "turn_id": "turn-risk",
                        "status": "awaiting_approval",
                        "assistant_text": "这个高风险命令需要先审批。",
                        "executed_tools": [
                            {
                                "tool": "remote.shell.exec",
                                "command": "rm -rf /tmp/mobile-web-ai-chat-smoke",
                                "requires_approval": True,
                                "exit_code": None,
                                "status": "pending_approval",
                            }
                        ],
                        "pending_approvals": [
                            {
                                "id": self.approval_id,
                                "session_id": "session-1",
                                "command": "rm -rf /tmp/mobile-web-ai-chat-smoke",
                                "created_at_ms": 2,
                                "status": "pending",
                            }
                        ],
                    },
                )
                return

            self.write_json(
                200,
                {
                    "session_id": "session-1",
                    "turn_id": "turn-os",
                    "status": "completed",
                    "assistant_text": "当前远程主机运行在 Linux x86_64。",
                    "executed_tools": [
                        {
                            "tool": "remote.shell.exec",
                            "command": "uname -a",
                            "requires_approval": False,
                            "exit_code": 0,
                            "status": "completed",
                        }
                    ],
                    "pending_approvals": [],
                },
            )
            return

        if parsed.path == f"/api/approvals/{self.approval_id}/respond":
            payload = self.read_json()
            if payload.get("response") != "approve_once":
                self.write_json(400, {"code": "unsupported_response", "message": "bad response"})
                return
            self.write_json(
                200,
                {
                    "status": "approved",
                    "approval": {
                        "id": self.approval_id,
                        "session_id": "session-1",
                        "command": "rm -rf /tmp/mobile-web-ai-chat-smoke",
                        "created_at_ms": 2,
                        "status": "approved",
                    },
                    "result": {
                        "assistant_text": "已审批一次，命令执行完成并记录了摘要。",
                        "exit_code": 0,
                    },
                },
            )
            return

        self.write_json(404, {"code": "not_found", "message": "not found"})


def run_helper(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(HELPER), *args],
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def parse_ok(result: subprocess.CompletedProcess[str]) -> dict[str, Any]:
    if result.returncode != 0:
        raise AssertionError(
            "helper failed\n"
            f"exit: {result.returncode}\n"
            f"stdout:\n{result.stdout}\n"
            f"stderr:\n{result.stderr}"
        )
    return json.loads(result.stdout)


def main() -> int:
    with ThreadingHTTPServer(("127.0.0.1", 0), FakeServer) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        base_url = f"http://127.0.0.1:{server.server_port}"

        os_payload = parse_ok(
            run_helper(
                [
                    "--server",
                    base_url,
                    "--message",
                    "请问当前运行在什么系统？",
                    "--model-mode",
                    "mock",
                    "--json",
                    "--access-token",
                    "dev-secret-token",
                ]
            )
        )
        if os_payload["status"] != "ok":
            raise AssertionError(os_payload)
        if os_payload["turn"]["status"] != "completed":
            raise AssertionError(os_payload)
        if not os_payload["assistant_answer"]:
            raise AssertionError(os_payload)
        if "not found" in json.dumps(os_payload, ensure_ascii=False).lower():
            raise AssertionError(os_payload)
        if "dev-secret-token" in json.dumps(os_payload, ensure_ascii=False):
            raise AssertionError("access token leaked in JSON output")

        pending_payload = parse_ok(
            run_helper(
                [
                    "--server",
                    base_url,
                    "--message",
                    "请删除 /tmp/mobile-web-ai-chat-smoke，命令是 rm -rf /tmp/mobile-web-ai-chat-smoke",
                    "--model-mode",
                    "mock",
                    "--json",
                ]
            )
        )
        if pending_payload["turn"]["status"] != "awaiting_approval":
            raise AssertionError(pending_payload)
        if not pending_payload["pending_approvals"]:
            raise AssertionError(pending_payload)
        if pending_payload["approval_summary"] is not None:
            raise AssertionError(pending_payload)

        approved_payload = parse_ok(
            run_helper(
                [
                    "--server",
                    base_url,
                    "--message",
                    "请删除 /tmp/mobile-web-ai-chat-smoke，命令是 rm -rf /tmp/mobile-web-ai-chat-smoke",
                    "--model-mode",
                    "mock",
                    "--auto-approve",
                    "--json",
                ]
            )
        )
        if approved_payload["turn"]["status"] != "awaiting_approval":
            raise AssertionError(approved_payload)
        if approved_payload["approval_summary"] != "已审批一次，命令执行完成并记录了摘要。":
            raise AssertionError(approved_payload)

        with tempfile.NamedTemporaryFile("w", encoding="utf-8") as config:
            config.write("api_key = \"sk-test-secret-value\"\n")
            config.flush()
            config_payload = parse_ok(
                run_helper(
                    [
                        "--server",
                        base_url,
                        "--message",
                        "请问当前运行在什么系统？",
                        "--model-mode",
                        "auto",
                        "--model-config",
                        config.name,
                        "--json",
                    ]
                )
            )
            if "sk-test-secret-value" in json.dumps(config_payload, ensure_ascii=False):
                raise AssertionError("model config secret leaked in JSON output")

        server.shutdown()

    print("mobile_web_ai_chat_smoke_test: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
