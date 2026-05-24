use std::time::Duration;

use kai_runner::server::RunnerApiHandler;
use kai_runner::{
    CommandLeaseAction, CommandLeaseEnvelope, KaiRunner, McpToolHandler, McpToolRequest,
    RunnerToolError, ShellApprovalNonceManager, ShellExecutionRequest, ShellExecutionResult,
    ShellExecutor,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
struct FakeMcpHandler;

impl McpToolHandler for FakeMcpHandler {
    fn call(&self, request: McpToolRequest) -> Result<Value, RunnerToolError> {
        Ok(json!({
            "server": request.server,
            "tool": request.tool,
            "echo": request.arguments,
        }))
    }
}

#[derive(Debug, Clone)]
struct FakeShellExecutor;

impl ShellExecutor for FakeShellExecutor {
    fn execute(&self, _request: ShellExecutionRequest) -> ShellExecutionResult {
        ShellExecutionResult {
            stdout: "should-not-run\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            timed_out: false,
        }
    }
}

#[test]
fn health_route_reports_runner_status() {
    let handler = RunnerApiHandler::default();

    let response = handler.handle("GET", "/health", None);

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        json!({
            "status": "ok",
            "mode": "runner"
        })
    );
}

#[test]
fn capabilities_route_returns_documented_runner_tools() {
    let handler = RunnerApiHandler::default();

    let response = handler.handle("GET", "/capabilities", None);

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        json!({
            "type": "capabilities",
            "mode": "runner",
            "tools": [
                "remote.shell.exec",
                "remote.powershell.exec",
                "remote.file.read",
                "remote.file.write",
                "remote.diagnose.system",
                "remote.package.install",
                "remote.browser.open",
                "remote.browser.extract_text",
                "remote.browser.click",
                "remote.bootstrap.guide",
                "remote.mcp.call"
            ]
        })
    );
}

#[test]
fn authenticated_health_route_allows_missing_token() {
    let handler = RunnerApiHandler::new_with_token(kai_runner::KaiRunner, "test-token");

    let response = handler.handle("GET", "/health", None);

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        json!({
            "status": "ok",
            "mode": "runner"
        })
    );
}

#[test]
fn authenticated_capabilities_route_rejects_missing_token() {
    let handler = RunnerApiHandler::new_with_token(kai_runner::KaiRunner, "test-token");

    let response = handler.handle("GET", "/capabilities", None);

    assert_eq!(response.status, 401);
    assert_eq!(
        response.body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
}

#[test]
fn authenticated_capabilities_route_rejects_wrong_token() {
    let handler = RunnerApiHandler::new_with_token(kai_runner::KaiRunner, "test-token");

    let response =
        handler.handle_with_bearer_token("GET", "/capabilities", None, Some("wrong-token"));

    assert_eq!(response.status, 401);
    assert_eq!(
        response.body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
}

#[test]
fn authenticated_capabilities_route_accepts_correct_token() {
    let handler = RunnerApiHandler::new_with_token(kai_runner::KaiRunner, "test-token");

    let response =
        handler.handle_with_bearer_token("GET", "/capabilities", None, Some("test-token"));

    assert_eq!(response.status, 200);
    assert_eq!(response.body["type"], json!("capabilities"));
    assert_eq!(response.body["mode"], json!("runner"));
}

#[test]
fn tool_call_route_runs_diagnose_through_runner_api() {
    let handler = RunnerApiHandler::default();

    let response = handler.handle(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "diag-1",
            "name": "remote.diagnose.system",
            "arguments": {}
        })),
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.body["type"], json!("tool_call"));
    assert_eq!(response.body["call_id"], json!("diag-1"));
    assert_eq!(response.body["success"], json!(true));
    assert_eq!(
        response.body["result"]["tool"],
        json!("remote.diagnose.system")
    );
    assert_eq!(
        response.body["result"]["system"]["os"],
        json!(std::env::consts::OS)
    );
}

