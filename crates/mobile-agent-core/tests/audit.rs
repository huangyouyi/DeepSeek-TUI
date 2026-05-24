use deepseek_mobile_agent_core::{
    ApprovalDecision, AuditEntry, AuditLog, RemoteToolCall, RemoteToolName, RemoteToolOutput,
};
use serde_json::json;

#[test]
fn audit_log_appends_supported_entry_types_with_sequential_seq() {
    let mut audit = AuditLog::default();

    audit.append_bootstrap_command_at(
        "2026-05-23T12:00:00Z",
        "session-1",
        "Run diagnostics",
        "uname -a\nsw_vers",
    );
    audit.append_bootstrap_output_at(
        "2026-05-23T12:00:01Z",
        "session-1",
        "Captured diagnostics",
        "Darwin\nmacOS 15.5",
    );
    audit.append_remote_tool_call_at(
        "2026-05-23T12:00:02Z",
        "session-1",
        &RemoteToolCall {
            call_id: "call-1".to_string(),
            name: RemoteToolName::ShellExec,
            arguments: json!({ "command": "pwd" }),
        },
    );
    audit.append_remote_tool_output_at(
        "2026-05-23T12:00:03Z",
        "session-1",
        &RemoteToolOutput {
            call_id: "call-1".to_string(),
            success: true,
            result: json!({ "stdout": "/tmp\n" }),
        },
    );
    audit.append_approval_decision_at(
        "2026-05-23T12:00:04Z",
        "session-1",
        "approval-1",
        "call-1",
        ApprovalDecision::Approved,
        "User approved shell command",
    );

    let entries = audit.entries();

    assert_eq!(
        entries.iter().map(|entry| entry.seq).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );

    assert_eq!(entries[0].kind, "bootstrap");
    assert_eq!(entries[0].action, "command");
    assert_eq!(entries[0].created_at, "2026-05-23T12:00:00Z");
    assert_eq!(entries[0].summary, "Run diagnostics");
    assert_eq!(entries[0].detail, "uname -a\nsw_vers");
    assert_eq!(entries[0].session_id.as_deref(), Some("session-1"));
    assert_eq!(entries[0].call_id, None);
    assert_eq!(entries[0].approval_id, None);

    assert_eq!(entries[2].kind, "remote_tool");
    assert_eq!(entries[2].action, "call");
    assert_eq!(entries[2].summary, "remote.shell.exec");
    assert_eq!(entries[2].detail, r#"{"command":"pwd"}"#);
    assert_eq!(entries[2].call_id.as_deref(), Some("call-1"));

    assert_eq!(entries[3].kind, "remote_tool");
    assert_eq!(entries[3].action, "output");
    assert_eq!(entries[3].summary, "success");
    assert_eq!(entries[3].detail, r#"{"stdout":"/tmp\n"}"#);
    assert_eq!(entries[3].call_id.as_deref(), Some("call-1"));

    assert_eq!(entries[4].kind, "approval");
    assert_eq!(entries[4].action, "approved");
    assert_eq!(entries[4].summary, "User approved shell command");
    assert_eq!(entries[4].approval_id.as_deref(), Some("approval-1"));
    assert_eq!(entries[4].call_id.as_deref(), Some("call-1"));
}

#[test]
fn audit_entry_supports_json_round_trip() {
    let entry = AuditEntry {
        seq: 7,
        kind: "approval".to_string(),
        action: "denied".to_string(),
        created_at: "2026-05-23T12:01:00Z".to_string(),
        summary: "User denied package install".to_string(),
        detail: "Not needed".to_string(),
        call_id: Some("call-7".to_string()),
        approval_id: Some("approval-7".to_string()),
        session_id: Some("session-1".to_string()),
    };

    let encoded = serde_json::to_string(&entry).expect("entry should serialize");
    let decoded: AuditEntry = serde_json::from_str(&encoded).expect("entry should deserialize");

    assert_eq!(decoded, entry);
}

#[test]
fn audit_log_supports_json_round_trip() {
    let mut audit = AuditLog::default();
    audit.append_bootstrap_command_at(
        "2026-05-23T12:02:00Z",
        "session-2",
        "Install command",
        "brew install jq",
    );

    let encoded = serde_json::to_string(&audit).expect("audit log should serialize");
    let decoded: AuditLog = serde_json::from_str(&encoded).expect("audit log should deserialize");

    assert_eq!(decoded.entries(), audit.entries());
}
