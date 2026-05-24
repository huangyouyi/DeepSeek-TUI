use deepseek_mobile_agent_core::agent_loop::AgentLoop;
use deepseek_mobile_agent_core::model::{CloudModelClient, CloudModelConfig, FakeHttpTransport};
use deepseek_mobile_agent_core::{
    FakeTransport, MobileEventKind, RemoteToolName, RemoteToolOutput,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn cloud_client() -> CloudModelClient {
    CloudModelClient::new(CloudModelConfig {
        provider: "deepseek".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-v4-flash".to_string(),
        api_key_present: true,
    })
}

#[test]
fn cloud_model_low_risk_tool_call_runs_through_agent_loop() {
    let mut http = FakeHttpTransport::new(vec![
        r#"{
            "choices": [
                {
                    "message": {
                        "content": "I'll inspect the system.",
                        "tool_calls": [
                            {
                                "id": "call-low",
                                "type": "function",
                                "function": {
                                    "name": "remote.shell.exec",
                                    "arguments": "{\"command\":\"uname -a\"}"
                                }
                            }
                        ]
                    }
                }
            ]
        }"#,
    ]);
    let model_response = cloud_client().complete_with_tools(
        &mut http,
        "inspect system",
        &[RemoteToolName::ShellExec],
    );
    let output = RemoteToolOutput {
        call_id: "call-low".to_string(),
        success: true,
        result: json!({ "stdout": "/Users/dev/project\n", "stderr": "", "exit_code": 0 }),
    };
    let mut transport = FakeTransport::default();
    transport.push_output(output.clone());

    let outcome = AgentLoop::new().run_model_tool_turn(model_response, &mut transport);

    assert_eq!(outcome.assistant_text, "I'll inspect the system.");
    assert_eq!(outcome.executed_tool_outputs, vec![output]);
    assert!(outcome.pending_approvals.is_empty());
    assert_eq!(transport.calls().len(), 1);
    assert_eq!(transport.calls()[0].call_id, "call-low");
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
fn cloud_model_high_risk_tool_call_becomes_pending_approval() {
    let mut http = FakeHttpTransport::new(vec![
        r#"{
            "choices": [
                {
                    "message": {
                        "content": "This needs approval first.",
                        "tool_calls": [
                            {
                                "id": "call-danger",
                                "type": "function",
                                "function": {
                                    "name": "remote.shell.exec",
                                    "arguments": "{\"command\":\"sudo xcode-select --switch /Applications/Xcode.app\"}"
                                }
                            }
                        ]
                    }
                }
            ]
        }"#,
    ]);
    let model_response =
        cloud_client().complete_with_tools(&mut http, "fix xcode", &[RemoteToolName::ShellExec]);
    let mut transport = FakeTransport::default();

    let outcome = AgentLoop::new().run_model_tool_turn(model_response, &mut transport);

    assert_eq!(outcome.assistant_text, "This needs approval first.");
    assert!(outcome.executed_tool_outputs.is_empty());
    assert_eq!(outcome.pending_approvals.len(), 1);
    assert_eq!(
        outcome.pending_approvals[0].approval_id,
        "approval-call-danger"
    );
    assert_eq!(outcome.pending_approvals[0].call_id, "call-danger");
    assert_eq!(outcome.pending_approvals[0].tool_name, "shell_exec");
    assert!(transport.calls().is_empty());
    assert_eq!(
        outcome
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![MobileEventKind::ApprovalRequired]
    );
}