#[test]
fn audit_recent_records_tool_browser_and_maintenance_events_in_order() {
    let handler = RunnerApiHandler::default();

    let diagnose = handler.handle(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "diag-audit-1",
            "name": "remote.diagnose.system",
            "arguments": {}
        })),
    );
    let browser = handler.handle(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "browser-audit-1",
            "name": "remote.browser.click",
            "arguments": {
                "session_id": "browser-session-1",
                "selector": "button[type=submit]"
            }
        })),
    );
    let maintenance = handler.handle(
        "POST",
        "/maintenance/plan",
        Some(json!({
            "action": "self_update",
            "dry_run": true,
            "idempotency_key": "audit-dry-run-1"
        })),
    );

    assert_eq!(diagnose.status, 200);
    assert_eq!(browser.status, 200);
    assert_eq!(maintenance.status, 200);

    let recent = handler.handle("GET", "/audit/recent", None);

    assert_eq!(recent.status, 200);
    assert_eq!(recent.body["type"], json!("audit_recent"));
    assert_eq!(recent.body["persistence"], json!("memory_scaffold"));
    assert_eq!(
        recent.body["audit"],
        json!([
            {
                "event": "tool_call",
                "status": "ok",
                "source": "runner",
                "metadata": {
                    "call_id": "diag-audit-1",
                    "tool": "remote.diagnose.system"
                }
            },
            {
                "event": "browser.action",
                "status": "blocked",
                "source": "runner",
                "metadata": {
                    "call_id": "browser-audit-1",
                    "tool": "remote.browser.click",
                    "error_code": "approval_required"
                }
            },
            {
                "event": "maintenance.execute",
                "status": "no_op",
                "reason": "dry_run"
            }
        ])
    );
}

#[test]
fn audit_recent_records_maintenance_nonce_consumed_replay_without_raw_secrets() {
    let handler = RunnerApiHandler::new_with_shell_runner_and_token(
        KaiRunner::with_shell_executor(FakeShellExecutor)
            .with_approval_nonce_manager(ShellApprovalNonceManager::new()),
        "test-token",
    );
    let nonce_response =
        handler.handle_with_bearer_token("POST", "/approval/nonce", None, Some("test-token"));
    let nonce = nonce_response.body["approval_nonce"].as_str().unwrap();
    let request = json!({
        "action": "uninstall",
        "dry_run": true,
        "execute": true,
        "approval": { "nonce": nonce }
    });

    let first = handler.handle_with_bearer_token(
        "POST",
        "/maintenance/plan",
        Some(request.clone()),
        Some("test-token"),
    );
    let second = handler.handle_with_bearer_token(
        "POST",
        "/maintenance/plan",
        Some(request),
        Some("test-token"),
    );

    assert_eq!(first.status, 400);
    assert_eq!(first.body["error"]["code"], json!("not_executable"));
    assert_eq!(second.status, 400);
    assert_eq!(second.body["error"]["code"], json!("approval_replayed"));

    let recent = handler.handle_with_bearer_token("GET", "/audit/recent", None, Some("test-token"));
    let audit_text = recent.body["audit"].to_string();

    assert_eq!(recent.status, 200);
    assert_eq!(
        recent.body["audit"],
        json!([
            {
                "event": "maintenance.approval_nonce",
                "status": "issued",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "issued"
                }
            },
            {
                "event": "maintenance.approval_nonce",
                "status": "consumed",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "consumed"
                }
            },
            {
                "event": "maintenance.execute",
                "status": "no_op",
                "reason": "not_executable"
            },
            {
                "event": "maintenance.approval_nonce",
                "status": "replay",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "replayed"
                }
            }
        ])
    );
    assert!(!audit_text.contains(nonce));
    assert!(!audit_text.contains("test-token"));
    assert!(!audit_text.contains("Bearer"));
}

