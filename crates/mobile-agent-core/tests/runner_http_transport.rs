use deepseek_mobile_agent_core::capabilities::{CapabilitySet, ExecutionMode};
use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName, RemoteToolOutput};
use deepseek_mobile_agent_core::transport::{
    RemoteToolTransport, RunnerHttpBackend, RunnerHttpClient, RunnerHttpClosureBackend,
    RunnerHttpExchange, RunnerHttpRequestSpec, RunnerHttpResponseSpec, RunnerHttpTransport,
    RunnerTransportConfig, TransportError,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[derive(Default)]
struct FakeRunnerHttpExchange {
    requests: Vec<RunnerHttpRequestSpec>,
    responses: Vec<Value>,
}

impl FakeRunnerHttpExchange {
    fn with_response(response: Value) -> Self {
        Self {
            requests: Vec::new(),
            responses: vec![response],
        }
    }
}

impl RunnerHttpExchange for FakeRunnerHttpExchange {
    fn exchange(&mut self, request: RunnerHttpRequestSpec) -> Result<Value, TransportError> {
        self.requests.push(request);
        if self.responses.is_empty() {
            return Err(TransportError::Failed("no fake HTTP response".to_string()));
        }
        Ok(self.responses.remove(0))
    }
}

impl RunnerHttpBackend for FakeRunnerHttpExchange {
    fn execute(
        &mut self,
        request: RunnerHttpRequestSpec,
    ) -> Result<RunnerHttpResponseSpec, TransportError> {
        self.exchange(request).map(RunnerHttpResponseSpec::ok_json)
    }
}

fn shell_call(call_id: &str, command: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": command }),
    }
}

fn mcp_call(call_id: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::McpCall,
        arguments: json!({
            "server": "filesystem",
            "tool": "read_file",
            "arguments": { "path": "README.md" }
        }),
    }
}

#[test]
fn http_transport_holds_endpoint_and_token_state() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example".to_string(),
        token_present: true,
    });

    assert_eq!(transport.endpoint(), "https://runner.example");
    assert!(transport.token_present());
}

#[test]
fn http_transport_with_bearer_token_prepares_redacted_auth_metadata() {
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/api", "runner-secret");

    assert!(transport.token_present());

    let request = transport.prepare_capabilities_request().unwrap();

    assert!(request.token_present);
    assert_eq!(request.auth_scheme(), Some("Bearer"));
    assert_eq!(
        request.authorization_header_value(),
        Some("Bearer runner-secret".to_string())
    );
    assert!(!format!("{request:?}").contains("runner-secret"));
}

#[test]
fn http_transport_without_bearer_token_has_no_auth_metadata() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/api".to_string(),
        token_present: false,
    });

    let request = transport.prepare_capabilities_request().unwrap();

    assert!(!request.token_present);
    assert_eq!(request.auth_scheme(), None);
    assert_eq!(request.authorization_header_value(), None);
}

#[test]
fn bearer_token_metadata_is_added_to_prepared_runner_requests() {
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/api", "runner-secret");

    let capabilities = transport.prepare_capabilities_request().unwrap();
    let tool_call = transport
        .prepare_tool_call_request(&shell_call("call-1", "pwd"))
        .unwrap();
    let mcp_call = transport
        .prepare_mcp_call_request(&mcp_call("mcp-1"))
        .unwrap();

    assert_eq!(
        capabilities.authorization_header_value(),
        Some("Bearer runner-secret".to_string())
    );
    assert_eq!(
        tool_call.authorization_header_value(),
        Some("Bearer runner-secret".to_string())
    );
    assert_eq!(
        mcp_call.authorization_header_value(),
        Some("Bearer runner-secret".to_string())
    );
}

#[test]
fn prepare_tool_call_request_builds_runner_http_post_spec() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/api/".to_string(),
        token_present: true,
    });

    let request = transport
        .prepare_tool_call_request(&shell_call("call-1", "pwd"))
        .unwrap();

    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://runner.example/api/tool-call");
    assert!(request.token_present);
    assert_eq!(
        request.body,
        json!({
            "call_id": "call-1",
            "name": "remote.shell.exec",
            "arguments": { "command": "pwd" }
        })
    );
}

#[test]
fn prepare_mcp_call_request_builds_runner_proxy_http_post_spec() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/api".to_string(),
        token_present: true,
    });

    let request = transport
        .prepare_mcp_call_request(&mcp_call("mcp-1"))
        .unwrap();

    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://runner.example/api/mcp/call");
    assert!(request.token_present);
    assert_eq!(
        request.body,
        json!({
            "call_id": "mcp-1",
            "arguments": {
                "server": "filesystem",
                "tool": "read_file",
                "arguments": { "path": "README.md" }
            }
        })
    );
}

#[test]
fn prepare_mcp_call_request_trims_endpoint_before_appending_proxy_path() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "  https://runner.example/api///  ".to_string(),
        token_present: false,
    });

    let request = transport
        .prepare_mcp_call_request(&mcp_call("mcp-1"))
        .unwrap();

    assert_eq!(request.url, "https://runner.example/api/mcp/call");
    assert!(!request.token_present);
}

