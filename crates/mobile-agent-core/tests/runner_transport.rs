use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName, RemoteToolOutput};
use deepseek_mobile_agent_core::transport::{
    RemoteToolTransport, RunnerTransportConfig, RunnerTransportState, RunnerWebSocketTransport,
    TransportError,
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

#[test]
fn runner_transport_starts_disconnected() {
    let transport = RunnerWebSocketTransport::new(RunnerTransportConfig {
        endpoint: "wss://runner.example/ws".to_string(),
        token_present: true,
    });

    assert_eq!(transport.state(), &RunnerTransportState::Disconnected);
}

#[test]
fn prepare_tool_call_request_matches_runner_tool_call_shape() {
    let transport = RunnerWebSocketTransport::new(RunnerTransportConfig {
        endpoint: "wss://runner.example/ws".to_string(),
        token_present: true,
    });

    let request = transport.prepare_tool_call_request(&shell_call("call-1", "pwd"));

    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "call_id": "call-1",
            "tool": "remote.shell.exec",
            "arguments": { "command": "pwd" }
        })
    );
}

#[test]
fn disconnected_runner_transport_returns_explicit_error() {
    let mut transport = RunnerWebSocketTransport::new(RunnerTransportConfig {
        endpoint: "wss://runner.example/ws".to_string(),
        token_present: true,
    });

    let err = transport.execute(shell_call("call-1", "pwd")).unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed("runner websocket transport is not connected".to_string())
    );
}

#[test]
fn connected_runner_transport_returns_preconfigured_output_without_network() {
    let mut transport = RunnerWebSocketTransport::new(RunnerTransportConfig {
        endpoint: "wss://runner.example/ws".to_string(),
        token_present: true,
    });
    let output = RemoteToolOutput {
        call_id: "call-1".to_string(),
        success: true,
        result: json!({ "stdout": "/tmp\n", "stderr": "", "exit_code": 0 }),
    };

    transport.connect_fake();
    transport.push_response(output.clone());

    assert_eq!(
        transport.state(),
        &RunnerTransportState::Connected {
            endpoint: "wss://runner.example/ws".to_string()
        }
    );
    assert_eq!(
        transport.execute(shell_call("call-1", "pwd")).unwrap(),
        output
    );
}