#[test]
fn audit_recent_records_command_lease_lifecycle_without_raw_secrets() {
    let approvals = ShellApprovalNonceManager::new();
    let valid_lease = CommandLeaseEnvelope::new(
        "lease-secret-valid",
        "idem-secret-valid",
        CommandLeaseAction::new("remote.shell.exec", "echo command-secret", None::<String>),
    );
    let replay_lease = CommandLeaseEnvelope::new(
        "lease-secret-replay",
        "idem-secret-replay",
        CommandLeaseAction::new("remote.shell.exec", "echo replay-secret", None::<String>),
    );
    let expired_lease = CommandLeaseEnvelope::new(
        "lease-secret-expired",
        "idem-secret-expired",
        CommandLeaseAction::new("remote.shell.exec", "echo expired-secret", None::<String>),
    );
    let invalid_action_lease = CommandLeaseEnvelope::new(
        "lease-secret-invalid",
        "idem-secret-invalid",
        CommandLeaseAction::new("remote.shell.exec", "echo approved-secret", None::<String>),
    );
    approvals.create_lease(valid_lease.clone(), Some(Duration::from_secs(30)));
    approvals.create_lease(replay_lease.clone(), Some(Duration::from_secs(30)));
    approvals.create_lease(expired_lease.clone(), Some(Duration::from_secs(30)));
    approvals.expire("lease-secret-expired");
    approvals.create_lease(invalid_action_lease.clone(), Some(Duration::from_secs(30)));

    let handler = RunnerApiHandler::new_with_shell_runner_and_token(
        KaiRunner::with_shell_executor(FakeShellExecutor)
            .with_approval_nonce_manager(approvals),
        "lease-audit-bearer-token-secret",
    );

    let nonce_response = handler.handle_with_bearer_token(
        "POST",
        "/approval/nonce",
        None,
        Some("lease-audit-bearer-token-secret"),
    );
    let issued_nonce = nonce_response.body["approval_nonce"].as_str().unwrap();

    let valid = handler.handle_with_bearer_token(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "lease-valid",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo command-secret",
                "lease": valid_lease,
                "env": { "SAFE_ENV": "env-secret-value" }
            }
        })),
        Some("lease-audit-bearer-token-secret"),
    );
    let replay_first = handler.handle_with_bearer_token(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "lease-replay-first",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo replay-secret",
                "lease": replay_lease.clone()
            }
        })),
        Some("lease-audit-bearer-token-secret"),
    );
    let replay_second = handler.handle_with_bearer_token(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "lease-replay-second",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo replay-secret",
                "lease": replay_lease
            }
        })),
        Some("lease-audit-bearer-token-secret"),
    );
    let expired = handler.handle_with_bearer_token(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "lease-expired",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo expired-secret",
                "lease": expired_lease
            }
        })),
        Some("lease-audit-bearer-token-secret"),
    );
    let invalid = handler.handle_with_bearer_token(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "lease-invalid",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo tampered-secret",
                "lease": invalid_action_lease
            }
        })),
        Some("lease-audit-bearer-token-secret"),
    );

    assert_eq!(nonce_response.status, 200);
    assert_eq!(valid.body["success"], json!(true));
    assert_eq!(replay_first.body["success"], json!(true));
    assert_eq!(
        replay_second.body["result"]["error"]["code"],
        json!("approval_replayed")
    );
    assert_eq!(
        expired.body["result"]["error"]["code"],
        json!("approval_expired")
    );
    assert_eq!(
        invalid.body["result"]["error"]["code"],
        json!("approval_required")
    );

    let recent = handler.handle_with_bearer_token(
        "GET",
        "/audit/recent",
        None,
        Some("lease-audit-bearer-token-secret"),
    );
    let audit = recent.body["audit"].as_array().unwrap();
    let lease_events = audit
        .iter()
        .filter(|event| event["event"] == json!("shell.command_lease"))
        .cloned()
        .collect::<Vec<_>>();
    let audit_text = recent.body["audit"].to_string();

    assert_eq!(recent.status, 200);
    assert_eq!(
        audit.first(),
        Some(&json!({
            "event": "maintenance.approval_nonce",
            "status": "issued",
            "nonce": {
                "label": "approval_nonce",
                "status": "issued"
            }
        }))
    );
    assert_eq!(
        lease_events,
        vec![
            json!({
                "event": "shell.command_lease",
                "status": "accepted",
                "lease": {
                    "label": "command_lease",
                    "status": "accepted"
                },
                "metadata": {
                    "call_id": "lease-valid",
                    "tool": "remote.shell.exec"
                }
            }),
            json!({
                "event": "shell.command_lease",
                "status": "consumed",
                "lease": {
                    "label": "command_lease",
                    "status": "consumed"
                },
                "metadata": {
                    "call_id": "lease-valid",
                    "tool": "remote.shell.exec"
                }
            }),
            json!({
                "event": "shell.command_lease",
                "status": "accepted",
                "lease": {
                    "label": "command_lease",
                    "status": "accepted"
                },
                "metadata": {
                    "call_id": "lease-replay-first",
                    "tool": "remote.shell.exec"
                }
            }),
            json!({
                "event": "shell.command_lease",
                "status": "consumed",
                "lease": {
                    "label": "command_lease",
                    "status": "consumed"
                },
                "metadata": {
                    "call_id": "lease-replay-first",
                    "tool": "remote.shell.exec"
                }
            }),
            json!({
                "event": "shell.command_lease",
                "status": "replay_rejected",
                "lease": {
                    "label": "command_lease",
                    "status": "replayed"
                },
                "metadata": {
                    "call_id": "lease-replay-second",
                    "tool": "remote.shell.exec",
                    "error_code": "approval_replayed"
                }
            }),
            json!({
                "event": "shell.command_lease",
                "status": "expired_rejected",
                "lease": {
                    "label": "command_lease",
                    "status": "expired"
                },
                "metadata": {
                    "call_id": "lease-expired",
                    "tool": "remote.shell.exec",
                    "error_code": "approval_expired"
                }
            }),
            json!({
                "event": "shell.command_lease",
                "status": "invalid_action_rejected",
                "lease": {
                    "label": "command_lease",
                    "status": "invalid_action"
                },
                "metadata": {
                    "call_id": "lease-invalid",
                    "tool": "remote.shell.exec",
                    "error_code": "approval_required"
                }
            })
        ]
    );
    assert!(!audit_text.contains(issued_nonce));
    assert!(!audit_text.contains("lease-audit-bearer-token-secret"));
    assert!(!audit_text.contains("lease-secret"));
    assert!(!audit_text.contains("idem-secret"));
    assert!(!audit_text.contains("command-secret"));
    assert!(!audit_text.contains("replay-secret"));
    assert!(!audit_text.contains("expired-secret"));
    assert!(!audit_text.contains("approved-secret"));
    assert!(!audit_text.contains("tampered-secret"));
    assert!(!audit_text.contains("env-secret-value"));
    assert!(!audit_text.contains("SAFE_ENV"));
}

