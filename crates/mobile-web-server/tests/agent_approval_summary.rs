use std::sync::{Arc, Mutex};
use std::time::Duration;

use deepseek_mobile_agent_core::ssh::SshCommandRequest;
use deepseek_mobile_web_server::{
    AppState, PendingApproval, SshTarget,
    approvals::ApprovalService,
    ssh_exec::{CommandRunError, CommandRunner, SshCommandOutput},
};
use pretty_assertions::assert_eq;
use serde_json::json;

#[derive(Clone, Debug)]
struct FakeRunner {
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeRunner {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls mutex").clone()
    }
}

impl CommandRunner for FakeRunner {
    fn run(
        &self,
        _target: &SshTarget,
        command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(command.command.clone());
        Ok(SshCommandOutput {
            stdout: format!("ran {}\n", command.command),
            stderr: String::new(),
            exit_code: Some(0),
            duration: Duration::from_millis(12),
            timed_out: false,
        })
    }
}

fn test_state() -> AppState {
    AppState::new(SshTarget {
        host: "127.0.0.1".to_string(),
        user: "tester".to_string(),
        port: 2222,
        key_present: false,
    })
}

fn agent_approval(id: &str, command: &str) -> PendingApproval {
    serde_json::from_value(json!({
        "id": id,
        "session_id": "session-1",
        "command": command,
        "created_at_ms": 100,
        "status": "pending",
        "agent_turn_id": "turn-1"
    }))
    .expect("agent approval must deserialize")
}

#[test]
fn agent_approval_summary_approve_once_appends_assistant_command_summary() {
    let state = test_state();
    let runner = FakeRunner::new();
    let service = ApprovalService::new(runner.clone());
    state.insert_pending_approval(agent_approval("approval-1", "uptime"));

    let response = service
        .respond(
            &state,
            "approval-1",
            serde_json::from_value(json!({"response": "approve_once"}))
                .expect("request must deserialize"),
        )
        .expect("approval must succeed");

    assert_eq!(response.status, "approved");
    assert_eq!(runner.calls(), vec!["uptime"]);
    let messages = state.messages("session-1");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "assistant");
    assert_eq!(
        messages[0].parts[0].text.as_deref(),
        Some("Approved command `uptime` completed with exit code 0.\nstdout:\nran uptime\n")
    );
}

#[test]
fn agent_approval_summary_reject_appends_assistant_not_executed_summary() {
    let state = test_state();
    let runner = FakeRunner::new();
    let service = ApprovalService::new(runner.clone());
    state.insert_pending_approval(agent_approval("approval-1", "systemctl restart ssh"));

    let response = service
        .respond(
            &state,
            "approval-1",
            serde_json::from_value(json!({"response": "reject"}))
                .expect("request must deserialize"),
        )
        .expect("approval must reject cleanly");

    assert_eq!(response.status, "rejected");
    assert!(runner.calls().is_empty());
    let messages = state.messages("session-1");
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].parts[0].text.as_deref(),
        Some("Rejected command `systemctl restart ssh`; it was not executed.")
    );
}

#[test]
fn agent_approval_summary_replay_still_returns_not_found_without_extra_summary() {
    let state = test_state();
    let runner = FakeRunner::new();
    let service = ApprovalService::new(runner.clone());
    state.insert_pending_approval(agent_approval("approval-1", "pwd"));

    service
        .respond(
            &state,
            "approval-1",
            serde_json::from_value(json!({"response": "approve_once"}))
                .expect("request must deserialize"),
        )
        .expect("first approval must succeed");
    let replay = service.respond(
        &state,
        "approval-1",
        serde_json::from_value(json!({"response": "approve_once"}))
            .expect("request must deserialize"),
    );

    assert!(replay.is_err());
    assert_eq!(runner.calls(), vec!["pwd"]);
    assert_eq!(state.messages("session-1").len(), 1);
}
