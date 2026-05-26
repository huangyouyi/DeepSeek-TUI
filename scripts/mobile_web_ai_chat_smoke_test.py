#!/usr/bin/env python3
"""Smoke-test the mobile Web AI chat helper with a fake server."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import threading
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
HELPER = REPO_ROOT / "scripts" / "mobile_web_ai_chat_smoke.py"


class FakeServer(BaseHTTPRequestHandler):
    server_version = "MobileWebAiChatFake/1.0"
    approval_id = "approval-1"
    messages: list[dict[str, Any]] = []
    last_topic: str | None = None

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

    def write_diagnostic_turn(
        self,
        turn_id: str,
        command: str,
        stdout: str,
        assistant_text: str,
    ) -> None:
        self.__class__.messages = [
            {
                "id": f"message-{turn_id}-assistant",
                "session_id": "session-1",
                "role": "assistant",
                "created_at_ms": 3,
                "parts": [
                    {
                        "id": f"part-{turn_id}-tool-1",
                        "kind": "tool",
                        "data": {
                            "tool": "remote.shell.exec",
                            "command": command,
                            "status": "completed",
                            "requires_approval": False,
                            "stdout": stdout,
                            "stderr": "",
                            "exit_code": 0,
                            "duration_ms": 10,
                            "timed_out": False,
                            "agent_turn_id": turn_id,
                        },
                    }
                ],
            }
        ]
        self.write_json(
            200,
            {
                "session_id": "session-1",
                "turn_id": turn_id,
                "status": "completed",
                "assistant_text": assistant_text,
                "executed_tools": [
                    {
                        "tool": "remote.shell.exec",
                        "command": command,
                        "requires_approval": False,
                        "exit_code": 0,
                        "status": "completed",
                    }
                ],
                "pending_approvals": [],
            },
        )

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
        if self.path == "/api/sessions/session-1/messages":
            self.write_json(200, {"messages": self.messages})
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
            mode = payload.get("mode")
            retry_turn_id = payload.get("retry_turn_id")
            if mode == "retry":
                self.__class__.messages = [
                    {
                        "id": "message-retry-assistant",
                        "session_id": "session-1",
                        "role": "assistant",
                        "created_at_ms": 5,
                        "parts": [
                            {
                                "id": "part-retry-text-1",
                                "kind": "text",
                                "text": "已接受重试请求，并复用上一轮用户意图。",
                            }
                        ],
                    }
                ]
                self.write_json(
                    200,
                    {
                        "session_id": "session-1",
                        "turn_id": "turn-retry",
                        "retry_turn_id": retry_turn_id,
                        "status": "completed",
                        "assistant_text": "已接受重试请求，并复用上一轮用户意图。",
                        "executed_tools": [],
                        "pending_approvals": [],
                    },
                )
                return

            is_continuation = mode == "continue" or ("刚才" in message and "继续" in message)
            if is_continuation and self.__class__.last_topic == "network":
                self.__class__.messages = [
                    {
                        "id": "message-network-continue-assistant",
                        "session_id": "session-1",
                        "role": "assistant",
                        "created_at_ms": 5,
                        "parts": [
                            {
                                "id": "part-network-continue-tool-1",
                                "kind": "tool",
                                "data": {
                                    "tool": "remote.shell.exec",
                                    "command": "ip route",
                                    "status": "completed",
                                    "requires_approval": False,
                                    "stdout": "default via 192.168.30.1 dev eth0",
                                    "stderr": "",
                                    "exit_code": 0,
                                    "duration_ms": 9,
                                    "timed_out": False,
                                    "agent_turn_id": "turn-network-continue",
                                },
                            }
                        ],
                    }
                ]
                self.write_json(
                    200,
                    {
                        "session_id": "session-1",
                        "turn_id": "turn-network-continue",
                        "status": "completed",
                        "assistant_text": "上一轮网络诊断看到接口正常，这轮继续检查默认路由。",
                        "executed_tools": [
                            {
                                "tool": "remote.shell.exec",
                                "command": "ip route",
                                "requires_approval": False,
                                "exit_code": 0,
                                "status": "completed",
                            }
                        ],
                        "pending_approvals": [],
                    },
                )
                return

            if "网络" in message:
                self.__class__.last_topic = "network"
                self.__class__.messages = [
                    {
                        "id": "message-network-assistant",
                        "session_id": "session-1",
                        "role": "assistant",
                        "created_at_ms": 3,
                        "parts": [
                            {
                                "id": "part-network-tool-1",
                                "kind": "tool",
                                "data": {
                                    "tool": "remote.shell.exec",
                                    "command": "ip addr show",
                                    "status": "completed",
                                    "requires_approval": False,
                                    "stdout": "eth0: inet 192.168.30.10/24",
                                    "stderr": "",
                                    "exit_code": 0,
                                    "duration_ms": 11,
                                    "timed_out": False,
                                    "agent_turn_id": "turn-network",
                                },
                            }
                        ],
                    }
                ]
                self.write_json(
                    200,
                    {
                        "session_id": "session-1",
                        "turn_id": "turn-network",
                        "status": "completed",
                        "assistant_text": "网络接口 eth0 已获取 192.168.30.10/24 地址。",
                        "executed_tools": [
                            {
                                "tool": "remote.shell.exec",
                                "command": "ip addr show",
                                "requires_approval": False,
                                "exit_code": 0,
                                "status": "completed",
                            }
                        ],
                        "pending_approvals": [],
                    },
                )
                return

            diagnostic_cases = [
                (
                    "dns",
                    ("dns", "DNS", "解析"),
                    "getent hosts deepseek.com || nslookup deepseek.com || cat /etc/resolv.conf",
                    "deepseek.com has address 104.18.0.1\nnameserver 192.168.30.1",
                    "结论：DNS 解析看起来正常，详情已在对话中以内联工具结果展示。",
                ),
                (
                    "docker",
                    ("docker", "Docker", "容器"),
                    "docker ps --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' || docker info",
                    "NAMES    IMAGE          STATUS\nweb      nginx:alpine  Up 5 minutes",
                    "结论：Docker 正在运行，当前可看到容器状态摘要。",
                ),
                (
                    "openwrt",
                    ("openwrt", "OpenWrt", "路由器"),
                    "ubus call system board; ip route; ip addr; cat /etc/resolv.conf",
                    "board_name='router'\ndefault via 192.168.1.1 dev br-lan",
                    "结论：OpenWrt/路由器基础网络信息已读取，可继续看路由和 DNS 细节。",
                ),
                (
                    "logs",
                    ("日志", "log", "Log"),
                    "journalctl -n 80 --no-pager || logread -l 80 || dmesg | tail -80",
                    "May 25 service[123]: started\nMay 25 kernel: link is up",
                    "结论：日志摘要已生成，未在最近记录中看到明显失败关键字。",
                ),
            ]
            for key, keywords, command, stdout, assistant_text in diagnostic_cases:
                if any(keyword in message for keyword in keywords):
                    self.write_diagnostic_turn(f"turn-{key}", command, stdout, assistant_text)
                    return

            if "rm -rf" in message or "删除" in message:
                self.__class__.messages = [
                    {
                        "id": "message-danger-assistant",
                        "session_id": "session-1",
                        "role": "assistant",
                        "created_at_ms": 3,
                        "parts": [
                            {
                                "id": "part-danger-tool-1",
                                "kind": "tool",
                                "data": {
                                    "tool": "remote.shell.exec",
                                    "command": "rm -rf /tmp/mobile-web-ai-chat-target",
                                    "status": "pending_approval",
                                    "requires_approval": True,
                                    "approval_id": self.approval_id,
                                    "agent_turn_id": "turn-danger",
                                },
                            }
                        ],
                    }
                ]
                self.write_json(
                    200,
                    {
                        "session_id": "session-1",
                        "turn_id": "turn-danger",
                        "status": "awaiting_approval",
                        "assistant_text": "这个高风险命令需要先审批。",
                        "executed_tools": [
                            {
                                "tool": "remote.shell.exec",
                                "command": "rm -rf /tmp/mobile-web-ai-chat-target",
                                "requires_approval": True,
                                "exit_code": None,
                                "status": "pending_approval",
                            }
                        ],
                        "pending_approvals": [
                            {
                                "id": self.approval_id,
                                "session_id": "session-1",
                                "command": "rm -rf /tmp/mobile-web-ai-chat-target",
                                "created_at_ms": 2,
                                "status": "pending",
                            }
                        ],
                    },
                )
                return

            self.__class__.messages = [
                {
                    "id": "message-os-assistant",
                    "session_id": "session-1",
                    "role": "assistant",
                    "created_at_ms": 3,
                    "parts": [
                        {
                            "id": "part-platform-tool-1",
                            "kind": "tool",
                            "data": {
                                "tool": "remote.shell.exec",
                                "command": "uname -a",
                                "status": "completed",
                                "requires_approval": False,
                                "stdout": "Linux x86_64",
                                "stderr": "",
                                "exit_code": 0,
                                "duration_ms": 12,
                                "timed_out": False,
                                "agent_turn_id": "turn-os",
                            },
                        }
                    ],
                }
            ]
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

        if parsed.path.startswith("/api/sessions/session-1/agent-turns/") and parsed.path.endswith("/stop"):
            parts = parsed.path.strip("/").split("/")
            turn_id = parts[-2]
            self.read_json()
            self.write_json(
                200,
                {
                    "session_id": "session-1",
                    "turn_id": turn_id,
                    "status": "stopped",
                    "assistant_text": "已停止本轮，未继续执行远程命令。",
                },
            )
            return

        if parsed.path == f"/api/approvals/{self.approval_id}/respond":
            payload = self.read_json()
            response_alias = payload.get("response")
            if response_alias not in {"approve_once", "approve_session", "reject", "reject_stop"}:
                self.write_json(400, {"code": "unsupported_response", "message": "bad response"})
                return
            if response_alias in {"reject", "reject_stop"}:
                self.__class__.messages = [
                    {
                        **message,
                        "parts": [
                            {
                                **part,
                                "data": {
                                    **part["data"],
                                    "status": "rejected",
                                },
                            }
                            if part.get("id") == "part-danger-tool-1"
                            else part
                            for part in message.get("parts", [])
                        ],
                    }
                    for message in self.messages
                ]
                self.write_json(
                    200,
                    {
                        "status": "stopped" if response_alias == "reject_stop" else "rejected",
                        "approval": {
                            "id": self.approval_id,
                            "session_id": "session-1",
                            "command": "rm -rf /tmp/mobile-web-ai-chat-target",
                            "created_at_ms": 2,
                            "status": "rejected",
                        },
                        "result": {
                            "assistant_text": "已拒绝并停止本轮。" if response_alias == "reject_stop" else "已拒绝执行该命令。",
                            "stopped": response_alias == "reject_stop",
                        },
                    },
                )
                return
            self.__class__.messages = [
                {
                    **message,
                    "parts": [
                        {
                            **part,
                            "data": {
                                **part["data"],
                                "status": "completed",
                                "stdout": "removed",
                                "stderr": "",
                                "exit_code": 0,
                                "duration_ms": 15,
                                "timed_out": False,
                            },
                        }
                        if part.get("id") == "part-danger-tool-1"
                        else part
                        for part in message.get("parts", [])
                    ],
                }
                for message in self.messages
            ]
            self.write_json(
                200,
                {
                    "status": "approved",
                    "approval": {
                        "id": self.approval_id,
                        "session_id": "session-1",
                        "command": "rm -rf /tmp/mobile-web-ai-chat-target",
                        "created_at_ms": 2,
                            "status": "approved",
                    },
                    "result": {
                        "assistant_text": "已允许本会话，命令执行完成并记录了摘要。"
                        if response_alias == "approve_session"
                        else "已审批一次，命令执行完成并记录了摘要。",
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


def assert_helper_diagnostic(
    base_url: str,
    message: str,
    expected_command: str,
    expected_text: str,
) -> dict[str, Any]:
    payload = parse_ok(
        run_helper(
            [
                "--server",
                base_url,
                "--message",
                message,
                "--model-mode",
                "mock",
                "--expect-command",
                expected_command,
                "--expect-assistant-contains",
                expected_text,
                "--json",
            ]
        )
    )
    if payload["turn"]["status"] != "completed":
        raise AssertionError(payload)
    if payload["turn"]["tool_parts"][0]["data"]["command"] != expected_command:
        raise AssertionError(payload)
    if expected_text not in payload["assistant_answer"]:
        raise AssertionError(payload)
    return payload


def request_json(base_url: str, method: str, path: str, payload: dict[str, Any] | None = None) -> dict[str, Any]:
    data = None
    headers = {"Accept": "application/json"}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"
    request = urllib.request.Request(base_url + path, data=data, headers=headers, method=method)
    with urllib.request.urlopen(request, timeout=5) as response:
        return json.loads(response.read().decode("utf-8"))


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
        if os_payload["turn"]["tool_parts"][0]["data"]["command"] != "uname -a":
            raise AssertionError(os_payload)
        if os_payload["turn"]["tool_parts"][0]["data"]["status"] != "completed":
            raise AssertionError(os_payload)
        if "not found" in json.dumps(os_payload, ensure_ascii=False).lower():
            raise AssertionError(os_payload)
        os_payload_json = json.dumps(os_payload, ensure_ascii=False)
        if "dev-secret-token" in os_payload_json:
            raise AssertionError("access token leaked in JSON output")
        if "sk-test-secret-value" in os_payload_json:
            raise AssertionError("model config secret leaked in JSON output")

        assert_helper_diagnostic(
            base_url,
            "帮我检查 DNS 解析是否正常",
            "getent hosts deepseek.com || nslookup deepseek.com || cat /etc/resolv.conf",
            "DNS",
        )
        assert_helper_diagnostic(
            base_url,
            "看看 Docker 容器状态",
            "docker ps --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' || docker info",
            "Docker",
        )
        assert_helper_diagnostic(
            base_url,
            "OpenWrt 路由器状态帮我看一下",
            "ubus call system board; ip route; ip addr; cat /etc/resolv.conf",
            "OpenWrt",
        )
        assert_helper_diagnostic(
            base_url,
            "请做一个日志摘要",
            "journalctl -n 80 --no-pager || logread -l 80 || dmesg | tail -80",
            "日志",
        )

        session = request_json(
            base_url,
            "POST",
            "/api/sessions",
            {"title": "mobile web ai chat multiturn smoke"},
        )
        session_id = session["id"]
        network_turn = request_json(
            base_url,
            "POST",
            f"/api/sessions/{session_id}/agent-turn",
            {"message": "帮我看一下这个网络问题", "model_mode": "mock"},
        )
        if network_turn["status"] != "completed":
            raise AssertionError(network_turn)
        if "网络" not in network_turn["assistant_text"]:
            raise AssertionError(network_turn)

        continue_turn = request_json(
            base_url,
            "POST",
            f"/api/sessions/{session_id}/agent-turn",
            {"message": "刚才那个网络问题继续帮我看", "model_mode": "mock", "mode": "continue"},
        )
        if continue_turn["status"] != "completed":
            raise AssertionError(continue_turn)
        if continue_turn["turn_id"] == network_turn["turn_id"]:
            raise AssertionError(continue_turn)
        if "上一轮" not in continue_turn["assistant_text"] or "网络" not in continue_turn["assistant_text"]:
            raise AssertionError(continue_turn)

        retry_turn = request_json(
            base_url,
            "POST",
            f"/api/sessions/{session_id}/agent-turn",
            {"message": "", "model_mode": "mock", "mode": "retry", "retry_turn_id": network_turn["turn_id"]},
        )
        if retry_turn["status"] != "completed":
            raise AssertionError(retry_turn)
        if retry_turn.get("retry_turn_id") != network_turn["turn_id"]:
            raise AssertionError(retry_turn)

        stopped = request_json(
            base_url,
            "POST",
            f"/api/sessions/{session_id}/agent-turns/{continue_turn['turn_id']}/stop",
            {},
        )
        if stopped != {
            "session_id": session_id,
            "turn_id": continue_turn["turn_id"],
            "status": "stopped",
            "assistant_text": "已停止本轮，未继续执行远程命令。",
        }:
            raise AssertionError(stopped)

        pending_payload = parse_ok(
            run_helper(
                [
                    "--server",
                    base_url,
                    "--message",
                    "请删除 /tmp/mobile-web-ai-chat-target，命令是 rm -rf /tmp/mobile-web-ai-chat-target",
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
        if pending_payload["turn"]["tool_parts"][0]["data"]["command"] != "rm -rf /tmp/mobile-web-ai-chat-target":
            raise AssertionError(pending_payload)
        if pending_payload["turn"]["tool_parts"][0]["data"]["status"] != "pending_approval":
            raise AssertionError(pending_payload)
        if pending_payload["approval_summary"] is not None:
            raise AssertionError(pending_payload)

        approved_payload = parse_ok(
            run_helper(
                [
                    "--server",
                    base_url,
                    "--message",
                    "请删除 /tmp/mobile-web-ai-chat-target，命令是 rm -rf /tmp/mobile-web-ai-chat-target",
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
        if approved_payload["approval_tool_parts"][0]["data"]["command"] != "rm -rf /tmp/mobile-web-ai-chat-target":
            raise AssertionError(approved_payload)
        if approved_payload["approval_tool_parts"][0]["data"]["status"] != "completed":
            raise AssertionError(approved_payload)

        allowed_payload = parse_ok(
            run_helper(
                [
                    "--server",
                    base_url,
                    "--message",
                    "请删除 /tmp/mobile-web-ai-chat-target，命令是 rm -rf /tmp/mobile-web-ai-chat-target",
                    "--model-mode",
                    "mock",
                    "--auto-approve",
                    "--approval-response",
                    "approve_session",
                    "--json",
                ]
            )
        )
        if allowed_payload["approval_summary"] != "已允许本会话，命令执行完成并记录了摘要。":
            raise AssertionError(allowed_payload)
        if allowed_payload["approval_tool_parts"][0]["data"]["status"] != "completed":
            raise AssertionError(allowed_payload)

        rejected_stop_payload = parse_ok(
            run_helper(
                [
                    "--server",
                    base_url,
                    "--message",
                    "请删除 /tmp/mobile-web-ai-chat-target，命令是 rm -rf /tmp/mobile-web-ai-chat-target",
                    "--model-mode",
                    "mock",
                    "--auto-approve",
                    "--approval-response",
                    "reject_stop",
                    "--json",
                ]
            )
        )
        if rejected_stop_payload["approval_summary"] != "已拒绝并停止本轮。":
            raise AssertionError(rejected_stop_payload)
        if rejected_stop_payload["approval_tool_parts"][0]["data"]["status"] != "rejected":
            raise AssertionError(rejected_stop_payload)

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
