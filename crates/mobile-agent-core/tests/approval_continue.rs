use deepseek_mobile_agent_core::agent_loop::{AgentLoop, ToolContinuationOutcome};
use deepseek_mobile_agent_core::{
    ApprovalDecision, FakeModelClient, FakeTransport, MobileEventKind, ModelResponse,
    RemoteToolCall, RemoteToolName, RemoteToolOutput,
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
fn approved_pending_tool_call_continues_through_transport() {
    let call = shell_call(
        "danger",
        "sudo xcode-select --switch /Applications/Xcode.app",
    );
    let output = RemoteToolOutput {
        call_id: "danger".to_string(),
        success: true,
        result: json!({ "stdout": "ok\n", "stderr": "", "exit_code": 0 }),
    };
    let mut model = FakeModelClient::new(vec![response("needs approval", vec![call.clone()])]);
    let mut transport = FakeTransport::default();

    let turn = AgentLoop::new().run_turn(&mut model, &mut transport, "fix xcode");

    assert!(transport.calls().is_empty());
    assert_eq!(turn.pending_approvals.len(), 1);
    assert_eq!(turn.pending_approvals[0].call, call);

    transport.push_output(output.clone());
    let continued = AgentLoop::new().continue_approved_tool(
        &mut transport,
        turn.pending_approvals[0].clone(),
        ApprovalDecision::Approved,
    );

    assert_eq!(transport.calls(), &[call]);
    assert_eq!(
        continued,
        ToolContinuationOutcome {
            approval_id: "approval-danger".to_string(),
            call_id: "danger".to_string(),
            decision: ApprovalDecision::Approved,
            executed_tool_output: Some(output),
            events: continued.events.clone(),
        }
    );
    assert_eq!(
        continued
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
fn denied_pending_tool_call_does_not_execute_transport() {
    let call = shell_call("danger", "rm -rf /tmp/deepseek-test");
    let mut model = FakeModelClient::new(vec![response("needs approval", vec![call.clone()])]);
    let mut transport = FakeTransport::default();

    let turn = AgentLoop::new().run_turn(&mut model, &mut transport, "clean temp");
    let continued = AgentLoop::new().continue_approved_tool(
        &mut transport,
        turn.pending_approvals[0].clone(),
        ApprovalDecision::Denied,
    );

    assert!(transport.calls().is_empty());
    assert_eq!(continued.approval_id, "approval-danger");
    assert_eq!(continued.call_id, "danger");
    assert_eq!(continued.decision, ApprovalDecision::Denied);
    assert_eq!(continued.executed_tool_output, None);
    assert!(continued.events.is_empty());
}
