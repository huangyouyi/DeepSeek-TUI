use std::collections::VecDeque;

use deepseek_mobile_agent_core::capabilities::ExecutionMode;
use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName, RemoteToolOutput};
use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, TransportError,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn diagnose_call(call_id: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::DiagnoseSystem,
        arguments: json!({
            "include": ["system", "runner"],
            "format": "json"
        }),
    }
}

#[test]
fn authenticated_runner_client_discovers_capabilities_and_executes_planned_tool_without_network() {
    let pairing_token = "paired-runner-token";
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", pairing_token);
    let mut captured_requests: Vec<RunnerHttpRequestSpec> = Vec::new();
    let mut responses = VecDeque::from([
        RunnerHttpResponseSpec::ok_json(json!({
            "tools": [
                "remote.diagnose.system",
                "remote.browser.open",
                "remote.package.install"
            ]
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "diagnose-1",
            "success": true,
            "result": {
                "platform": "linux",
                "shell": "bash",
                "checks": [
                    { "name": "runner", "ok": true }
                ]
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

    let capabilities = client.discover_capabilities().unwrap();
    let output = client
        .execute_tool_call(diagnose_call("diagnose-1"))
        .unwrap();
    drop(client);

    assert_eq!(capabilities.mode, ExecutionMode::Runner);
    assert_eq!(
        capabilities.tools,
        vec![
            RemoteToolName::DiagnoseSystem,
            RemoteToolName::BrowserOpen,
            RemoteToolName::PackageInstall,
        ]
    );
    assert!(capabilities.allows(RemoteToolName::DiagnoseSystem));
    assert!(capabilities.allows(RemoteToolName::BrowserOpen));
    assert!(capabilities.allows(RemoteToolName::PackageInstall));

    assert_eq!(captured_requests.len(), 2);
    for request in &captured_requests {
        assert!(request.token_present);
        assert_eq!(request.auth_scheme(), Some("Bearer"));
        assert_eq!(
            request.authorization_header_value(),
            Some(format!("Bearer {pairing_token}"))
        );
    }

    assert_eq!(captured_requests[0].method, "GET");
    assert_eq!(
        captured_requests[0].url,
        "https://runner.example/mobile/capabilities"
    );
    assert_eq!(captured_requests[0].body, Value::Null);

    assert_eq!(captured_requests[1].method, "POST");
    assert_eq!(
        captured_requests[1].url,
        "https://runner.example/mobile/tool-call"
    );
    assert_eq!(
        captured_requests[1].body,
        json!({
            "call_id": "diagnose-1",
            "name": "remote.diagnose.system",
            "arguments": {
                "include": ["system", "runner"],
                "format": "json"
            }
        })
    );
    assert_eq!(
        output,
        RemoteToolOutput {
            call_id: "diagnose-1".to_string(),
            success: true,
            result: json!({
                "platform": "linux",
                "shell": "bash",
                "checks": [
                    { "name": "runner", "ok": true }
                ]
            }),
        }
    );
    assert_eq!(output.result["platform"], "linux");
    assert_eq!(output.result["checks"][0]["ok"], true);
}
