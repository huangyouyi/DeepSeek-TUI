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

fn tool_part_data(state: &AppState, session_id: &str, status: &str) -> serde_json::Value {
    state
        .messages(session_id)
        .into_iter()
        .flat_map(|message| message.parts)
        .find(|part| part.kind == "tool" && part.data["status"] == status)
        .map(|part| part.data)
        .unwrap_or_else(|| panic!("expected {status} tool part"))
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
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "assistant");
    assert_eq!(
        messages[0].parts[0].text.as_deref(),
        Some("Approved command `uptime` completed with exit code 0.\nstdout:\nran uptime\n")
    );
    assert_eq!(
        messages[1].parts[0].text.as_deref(),
        Some(
            "本轮远程命令已全部执行完成。结果如下：\n\n1. Approved command `uptime` completed with exit code 0.\nstdout:\nran uptime"
        )
    );
    assert_eq!(
        response.result["summary"],
        "本轮远程命令已全部执行完成。结果如下：\n\n1. Approved command `uptime` completed with exit code 0.\nstdout:\nran uptime"
    );

    let tool = tool_part_data(&state, "session-1", "completed");
    assert_eq!(tool["approval_id"], "approval-1");
    assert_eq!(tool["turn_id"], "turn-1");
    assert_eq!(tool["agent_turn_id"], "turn-1");
    assert_eq!(tool["tool"], "remote.shell.exec");
    assert_eq!(tool["requires_approval"], true);
    assert_eq!(tool["command"], "uptime");
    assert_eq!(tool["output"], "ran uptime\n");
    assert_eq!(tool["stdout"], "ran uptime\n");
    assert_eq!(tool["stderr"], "");
    assert_eq!(tool["exit_code"], 0);
    assert_eq!(tool["duration_ms"], 12);
    assert_eq!(tool["timed_out"], false);
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
    assert_eq!(
        response.result["summary"],
        "本轮已按用户要求停止，未继续执行后续远程命令。已记录结果如下：\n\n1. Rejected command `systemctl restart ssh`; it was not executed."
    );
    assert_eq!(
        response.result["assistant_text"],
        "本轮已按用户要求停止，未继续执行后续远程命令。已记录结果如下：\n\n1. Rejected command `systemctl restart ssh`; it was not executed."
    );
    assert!(runner.calls().is_empty());
    let messages = state.messages("session-1");
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0].parts[0].text.as_deref(),
        Some("Rejected command `systemctl restart ssh`; it was not executed.")
    );
    assert_eq!(
        messages[1].parts[0].text.as_deref(),
        Some(
            "本轮已按用户要求停止，未继续执行后续远程命令。已记录结果如下：\n\n1. Rejected command `systemctl restart ssh`; it was not executed."
        )
    );

    let tool = tool_part_data(&state, "session-1", "rejected");
    assert_eq!(tool["approval_id"], "approval-1");
    assert_eq!(tool["turn_id"], "turn-1");
    assert_eq!(tool["agent_turn_id"], "turn-1");
    assert_eq!(tool["tool"], "remote.shell.exec");
    assert_eq!(tool["requires_approval"], true);
    assert_eq!(tool["command"], "systemctl restart ssh");
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
    assert_eq!(state.messages("session-1").len(), 2);
}

#[test]
fn agent_approval_summary_waits_for_all_same_turn_approvals_before_final_answer() {
    let state = test_state();
    let runner = FakeRunner::new();
    let service = ApprovalService::new(runner.clone());
    state.insert_pending_approval(agent_approval("approval-1", "uname -a"));
    state.insert_pending_approval(agent_approval("approval-2", "df -h"));

    service
        .respond(
            &state,
            "approval-1",
            serde_json::from_value(json!({"response": "approve_once"}))
                .expect("request must deserialize"),
        )
        .expect("first approval must succeed");
    let messages = state.messages("session-1");
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0].parts[0]
            .text
            .as_deref()
            .unwrap_or("")
            .contains("Approved command `uname -a`")
    );

    service
        .respond(
            &state,
            "approval-2",
            serde_json::from_value(json!({"response": "approve_once"}))
                .expect("request must deserialize"),
        )
        .expect("second approval must succeed");

    let messages = state.messages("session-1");
    assert_eq!(messages.len(), 3);
    assert!(
        messages[1].parts[0]
            .text
            .as_deref()
            .unwrap_or("")
            .contains("Approved command `df -h`")
    );
    assert_eq!(
        messages[2].parts[0].text.as_deref(),
        Some(
            "本轮远程命令已全部执行完成。结果如下：\n\n1. Approved command `uname -a` completed with exit code 0.\nstdout:\nran uname -a\n\n2. Approved command `df -h` completed with exit code 0.\nstdout:\nran df -h"
        )
    );
}

#[test]
fn agent_approval_summary_reject_stop_stops_remaining_same_turn_approvals() {
    let state = test_state();
    let runner = FakeRunner::new();
    let service = ApprovalService::new(runner.clone());
    state.insert_pending_approval(agent_approval("approval-1", "opkg update"));
    state.insert_pending_approval(agent_approval("approval-2", "systemctl restart ssh"));
    state.insert_pending_approval(
        serde_json::from_value(json!({
            "id": "approval-other-turn",
            "session_id": "session-1",
            "command": "reboot",
            "created_at_ms": 100,
            "status": "pending",
            "agent_turn_id": "turn-2"
        }))
        .expect("agent approval must deserialize"),
    );

    let response = service
        .respond(
            &state,
            "approval-1",
            serde_json::from_value(json!({"response": "reject_stop"}))
                .expect("request must deserialize"),
        )
        .expect("approval must reject and stop cleanly");

    assert_eq!(response.status, "rejected");
    assert_eq!(response.result["stopped_count"], 1);
    assert!(
        response.result["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("本轮已按用户要求停止")
    );
    assert!(runner.calls().is_empty());
    let remaining = state.pending_approvals();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "approval-other-turn");

    let messages = state.messages("session-1");
    assert_eq!(messages.len(), 3);
    let rejected_tool = tool_part_data(&state, "session-1", "rejected");
    assert_eq!(rejected_tool["approval_id"], "approval-1");
    let stopped_tool = tool_part_data(&state, "session-1", "stopped");
    assert_eq!(stopped_tool["approval_id"], "approval-2");

    let audit = state.audit_recent();
    let last = audit.last().expect("audit entry");
    assert_eq!(last.kind, "approval.rejected");
    assert_eq!(last.metadata["response"], "reject_stop");
    assert_eq!(last.metadata["stopped_count"], 1);
    assert_eq!(last.metadata["risk"]["level"], "high");
    assert_eq!(last.metadata["target"], "remote.shell.exec");
}