#[test]
fn prepare_mcp_call_request_rejects_empty_endpoint() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: " / ".to_string(),
        token_present: true,
    });

    let err = transport
        .prepare_mcp_call_request(&mcp_call("mcp-1"))
        .unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed("runner MCP call transport is not configured".to_string())
    );
}

#[test]
fn prepare_mcp_call_request_rejects_non_mcp_tools() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example".to_string(),
        token_present: true,
    });

    let err = transport
        .prepare_mcp_call_request(&shell_call("call-1", "pwd"))
        .unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed(
            "runner MCP call request expected remote.mcp.call but received remote.shell.exec"
                .to_string()
        )
    );
}

#[test]
fn prepare_capabilities_request_trims_endpoint_before_appending_path() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "  https://runner.example/api///  ".to_string(),
        token_present: false,
    });

    let request = transport.prepare_capabilities_request().unwrap();

    assert_eq!(request.method, "GET");
    assert_eq!(request.url, "https://runner.example/api/capabilities");
    assert!(!request.token_present);
    assert_eq!(request.body, Value::Null);
}

#[test]
fn parse_capabilities_response_filters_unknown_tools_into_runner_capabilities() {
    let capabilities = RunnerHttpTransport::parse_capabilities_response(json!({
        "tools": [
            "remote.shell.exec",
            "remote.unknown.experimental",
            "remote.mcp.call",
            "remote.shell.exec"
        ]
    }))
    .unwrap();

    assert_eq!(
        capabilities,
        CapabilitySet {
            mode: ExecutionMode::Runner,
            tools: vec![RemoteToolName::ShellExec, RemoteToolName::McpCall],
        }
    );
}

#[test]
fn fake_http_exchange_discovers_capabilities() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/api/".to_string(),
        token_present: true,
    });
    let mut http = FakeRunnerHttpExchange::with_response(json!({
        "tools": ["remote.file.read", "remote.browser.open"]
    }));

    let capabilities = transport.discover_capabilities(&mut http).unwrap();

    assert_eq!(
        capabilities.tools,
        vec![RemoteToolName::FileRead, RemoteToolName::BrowserOpen]
    );
    assert_eq!(http.requests.len(), 1);
    assert_eq!(http.requests[0].method, "GET");
    assert_eq!(
        http.requests[0].url,
        "https://runner.example/api/capabilities"
    );
    assert_eq!(http.requests[0].body, Value::Null);
}

#[test]
fn fake_http_exchange_executes_tool_call_and_preserves_request() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/api".to_string(),
        token_present: true,
    });
    let mut http = FakeRunnerHttpExchange::with_response(json!({
        "type": "tool_call",
        "call_id": "call-2",
        "success": true,
        "result": { "stdout": "ok\n", "stderr": "", "exit_code": 0 }
    }));

    let output = transport
        .execute_tool_call_exchange(shell_call("call-2", "echo ok"), &mut http)
        .unwrap();

    assert_eq!(
        output,
        RemoteToolOutput {
            call_id: "call-2".to_string(),
            success: true,
            result: json!({ "stdout": "ok\n", "stderr": "", "exit_code": 0 }),
        }
    );
    assert_eq!(http.requests.len(), 1);
    assert_eq!(http.requests[0].method, "POST");
    assert_eq!(http.requests[0].url, "https://runner.example/api/tool-call");
    assert_eq!(
        http.requests[0].body,
        json!({
            "call_id": "call-2",
            "name": "remote.shell.exec",
            "arguments": { "command": "echo ok" }
        })
    );
}

#[test]
fn http_client_discovers_capabilities_through_backend_request_boundary() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/api/".to_string(),
        token_present: true,
    });
    let backend = FakeRunnerHttpExchange::with_response(json!({
        "tools": ["remote.shell.exec", "remote.mcp.call"]
    }));
    let mut client = RunnerHttpClient::new(transport, backend);

    let capabilities = client.discover_capabilities().unwrap();

    assert_eq!(
        capabilities.tools,
        vec![RemoteToolName::ShellExec, RemoteToolName::McpCall]
    );
    let backend = client.backend();
    assert_eq!(backend.requests.len(), 1);
    assert_eq!(backend.requests[0].method, "GET");
    assert_eq!(
        backend.requests[0].url,
        "https://runner.example/api/capabilities"
    );
    assert!(backend.requests[0].token_present);
    assert_eq!(backend.requests[0].body, Value::Null);
}

