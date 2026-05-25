use deepseek_mobile_agent_core::agent_loop::AgentLoop;
use deepseek_mobile_agent_core::{
    FakeModelClient, FakeTransport, MobileEventKind, ModelResponse, RemoteToolCall, RemoteToolName,
    RemoteToolOutput,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn shell_call(call_id: &str, command: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": command }),
    }
}

fn response(text: &str, tool_calls: Vec<RemoteToolCall>) -> ModelResponse {
    ModelResponse {
        text: text.to_string(),
        tool_calls,
    }
}

#[test]
fn low_risk_shell_exec_runs_through_transport() {
    let call = shell_call("call-1", "uname -a");
    let output = RemoteToolOutput {
        call_id: "call-1".to_string(),
        success: true,
        result: json!({ "stdout": "/tmp\n", "stderr": "", "exit_code": 0 }),
    };
    let mut model = FakeModelClient::new(vec![response("checking", vec![call.clone()])]);
    let mut transport = FakeTransport::default();
    transport.push_output(output.clone());

    let outcome = AgentLoop::new().run_turn(&mut model, &mut transport, "where am I?");

    assert_eq!(model.prompts(), &["where am I?".to_string()]);
    assert_eq!(transport.calls(), &[call]);
    assert_eq!(outcome.assistant_text, "checking");
    assert_eq!(outcome.executed_tool_outputs, vec![output]);
    assert!(outcome.pending_approvals.is_empty());
    assert_eq!(
        outcome
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            MobileEventKind::ToolCallStarted,
            MobileEventKind::ToolCallCompleted,
        ]
    );
}

#[test]
fn high_risk_sudo_shell_exec_requires_approval_without_transport_execution() {
    let call = shell_call(
        "danger",
        "sudo xcode-select --switch /Applications/Xcode.app",
    );
    let mut model = FakeModelClient::new(vec![response("needs approval", vec![call])]);
    let mut transport = FakeTransport::default();

    let outcome = AgentLoop::new().run_turn(&mut model, &mut transport, "fix xcode");

    assert!(transport.calls().is_empty());
    assert_eq!(outcome.assistant_text, "needs approval");
    assert!(outcome.executed_tool_outputs.is_empty());
    assert_eq!(outcome.pending_approvals.len(), 1);
    assert_eq!(outcome.pending_approvals[0].call_id, "danger");
    assert_eq!(outcome.pending_approvals[0].tool_name, "shell_exec");
    assert_eq!(
        outcome
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![MobileEventKind::ApprovalRequired]
    );
}

#[test]
fn text_only_response_does_not_execute_tools() {
    let mut model = FakeModelClient::new(vec![response("plain answer", Vec::new())]);
    let mut transport = FakeTransport::default();

    let outcome = AgentLoop::new().run_turn(&mut model, &mut transport, "hello");

    assert_eq!(transport.calls(), &[]);
    assert_eq!(outcome.assistant_text, "plain answer");
    assert!(outcome.executed_tool_outputs.is_empty());
    assert!(outcome.pending_approvals.is_empty());
    assert!(outcome.events.is_empty());
}
