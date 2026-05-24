use std::collections::VecDeque;

use deepseek_mobile_agent_core::audit::{AuditEntry, AuditLog};
use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, TransportError,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn parse_recent_entries(body: Value) -> Vec<AuditEntry> {
    serde_json::from_value::<Vec<AuditEntry>>(body["entries"].clone())
        .expect("runner recent audit entries should parse")
}

#[test]
fn pulls_runner_recent_audit_and_merges_maintenance_browser_and_tool_entries_without_network() {
    let raw_nonce = "raw-runner-approval-nonce";
    let bearer_token = "runner-audit-bearer-token";
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", bearer_token);
    let maintenance_detail = json!({
        "event": "maintenance.approval_nonce",
        "status": "consumed",
        "nonce": {
            "label": "approval_nonce",
            "status": "consumed",
            "raw": raw_nonce
        },
        "authorization": format!("Bearer {bearer_token}")
    })
    .to_string();
    let browser_detail = json!({
        "type": "browser_action_blocked",
        "tool": "remote.browser.click",
        "action": "download",
        "session_id": "browser-session-1",
        "selector": "a[data-action=download]",
        "reason": "approval_required",
        "metadata": {
            "scenario": "install-pro-editor",
            "nonce": raw_nonce,
            "authorization": format!("Bearer {bearer_token}")
        }
    })
    .to_string();
    let tool_detail = json!({
        "tool": "remote.shell.exec",
        "stdout": "ok\n",
        "stderr": "",
        "token": bearer_token
    })
    .to_string();
    let mut captured_requests: Vec<RunnerHttpRequestSpec> = Vec::new();
    let mut responses = VecDeque::from([RunnerHttpResponseSpec::ok_json(json!({
        "entries": [
            {
                "seq": 42,
                "kind": "runner_maintenance",
                "action": "approval_nonce",
                "created_at": "2026-05-24T13:00:00Z",
                "summary": "maintenance approval nonce consumed",
                "detail": maintenance_detail,
                "call_id": null,
                "approval_id": null,
                "session_id": "runner-session-1"
            },
            {
                "seq": 43,
                "kind": "runner_browser",
                "action": "download",
                "created_at": "2026-05-24T13:00:01Z",
                "summary": "browser download approval_required",
                "detail": browser_detail,
                "call_id": null,
                "approval_id": null,
                "session_id": "runner-session-1"
            },
            {
                "seq": 44,
                "kind": "remote_tool",
                "action": "output",
                "created_at": "2026-05-24T13:00:02Z",
                "summary": "success",
                "detail": tool_detail,
                "call_id": "call-shell-1",
                "approval_id": null,
                "session_id": "runner-session-1"
            }
        ]
    }))]);
    let backend = RunnerHttpClosureBackend::new(|request: RunnerHttpRequestSpec| {
        captured_requests.push(request);
        responses
            .pop_front()
            .ok_or_else(|| TransportError::Failed("unexpected audit request".to_string()))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    let body = client
        .fetch_recent_audit()
        .expect("recent audit request should stay in-process");
    drop(client);
    let recent_entries = parse_recent_entries(body);

    let mut audit = AuditLog::default();
    audit.append_bootstrap_command_at(
        "2026-05-24T12:59:59Z",
        "mobile-session-1",
        "existing local bootstrap",
        "diagnose runner",
    );
    audit.append_runner_recent_entries("mobile-session-1", recent_entries);

    assert_eq!(captured_requests.len(), 1);
    assert_eq!(captured_requests[0].method, "GET");
    assert_eq!(
        captured_requests[0].url,
        "https://runner.example/mobile/audit/recent"
    );
    assert_eq!(captured_requests[0].body, Value::Null);
    assert_eq!(captured_requests[0].auth_scheme(), Some("Bearer"));
    assert_eq!(
        captured_requests[0].authorization_header_value(),
        Some(format!("Bearer {bearer_token}"))
    );
    assert!(!format!("{captured_requests:?}").contains(bearer_token));

    let entries = audit.entries();
    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.seq, entry.kind.as_str(), entry.action.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (1, "bootstrap", "command"),
            (2, "runner_maintenance", "approval_nonce"),
            (3, "runner_browser", "download"),
            (4, "remote_tool", "output"),
        ]
    );
    assert_eq!(entries[1].created_at, "2026-05-24T13:00:00Z");
    assert_eq!(entries[2].created_at, "2026-05-24T13:00:01Z");
    assert_eq!(entries[3].call_id.as_deref(), Some("call-shell-1"));
    assert!(entries[1].detail.contains("approval_nonce"));
    assert!(entries[2].detail.contains("install-pro-editor"));
    assert!(entries[3].detail.contains("remote.shell.exec"));
    assert!(
        entries
            .iter()
            .all(|entry| { entry.session_id.as_deref() == Some("mobile-session-1") })
    );

    let rendered = serde_json::to_string(&audit).expect("merged audit log should serialize");
    assert!(rendered.contains("runner_maintenance"));
    assert!(rendered.contains("runner_browser"));
    assert!(rendered.contains("remote_tool"));
    assert!(!rendered.contains(raw_nonce));
    assert!(!rendered.contains(bearer_token));
}
