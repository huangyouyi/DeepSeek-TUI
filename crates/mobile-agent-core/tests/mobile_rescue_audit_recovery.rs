use deepseek_mobile_agent_core::{
    ApprovalGate, AuditLog, InMemorySnapshotStore, PendingApprovalSnapshot, PendingTurnSnapshot,
    PendingTurnStore, RemoteToolCall, RemoteToolName, RiskLevel, SqliteSnapshotStore,
};
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn approval_for(
    call_id: &str,
    name: RemoteToolName,
    arguments: serde_json::Value,
) -> PendingApprovalSnapshot {
    let call = RemoteToolCall {
        call_id: call_id.to_string(),
        name,
        arguments,
    };
    let approval = ApprovalGate
        .evaluate(&call)
        .expect("call should require approval");

    PendingApprovalSnapshot::from_request(
        &approval,
        format!(
            "Review {} before running it on the rescued device",
            approval.tool_name
        ),
    )
}

fn sqlite_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("deepseek-mobile-rescue-audit-recovery-{nanos}.db"))
}

#[test]
fn audit_records_shell_browser_and_maintenance_approvals_with_structured_metadata() {
    let mut audit = AuditLog::default();

    let shell = ApprovalGate
        .evaluate(&RemoteToolCall {
            call_id: "shell-danger".to_string(),
            name: RemoteToolName::ShellExec,
            arguments: json!({ "command": "sudo rm -rf /tmp/deepseek-cache" }),
        })
        .expect("destructive shell command should require approval");
    let browser = ApprovalGate
        .evaluate(&RemoteToolCall {
            call_id: "browser-click".to_string(),
            name: RemoteToolName::BrowserClick,
            arguments: json!({ "session_id": "browser-session-1", "selector": "#repair" }),
        })
        .expect("browser click should require approval");
    let maintenance = ApprovalGate
        .evaluate(&RemoteToolCall {
            call_id: "maintenance-mcp".to_string(),
            name: RemoteToolName::McpCall,
            arguments: json!({
                "server": "mobile-maintenance",
                "method": "repair_homebrew",
                "params": { "dry_run": false }
            }),
        })
        .expect("maintenance MCP call should require approval");

    audit.append_approval_required_at(
        "2026-05-24T10:00:00Z",
        "mobile-session-1",
        &shell,
        "Approve privileged shell cleanup",
    );
    audit.append_approval_required_at(
        "2026-05-24T10:00:01Z",
        "mobile-session-1",
        &browser,
        "Approve browser repair click",
    );
    audit.append_approval_required_at(
        "2026-05-24T10:00:02Z",
        "mobile-session-1",
        &maintenance,
        "Approve maintenance MCP repair",
    );

    let entries = audit.entries();
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().all(|entry| entry.kind == "approval"));
    assert!(entries.iter().all(|entry| entry.action == "required"));

    let shell_detail: serde_json::Value =
        serde_json::from_str(&entries[0].detail).expect("detail should be structured JSON");
    assert_eq!(
        entries[0].approval_id.as_deref(),
        Some("approval-shell-danger")
    );
    assert_eq!(entries[0].call_id.as_deref(), Some("shell-danger"));
    assert_eq!(shell_detail["tool_name"], "shell_exec");
    assert_eq!(shell_detail["risk"]["level"], "High");
    assert_eq!(
        shell_detail["risk"]["reason"],
        "command can make privileged, destructive, or security changes"
    );
    assert_eq!(
        shell_detail["user_visible_summary"],
        "Approve privileged shell cleanup"
    );

    let browser_detail: serde_json::Value =
        serde_json::from_str(&entries[1].detail).expect("detail should be structured JSON");
    assert_eq!(
        entries[1].approval_id.as_deref(),
        Some("approval-browser-click")
    );
    assert_eq!(browser_detail["tool_name"], "browser_click");
    assert_eq!(browser_detail["risk"]["level"], "Medium");
    assert_eq!(
        browser_detail["user_visible_summary"],
        "Approve browser repair click"
    );

    let maintenance_detail: serde_json::Value =
        serde_json::from_str(&entries[2].detail).expect("detail should be structured JSON");
    assert_eq!(
        entries[2].approval_id.as_deref(),
        Some("approval-maintenance-mcp")
    );
    assert_eq!(maintenance_detail["tool_name"], "mcp_call");
    assert_eq!(maintenance_detail["risk"]["level"], "Medium");
    assert_eq!(
        maintenance_detail["arguments"]["server"],
        "mobile-maintenance"
    );
    assert_eq!(
        maintenance_detail["user_visible_summary"],
        "Approve maintenance MCP repair"
    );
}

