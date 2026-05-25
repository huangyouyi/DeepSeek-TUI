use deepseek_mobile_agent_core::connection::RunnerPairingBootstrapInput;
use deepseek_mobile_agent_core::{
    AuditLog, BootstrapSession, ConnectionProfile, ConnectionStatus, ExecutionMode, MobileEvent,
    MobileEventKind, MobileSession, RemoteToolName,
};
use pretty_assertions::assert_eq;
use serde_json::Value;

#[test]
fn bootstrap_profile_upgrades_to_runner_from_reported_tools() {
    let profile = ConnectionProfile::bootstrap("phone-1", "Phone runner");

    let upgraded = profile
        .upgrade_to_runner_from_report([
            "remote.shell.exec",
            "remote.mcp.call",
            "remote.unknown",
            "remote.shell.exec",
            "remote.browser.open",
        ])
        .expect("runner report should contain valid tools");

    assert_eq!(upgraded.id, "phone-1");
    assert_eq!(upgraded.label, "Phone runner");
    assert_eq!(upgraded.status, ConnectionStatus::Connected);
    assert_eq!(upgraded.mode(), ExecutionMode::Runner);
    assert_eq!(
        upgraded.capabilities.tools,
        vec![
            RemoteToolName::ShellExec,
            RemoteToolName::McpCall,
            RemoteToolName::BrowserOpen,
        ]
    );
    assert!(upgraded.allows_tool(RemoteToolName::ShellExec));
    assert!(upgraded.allows_tool(RemoteToolName::McpCall));
    assert!(!upgraded.allows_tool(RemoteToolName::FileWrite));
}

#[test]
fn unreachable_profile_can_upgrade_to_connected_runner() {
    let upgraded = ConnectionProfile::unreachable("phone-2", "Offline phone")
        .upgrade_to_runner_from_report(["remote.file.read"])
        .expect("runner report should contain a valid tool");

    assert_eq!(upgraded.id, "phone-2");
    assert_eq!(upgraded.label, "Offline phone");
    assert_eq!(upgraded.status, ConnectionStatus::Connected);
    assert_eq!(upgraded.mode(), ExecutionMode::Runner);
    assert_eq!(upgraded.capabilities.tools, vec![RemoteToolName::FileRead]);
}

#[test]
fn report_without_valid_runner_tools_returns_error_and_keeps_bootstrap_available() {
    let profile = ConnectionProfile::bootstrap("phone-3", "Manual bootstrap");

    let error = profile
        .clone()
        .upgrade_to_runner_from_report(["remote.unknown", "remote.file.delete"])
        .expect_err("upgrade should fail without any valid runner tools");

    assert_eq!(
        error.to_string(),
        "runner capability report did not include any recognized tools"
    );
    assert_eq!(profile.status, ConnectionStatus::ManualBootstrap);
    assert_eq!(profile.mode(), ExecutionMode::Bootstrap);
    assert!(profile.allows_tool(RemoteToolName::BootstrapGuide));
}

#[test]
fn pairing_bootstrap_input_upgrades_to_runner_profile_with_redacted_metadata() {
    let profile = ConnectionProfile::bootstrap("phone-4", "Pairing bootstrap");
    let pairing_token = "pair-secret-123";

    let upgraded = profile
        .upgrade_to_runner_from_pairing_bootstrap(RunnerPairingBootstrapInput::new(
            " http://127.0.0.1:39117/mobile/ ",
            pairing_token,
            r#"{
              "capabilities": {
                "tools": [
                  "remote.shell.exec",
                  "remote.file.read",
                  "remote.browser.extract_text",
                  "remote.unknown"
                ]
              }
            }"#,
        ))
        .expect("pairing bootstrap report should upgrade to runner");

    let metadata = upgraded
        .runner_metadata
        .as_ref()
        .expect("runner upgrade should store redacted metadata");

    assert_eq!(upgraded.status, ConnectionStatus::Connected);
    assert_eq!(upgraded.mode(), ExecutionMode::Runner);
    assert_eq!(
        upgraded.capabilities.tools,
        vec![
            RemoteToolName::ShellExec,
            RemoteToolName::FileRead,
            RemoteToolName::BrowserExtractText,
        ]
    );
    assert_eq!(metadata.endpoint, "http://127.0.0.1:39117/mobile");
    assert!(metadata.pairing_token_present);
    assert_eq!(metadata.pairing_token_label, "<redacted>");
    assert!(!format!("{upgraded:?}").contains(pairing_token));
}

#[test]
fn upgraded_runner_profile_can_emit_session_event_and_audit_entry_without_token_leak() {
    let pairing_token = "pair-secret-456";
    let upgraded = ConnectionProfile::bootstrap("phone-5", "Pairing bootstrap")
        .upgrade_to_runner_from_pairing_bootstrap(RunnerPairingBootstrapInput::new(
            "https://runner.example/pair",
            pairing_token,
            "remote.shell.exec remote.mcp.call",
        ))
        .expect("pairing bootstrap report should upgrade to runner");

    let mut session = MobileSession {
        id: "session-1".to_string(),
        title: "phone rescue".to_string(),
        bootstrap: BootstrapSession::default(),
        events: vec![MobileEvent {
            seq: 1,
            kind: MobileEventKind::SessionCreated,
            payload: serde_json::json!({ "session_id": "session-1" }),
        }],
    };
    session
        .events
        .push(upgraded.runner_upgrade_event(2, &session.id));

    assert_eq!(session.events[1].seq, 2);
    assert_eq!(
        session.events[1].kind,
        MobileEventKind::BootstrapStepCreated
    );
    assert_eq!(
        session.events[1].payload["action"],
        "runner_profile_upgraded"
    );
    assert_eq!(session.events[1].payload["connection_id"], "phone-5");
    assert_eq!(
        session.events[1].payload["runner"]["endpoint"],
        "https://runner.example/pair"
    );
    assert_eq!(
        session.events[1].payload["runner"]["pairing_token_present"],
        true
    );
    assert_eq!(
        session.events[1].payload["runner"]["pairing_token_label"],
        "<redacted>"
    );
    assert!(
        !session.events[1]
            .payload
            .to_string()
            .contains(pairing_token)
    );

    let mut audit = AuditLog::default();
    upgraded.append_runner_upgrade_audit_at(&mut audit, "2026-05-24T10:00:00Z", &session.id);

    let entry = audit
        .entries()
        .first()
        .expect("audit entry should be appended");
    assert_eq!(entry.kind, "connection");
    assert_eq!(entry.action, "runner_profile_upgraded");
    assert_eq!(entry.session_id.as_deref(), Some("session-1"));
    assert!(!entry.detail.contains(pairing_token));

    let detail: Value = serde_json::from_str(&entry.detail).expect("detail should be JSON");
    assert_eq!(detail["runner"]["endpoint"], "https://runner.example/pair");
    assert_eq!(detail["runner"]["pairing_token_present"], true);
    assert_eq!(detail["runner"]["pairing_token_label"], "<redacted>");
    assert_eq!(
        detail["capabilities"]["tools"],
        serde_json::json!(["remote.shell.exec", "remote.mcp.call"])
    );
}