#[test]
fn http_client_executes_tool_call_through_backend_request_boundary() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/api".to_string(),
        token_present: true,
    });
    let backend = FakeRunnerHttpExchange::with_response(json!({
        "type": "tool_call",
        "call_id": "call-3",
        "success": true,
        "result": { "stdout": "client\n", "stderr": "", "exit_code": 0 }
    }));
    let mut client = RunnerHttpClient::new(transport, backend);

    let output = client
        .execute_tool_call(shell_call("call-3", "printf client"))
        .unwrap();

    assert_eq!(
        output,
        RemoteToolOutput {
            call_id: "call-3".to_string(),
            success: true,
            result: json!({ "stdout": "client\n", "stderr": "", "exit_code": 0 }),
        }
    );
    let backend = client.backend();
    assert_eq!(backend.requests.len(), 1);
    assert_eq!(backend.requests[0].method, "POST");
    assert_eq!(
        backend.requests[0].url,
        "https://runner.example/api/tool-call"
    );
    assert!(backend.requests[0].token_present);
    assert_eq!(
        backend.requests[0].body,
        json!({
            "call_id": "call-3",
            "name": "remote.shell.exec",
            "arguments": { "command": "printf client" }
        })
    );
}

#[test]
fn closure_http_backend_receives_auth_metadata_and_parses_tool_response() {
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/api/", "runner-secret");
    let mut captured: Option<RunnerHttpRequestSpec> = None;
    let backend = RunnerHttpClosureBackend::new(|request: RunnerHttpRequestSpec| {
        captured = Some(request);
        Ok(RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "call-4",
            "success": true,
            "result": { "stdout": "closure\n", "stderr": "", "exit_code": 0 }
        })))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    let output = client
        .execute_tool_call(shell_call("call-4", "echo closure"))
        .unwrap();
    drop(client);

    let request = captured.expect("closure backend should receive request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://runner.example/api/tool-call");
    assert!(request.token_present);
    assert_eq!(
        request.authorization_header_value(),
        Some("Bearer runner-secret".to_string())
    );
    assert_eq!(
        request.body,
        json!({
            "call_id": "call-4",
            "name": "remote.shell.exec",
            "arguments": { "command": "echo closure" }
        })
    );
    assert_eq!(
        output,
        RemoteToolOutput {
            call_id: "call-4".to_string(),
            success: true,
            result: json!({ "stdout": "closure\n", "stderr": "", "exit_code": 0 }),
        }
    );
}

#[test]
fn closure_http_backend_receives_get_request_and_parses_capabilities_response() {
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/api", "runner-secret");
    let mut captured: Option<RunnerHttpRequestSpec> = None;
    let backend = RunnerHttpClosureBackend::new(|request: RunnerHttpRequestSpec| {
        captured = Some(request);
        Ok(RunnerHttpResponseSpec::json(
            200,
            json!({ "tools": ["remote.shell.exec"] }),
        ))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    let capabilities = client.discover_capabilities().unwrap();
    drop(client);

    let request = captured.expect("closure backend should receive request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, "https://runner.example/api/capabilities");
    assert_eq!(request.body, Value::Null);
    assert_eq!(
        request.authorization_header_value(),
        Some("Bearer runner-secret".to_string())
    );
    assert_eq!(capabilities.tools, vec![RemoteToolName::ShellExec]);
}

#[test]
fn parse_tool_call_response_decodes_runner_http_json_body() {
    let output = RunnerHttpTransport::parse_tool_call_response(json!({
        "type": "tool_call",
        "call_id": "call-1",
        "success": true,
        "result": { "stdout": "/tmp\n", "stderr": "", "exit_code": 0 }
    }))
    .unwrap();

    assert_eq!(
        output,
        RemoteToolOutput {
            call_id: "call-1".to_string(),
            success: true,
            result: json!({ "stdout": "/tmp\n", "stderr": "", "exit_code": 0 }),
        }
    );
}

#[test]
fn unconfigured_http_transport_returns_explicit_error() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: " ".to_string(),
        token_present: true,
    });

    let err = transport
        .prepare_tool_call_request(&shell_call("call-1", "pwd"))
        .unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed("runner http transport is not configured".to_string())
    );
}

#[test]
fn disconnected_http_transport_returns_explicit_error_without_network() {
    let mut transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example".to_string(),
        token_present: true,
    });

    let err = transport.execute(shell_call("call-1", "pwd")).unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed("runner http transport is not connected".to_string())
    );
}

#[test]
fn connected_fake_http_transport_parses_preconfigured_runner_response() {
    let mut transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example".to_string(),
        token_present: true,
    });
    transport.connect_fake();
    transport.push_response_body(json!({
        "type": "tool_call",
        "call_id": "call-1",
        "success": false,
        "result": { "error": "blocked by runner policy" }
    }));

    let output = transport.execute(shell_call("call-1", "rm -rf /")).unwrap();

    assert_eq!(
        transport.requests()[0].body,
        json!({
            "call_id": "call-1",
            "name": "remote.shell.exec",
            "arguments": { "command": "rm -rf /" }
        })
    );
    assert_eq!(
        output,
        RemoteToolOutput {
            call_id: "call-1".to_string(),
            success: false,
            result: json!({ "error": "blocked by runner policy" }),
        }
    );
}
