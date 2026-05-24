use std::collections::VecDeque;

use deepseek_mobile_agent_core::agent_loop::AgentLoop;
use deepseek_mobile_agent_core::model::ModelResponse;
use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, TransportError,
};
use deepseek_mobile_agent_core::{
    FakeTransport, MobileEventKind, RemoteToolCall, RemoteToolName, RemoteToolOutput, RiskLevel,
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
fn runner_http_browser_session_carries_session_id_and_auth_without_network() {
    let pairing_token = "paired-browser-token";
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", pairing_token);
    let mut captured_requests: Vec<RunnerHttpRequestSpec> = Vec::new();
    let mut responses = VecDeque::from([
        RunnerHttpResponseSpec::ok_json(json!({
            "call_id": "browser-open",
            "success": true,
            "result": {
                "session_id": "browser-session-123",
                "url": "https://example.test/docs"
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "call_id": "browser-extract",
            "success": true,
            "result": {
                "session_id": "browser-session-123",
                "text": "DeepSeek mobile browser fixture"
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "call_id": "browser-extract-unavailable",
            "success": false,
            "result": {
                "error": "browser_unavailable",
                "session_id": "browser-session-123"
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

    let open_output = client
        .execute_tool_call(browser_call(
            "browser-open",
            RemoteToolName::BrowserOpen,
            json!({ "url": "https://example.test/docs" }),
        ))
        .unwrap();
    let session_id = open_output.result["session_id"]
        .as_str()
        .expect("browser.open must return a session_id");
    let extract_output = client
        .execute_tool_call(browser_call(
            "browser-extract",
            RemoteToolName::BrowserExtractText,
            json!({ "session_id": session_id }),
        ))
        .unwrap();
    let unavailable_output = client
        .execute_tool_call(browser_call(
            "browser-extract-unavailable",
            RemoteToolName::BrowserExtractText,
            json!({ "session_id": session_id }),
        ))
        .unwrap();
    drop(client);

    assert_eq!(
        open_output,
        RemoteToolOutput {
            call_id: "browser-open".to_string(),
            success: true,
            result: json!({
                "session_id": "browser-session-123",
                "url": "https://example.test/docs"
            }),
        }
    );
    assert_eq!(extract_output.call_id, "browser-extract");
    assert!(extract_output.success);
    assert_eq!(extract_output.result["session_id"], session_id);
    assert_eq!(
        extract_output.result["text"],
        "DeepSeek mobile browser fixture"
    );
    assert_eq!(unavailable_output.call_id, "browser-extract-unavailable");
    assert!(!unavailable_output.success);
    assert_eq!(unavailable_output.result["error"], "browser_unavailable");
    assert_eq!(unavailable_output.result["session_id"], session_id);

    assert_eq!(captured_requests.len(), 3);
    for request in &captured_requests {
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://runner.example/mobile/tool-call");
        assert!(request.token_present);
        assert_eq!(request.auth_scheme(), Some("Bearer"));
        assert_eq!(
            request.authorization_header_value(),
            Some(format!("Bearer {pairing_token}"))
        );
    }
    assert_eq!(
        captured_requests[0].body,
        json!({
            "call_id": "browser-open",
            "name": "remote.browser.open",
            "arguments": { "url": "https://example.test/docs" }
        })
    );
    assert_eq!(
        captured_requests[1].body,
        json!({
            "call_id": "browser-extract",
            "name": "remote.browser.extract_text",
            "arguments": { "session_id": "browser-session-123" }
        })
    );
    assert_eq!(
        captured_requests[2].body,
        json!({
            "call_id": "browser-extract-unavailable",
            "name": "remote.browser.extract_text",
            "arguments": { "session_id": "browser-session-123" }
        })
    );
}

#[test]
fn browser_click_becomes_pending_medium_risk_approval_before_runner_execution() {
    let model_response = ModelResponse {
        text: "Clicking requires approval first.".to_string(),
        tool_calls: vec![browser_call(
            "browser-click",
            RemoteToolName::BrowserClick,
            json!({
                "session_id": "browser-session-123",
                "selector": "#confirm"
            }),
        )],
    };
    let mut transport = FakeTransport::default();

    let outcome = AgentLoop::new().run_model_tool_turn(model_response, &mut transport);

    assert_eq!(outcome.assistant_text, "Clicking requires approval first.");
    assert!(outcome.executed_tool_outputs.is_empty());
    assert!(transport.calls().is_empty());
    assert_eq!(outcome.pending_approvals.len(), 1);
    assert_eq!(
        outcome.pending_approvals[0].approval_id,
        "approval-browser-click"
    );
    assert_eq!(outcome.pending_approvals[0].call_id, "browser-click");
    assert_eq!(outcome.pending_approvals[0].tool_name, "browser_click");
    assert_eq!(outcome.pending_approvals[0].risk.level, RiskLevel::Medium);
    assert_eq!(
        outcome.pending_approvals[0].call.arguments["session_id"],
        "browser-session-123"
    );
    assert_eq!(
        outcome
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![MobileEventKind::ApprovalRequired]
    );
    assert_eq!(outcome.events[0].payload["call_id"], "browser-click");
    assert_eq!(outcome.events[0].payload["tool_name"], "browser_click");
    assert_eq!(outcome.events[0].payload["risk"]["level"], "Medium");
}