#[test]
fn audit_recent_trims_to_recent_32_events_in_stable_order() {
    let handler = RunnerApiHandler::default();

    for index in 0..35 {
        let response = handler.handle(
            "POST",
            "/tool-call",
            Some(json!({
                "call_id": format!("diag-audit-{index:02}"),
                "name": "remote.diagnose.system",
                "arguments": {}
            })),
        );

        assert_eq!(response.status, 200);
    }

    let recent = handler.handle("GET", "/audit/recent", None);
    let audit = recent.body["audit"]
        .as_array()
        .expect("audit recent must return an array");
    let call_ids = audit
        .iter()
        .map(|event| event["metadata"]["call_id"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(recent.status, 200);
    assert_eq!(audit.len(), 32);
    assert_eq!(call_ids.first(), Some(&"diag-audit-03"));
    assert_eq!(call_ids.last(), Some(&"diag-audit-34"));
    assert_eq!(
        call_ids,
        (3..35)
            .map(|index| format!("diag-audit-{index:02}"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn audit_recent_redacts_bearer_nonce_and_tool_arguments() {
    let handler = RunnerApiHandler::new_with_shell_runner_and_token(
        KaiRunner::with_shell_executor(FakeShellExecutor)
            .with_approval_nonce_manager(ShellApprovalNonceManager::new()),
        "audit-bearer-token-secret",
    );
    let nonce_response = handler.handle_with_bearer_token(
        "POST",
        "/approval/nonce",
        None,
        Some("audit-bearer-token-secret"),
    );
    let nonce = nonce_response.body["approval_nonce"].as_str().unwrap();

    let response = handler.handle_with_bearer_token(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "shell-audit-redaction",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo raw-tool-argument-secret",
                "approval_nonce": nonce,
                "env": {
                    "RUNNER_SECRET": "nested-tool-argument-secret"
                }
            }
        })),
        Some("audit-bearer-token-secret"),
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.body["success"], json!(true));

    let recent = handler.handle_with_bearer_token(
        "GET",
        "/audit/recent",
        None,
        Some("audit-bearer-token-secret"),
    );
    let audit_text = recent.body["audit"].to_string();

    assert_eq!(recent.status, 200);
    assert_eq!(
        recent.body["audit"],
        json!([
            {
                "event": "tool_call",
                "status": "ok",
                "source": "runner",
                "metadata": {
                    "call_id": "shell-audit-redaction",
                    "tool": "remote.shell.exec"
                }
            }
        ])
    );
    assert!(!audit_text.contains("audit-bearer-token-secret"));
    assert!(!audit_text.contains("Bearer"));
    assert!(!audit_text.contains(nonce));
    assert!(!audit_text.contains("approval_nonce"));
    assert!(!audit_text.contains("echo raw-tool-argument-secret"));
    assert!(!audit_text.contains("nested-tool-argument-secret"));
}

#[test]
fn tool_call_route_returns_blocked_shell_result_without_executing() {
    let handler = RunnerApiHandler::default();

    let response = handler.handle(
        "POST",
        "/tool-call",
        Some(json!({
            "call_id": "shell-1",
            "name": "remote.shell.exec",
            "arguments": { "command": "echo should-not-run" }
        })),
    );

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        json!({
            "type": "tool_call",
            "call_id": "shell-1",
            "success": false,
            "result": {
                "tool": "remote.shell.exec",
                "status": "error",
                "error": {
                    "code": "blocked",
                    "message": "kai-runner scaffold does not execute local shell commands"
                }
            }
        })
    );
}

#[test]
fn mcp_call_route_maps_mobile_core_body_to_runner_tool_call() {
    let handler = RunnerApiHandler::default();

    let response = handler.handle(
        "POST",
        "/mcp/call",
        Some(json!({
            "call_id": "mcp-1",
            "arguments": {
                "server": "demo",
                "tool": "lookup",
                "arguments": { "query": "rust" }
            }
        })),
    );

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        json!({
            "type": "tool_call",
            "call_id": "mcp-1",
            "success": false,
            "result": {
                "tool": "remote.mcp.call",
                "status": "error",
                "error": {
                    "code": "unsupported",
                    "message": "remote.mcp.call is not implemented in the kai-runner scaffold"
                }
            }
        })
    );
}

