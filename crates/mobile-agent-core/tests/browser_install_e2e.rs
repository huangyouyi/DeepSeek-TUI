use std::collections::VecDeque;

use deepseek_mobile_agent_core::audit::AuditLog;
use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName};
use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, TransportError,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn browser_call(call_id: &str, name: RemoteToolName, arguments: Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name,
        arguments,
    }
}

#[test]
fn browser_assisted_install_records_download_approval_audit_without_network() {
    let raw_nonce = "raw-browser-approval-nonce";
    let bearer_token = "runner-browser-token";
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", bearer_token);
    let mut captured_requests: Vec<RunnerHttpRequestSpec> = Vec::new();
    let mut responses = VecDeque::from([
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "browser-open-install",
            "success": true,
            "result": {
                "tool": "remote.browser.open",
                "status": "ok",
                "data": {
                    "status": "planned",
                    "session_id": "browser-session-1",
                    "url": "https://example.com/downloads/pro-editor",
                    "next_approval_required": false
                }
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "browser-extract-install",
            "success": true,
            "result": {
                "tool": "remote.browser.extract_text",
                "status": "ok",
                "data": {
                    "session_id": "browser-session-1",
                    "url": "https://example.com/downloads/pro-editor",
                    "text": "Pro Editor\nDownload for macOS"
                }
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "browser-click-download",
            "success": false,
            "result": {
                "tool": "remote.browser.click",
                "status": "error",
                "error": {
                    "code": "approval_required",
                    "message": "browser.click requires approval"
                },
                "data": {
                    "session_id": "browser-session-1",
                    "selector": "a[data-action=download]",
                    "label": "Download for macOS",
                    "action": "download",
                    "approval_required": true,
                    "risk": "state_changing",
                    "metadata": {
                        "scenario": "install-pro-editor",
                        "nonce": raw_nonce,
                        "authorization": format!("Bearer {bearer_token}")
                    },
                    "audit": [
                        {
                            "type": "browser_action_blocked",
                            "tool": "remote.browser.click",
                            "action": "download",
                            "session_id": "browser-session-1",
                            "selector": "a[data-action=download]",
                            "reason": "approval_required",
                            "raw_nonce": raw_nonce
                        }
                    ]
                }
            }
        })),
    ]);
    let backend = RunnerHttpClosureBackend::new(|request: RunnerHttpRequestSpec| {
        captured_requests.push(request);
        responses
            .pop_front()
            .ok_or_else(|| TransportError::Failed("unexpected runner request".to_string()))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    let open = client
        .execute_tool_call_with_browser_audit(browser_call(
            "browser-open-install",
            RemoteToolName::BrowserOpen,
            json!({
                "url": "https://example.com/downloads/pro-editor",
                "profile": "installer"
            }),
        ))
        .unwrap();
    let extract = client
        .execute_tool_call_with_browser_audit(browser_call(
            "browser-extract-install",
            RemoteToolName::BrowserExtractText,
            json!({
                "session_id": "browser-session-1",
                "selector": "main"
            }),
        ))
        .unwrap();
    let click = client
        .execute_tool_call_with_browser_audit(browser_call(
            "browser-click-download",
            RemoteToolName::BrowserClick,
            json!({
                "session_id": "browser-session-1",
                "selector": "a[data-action=download]",
                "label": "Download for macOS",
                "action": "download",
                "metadata": { "scenario": "install-pro-editor" }
            }),
        ))
        .unwrap();
    drop(client);

    assert!(open.output.success);
    assert_eq!(
        open.output.result["data"]["session_id"],
        json!("browser-session-1")
    );
    assert!(open.audit.is_empty());
    assert!(extract.output.success);
    assert_eq!(
        extract.output.result["data"]["text"],
        json!("Pro Editor\nDownload for macOS")
    );
    assert!(extract.audit.is_empty());

    assert!(!click.output.success);
    assert_eq!(
        click.output.result["error"]["code"],
        json!("approval_required")
    );
    assert_eq!(click.audit.len(), 1);
    assert_eq!(click.audit[0].record_type, "browser_action_blocked");
    assert_eq!(click.audit[0].tool, "remote.browser.click");
    assert_eq!(click.audit[0].action.as_deref(), Some("download"));
    assert_eq!(click.audit[0].risk.as_deref(), Some("state_changing"));
    assert_eq!(click.audit[0].approval_required, Some(true));
    assert_eq!(click.audit[0].reason.as_deref(), Some("approval_required"));
    assert_eq!(
        click.audit[0].metadata["scenario"],
        json!("install-pro-editor")
    );
    let returned_output = serde_json::to_string(&click.output).expect("output should serialize");
    assert!(!returned_output.contains(raw_nonce));
    assert!(!returned_output.contains(bearer_token));

    let mut audit = AuditLog::default();
    for record in &click.audit {
        audit.append_runner_browser_audit_at("2026-05-24T12:00:00Z", "mobile-session-1", record);
    }

    let rendered = serde_json::to_string(&audit).expect("audit log should serialize");
    assert!(rendered.contains("browser_action_blocked"));
    assert!(rendered.contains("download"));
    assert!(rendered.contains("state_changing"));
    assert!(rendered.contains("approval_required"));
    assert!(!rendered.contains(raw_nonce));
    assert!(!rendered.contains(bearer_token));

    assert_eq!(captured_requests.len(), 3);
    assert!(captured_requests.iter().all(|request| {
        request.url == "https://runner.example/mobile/tool-call"
            && request.auth_scheme() == Some("Bearer")
    }));
    assert!(!format!("{captured_requests:?}").contains(bearer_token));
}

#[test]
fn parses_sensitive_browser_action_risk_and_approval_metadata() {
    for action in ["download", "login", "submit"] {
        let parsed = RunnerHttpTransport::parse_tool_call_response_with_browser_audit(json!({
            "type": "tool_call",
            "call_id": format!("browser-click-{action}"),
            "success": false,
            "result": {
                "tool": "remote.browser.click",
                "status": "error",
                "error": { "code": "approval_required" },
                "data": {
                    "session_id": "browser-session-1",
                    "selector": format!("[data-action={action}]"),
                    "action": action,
                    "approval_required": true,
                    "risk": "state_changing",
                    "audit": [{
                        "type": "browser_action_blocked",
                        "tool": "remote.browser.click",
                        "action": action,
                        "session_id": "browser-session-1",
                        "selector": format!("[data-action={action}]"),
                        "reason": "approval_required"
                    }]
                }
            }
        }))
        .unwrap();

        assert_eq!(parsed.audit.len(), 1);
        assert_eq!(parsed.audit[0].action.as_deref(), Some(action));
        assert_eq!(parsed.audit[0].risk.as_deref(), Some("state_changing"));
        assert_eq!(parsed.audit[0].approval_required, Some(true));
        assert_eq!(parsed.audit[0].reason.as_deref(), Some("approval_required"));
    }
}
