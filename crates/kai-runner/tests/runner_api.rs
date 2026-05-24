use kai_runner::{KaiRunner, RunnerRequest, RunnerResponse};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn runner_api_types_round_trip_through_json() {
    let request = RunnerRequest::ToolCall {
        call_id: "call-1".to_string(),
        name: "remote.diagnose.system".to_string(),
        arguments: json!({ "verbose": true }),
    };

    let encoded = serde_json::to_string(&request).unwrap();
    let decoded: RunnerRequest = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, request);

    let response = RunnerResponse::ToolCall {
        call_id: "call-1".to_string(),
        success: true,
        result: json!({ "tool": "remote.diagnose.system" }),
    };

    let encoded = serde_json::to_string(&response).unwrap();
    let decoded: RunnerResponse = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, response);
}

#[test]
fn handle_request_returns_capabilities_with_documented_tool_names() {
    let runner = KaiRunner;

    let response = runner.handle_request(RunnerRequest::Capabilities);

    assert_eq!(
        response,
        RunnerResponse::Capabilities {
            mode: "runner".to_string(),
            tools: vec![
                "remote.shell.exec".to_string(),
                "remote.powershell.exec".to_string(),
                "remote.file.read".to_string(),
                "remote.file.write".to_string(),
                "remote.diagnose.system".to_string(),
                "remote.package.install".to_string(),
                "remote.browser.open".to_string(),
                "remote.browser.extract_text".to_string(),
                "remote.browser.click".to_string(),
                "remote.bootstrap.guide".to_string(),
                "remote.mcp.call".to_string(),
            ],
        }
    );
}

#[test]
fn handle_request_routes_diagnose_tool_calls_through_execute() {
    let runner = KaiRunner;

    let response = runner.handle_request(RunnerRequest::ToolCall {
        call_id: "diag-1".to_string(),
        name: "remote.diagnose.system".to_string(),
        arguments: json!({}),
    });

    let RunnerResponse::ToolCall {
        call_id,
        success,
        result,
    } = response
    else {
        panic!("expected tool_call response");
    };

    assert_eq!(call_id, "diag-1");
    assert!(success);
    assert_eq!(result["tool"], json!("remote.diagnose.system"));
    assert_eq!(result["system"]["os"], json!(std::env::consts::OS));
}

#[test]
fn handle_request_blocks_shell_tool_calls_with_structured_error() {
    let runner = KaiRunner;

    let response = runner.handle_request(RunnerRequest::ToolCall {
        call_id: "shell-1".to_string(),
        name: "remote.shell.exec".to_string(),
        arguments: json!({ "command": "echo should-not-run" }),
    });

    assert_eq!(
        response,
        RunnerResponse::ToolCall {
            call_id: "shell-1".to_string(),
            success: false,
            result: json!({
                "tool": "remote.shell.exec",
                "status": "error",
                "error": {
                    "code": "blocked",
                    "message": "kai-runner scaffold does not execute local shell commands"
                }
            }),
        }
    );
}

#[test]
fn handle_request_returns_structured_error_for_unsupported_tool_calls() {
    let runner = KaiRunner;

    let response = runner.handle_request(RunnerRequest::ToolCall {
        call_id: "unsupported-1".to_string(),
        name: "remote.file.read".to_string(),
        arguments: json!({ "path": "/tmp/example" }),
    });

    assert_eq!(
        response,
        RunnerResponse::ToolCall {
            call_id: "unsupported-1".to_string(),
            success: false,
            result: json!({
                "tool": "remote.file.read",
                "status": "error",
                "error": {
                    "code": "unsupported",
                    "message": "remote.file.read is not implemented in the kai-runner scaffold"
                }
            }),
        }
    );
}

#[test]
fn handle_request_returns_structured_error_for_unknown_tool_calls() {
    let runner = KaiRunner;

    let response = runner.handle_request(RunnerRequest::ToolCall {
        call_id: "unknown-1".to_string(),
        name: "remote.future.tool".to_string(),
        arguments: json!({}),
    });

    assert_eq!(
        response,
        RunnerResponse::ToolCall {
            call_id: "unknown-1".to_string(),
            success: false,
            result: json!({
                "tool": "remote.future.tool",
                "status": "error",
                "error": {
                    "code": "unknown_tool",
                    "message": "unknown remote tool name: remote.future.tool"
                }
            }),
        }
    );
}