#[test]
fn mcp_enabled_handler_routes_registered_mcp_call() {
    let handler = RunnerApiHandler::new_with_mcp_runner(KaiRunner::with_mcp_handler(
        "local",
        "echo",
        FakeMcpHandler,
    ));

    let response = handler.handle(
        "POST",
        "/mcp/call",
        Some(json!({
            "call_id": "mcp-registered-1",
            "arguments": {
                "server": "local",
                "tool": "echo",
                "arguments": { "text": "hello" }
            }
        })),
    );

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        json!({
            "type": "tool_call",
            "call_id": "mcp-registered-1",
            "success": true,
            "result": {
                "tool": "remote.mcp.call",
                "status": "ok",
                "data": {
                    "server": "local",
                    "tool": "echo",
                    "echo": { "text": "hello" }
                }
            }
        })
    );
}

#[test]
fn mcp_enabled_handler_returns_unknown_mcp_tool() {
    let handler = RunnerApiHandler::new_with_mcp_runner(KaiRunner::with_mcp_handler(
        "local",
        "echo",
        FakeMcpHandler,
    ));

    let response = handler.handle(
        "POST",
        "/mcp/call",
        Some(json!({
            "call_id": "mcp-missing-1",
            "arguments": {
                "server": "local",
                "tool": "missing",
                "arguments": {}
            }
        })),
    );

    assert_eq!(response.status, 200);
    assert_eq!(response.body["type"], json!("tool_call"));
    assert_eq!(response.body["call_id"], json!("mcp-missing-1"));
    assert_eq!(response.body["success"], json!(false));
    assert_eq!(response.body["result"]["tool"], json!("remote.mcp.call"));
    assert_eq!(response.body["result"]["status"], json!("error"));
    assert_eq!(
        response.body["result"]["error"]["code"],
        json!("unknown_mcp_tool")
    );
}

#[test]
fn mcp_call_route_rejects_missing_call_id() {
    let handler = RunnerApiHandler::default();

    let response = handler.handle(
        "POST",
        "/mcp/call",
        Some(json!({
            "arguments": {}
        })),
    );

    assert_eq!(response.status, 400);
    assert_eq!(
        response.body,
        json!({
            "error": {
                "code": "bad_request",
                "message": "invalid mcp-call JSON body"
            }
        })
    );
}

#[test]
fn unknown_routes_return_json_not_found() {
    let handler = RunnerApiHandler::default();

    let response = handler.handle("GET", "/missing", None);

    assert_eq!(response.status, 404);
    assert_eq!(
        response.body,
        json!({
            "error": {
                "code": "not_found",
                "message": "GET /missing is not supported by kai-runner"
            }
        })
    );
}