#[test]
fn pending_turn_recovery_preserves_approval_risk_tool_name_and_user_visible_summary() {
    let snapshot = PendingTurnSnapshot {
        session_id: "mobile-session-1".to_string(),
        turn_id: "turn-rescue-7".to_string(),
        prompt: "repair the phone-side Homebrew install".to_string(),
        created_at: "2026-05-24T10:05:00Z".to_string(),
        pending_approval_ids: vec!["approval-shell-danger".to_string()],
        pending_tool_call_ids: vec!["shell-danger".to_string()],
        pending_approvals: vec![approval_for(
            "shell-danger",
            RemoteToolName::ShellExec,
            json!({ "command": "sudo launchctl kickstart system/com.example.fix" }),
        )],
    };
    let mut store = InMemorySnapshotStore::default();

    store.save_pending_turn(snapshot.clone());
    let restored = store
        .load_pending_turn("mobile-session-1", "turn-rescue-7")
        .expect("pending turn should restore");

    assert_eq!(restored.pending_approval_ids, vec!["approval-shell-danger"]);
    assert_eq!(restored.pending_tool_call_ids, vec!["shell-danger"]);
    assert_eq!(restored.pending_approvals.len(), 1);
    assert_eq!(
        restored.pending_approvals[0].approval_id,
        "approval-shell-danger"
    );
    assert_eq!(restored.pending_approvals[0].call_id, "shell-danger");
    assert_eq!(restored.pending_approvals[0].tool_name, "shell_exec");
    assert_eq!(restored.pending_approvals[0].risk_level, RiskLevel::High);
    assert_eq!(
        restored.pending_approvals[0].risk_reason,
        "command can make privileged, destructive, or security changes"
    );
    assert_eq!(
        restored.pending_approvals[0].user_visible_summary,
        "Review shell_exec before running it on the rescued device"
    );
}

#[test]
fn sqlite_pending_turn_recovery_preserves_browser_approval_details_after_reopen() {
    let path = sqlite_path();
    let snapshot = PendingTurnSnapshot {
        session_id: "mobile-session-2".to_string(),
        turn_id: "turn-browser-1".to_string(),
        prompt: "finish web console recovery".to_string(),
        created_at: "2026-05-24T10:06:00Z".to_string(),
        pending_approval_ids: vec!["approval-browser-click".to_string()],
        pending_tool_call_ids: vec!["browser-click".to_string()],
        pending_approvals: vec![approval_for(
            "browser-click",
            RemoteToolName::BrowserClick,
            json!({ "session_id": "browser-session-2", "selector": "#continue" }),
        )],
    };
    let mut store = SqliteSnapshotStore::open(&path).expect("sqlite store should open");

    store.save_pending_turn(snapshot.clone());
    drop(store);

    let store = SqliteSnapshotStore::open(&path).expect("sqlite store should reopen");
    let restored = store
        .load_pending_turn("mobile-session-2", "turn-browser-1")
        .expect("pending turn should restore");

    assert_eq!(restored.pending_approvals, snapshot.pending_approvals);
    assert_eq!(restored.pending_approvals[0].tool_name, "browser_click");
    assert_eq!(restored.pending_approvals[0].risk_level, RiskLevel::Medium);
    assert_eq!(
        restored.pending_approvals[0].user_visible_summary,
        "Review browser_click before running it on the rescued device"
    );

    let _ = std::fs::remove_file(path);
}
