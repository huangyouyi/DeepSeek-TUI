use deepseek_mobile_agent_core::MobileEventKind;
use deepseek_mobile_agent_core::event::ToolEventStreamBuilder;
use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName};
use deepseek_mobile_agent_core::transport::{
    RemoteToolTransport, RunnerHttpResponseSpec, RunnerHttpTransport, RunnerTransportConfig,
    RunnerWebSocketTransport, TransportError,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn long_running_shell_call() -> RemoteToolCall {
    RemoteToolCall {
        call_id: "lf-f8-call".to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({
            "command": "sleep 30",
            "timeout_ms": 1500,
            "cancel_token": "cancel-lf-f8-call",
            "status_poll_interval_ms": 250,
        }),
    }
}

#[test]
fn lf_f8_timeout_and_cancel_events_are_terminal_payload_contracts() {
    let mut stream = ToolEventStreamBuilder::starting_at("lf-f8-call", "remote.shell.exec", 41);

    let timeout = stream.timed_out(1500, 1512, "runner deadline exceeded");
    let cancel = stream.cancelled(223, "user requested cancellation");

    assert_eq!(timeout.seq, 41);
    assert_eq!(timeout.kind, MobileEventKind::ToolTimedOut);
    assert_eq!(
        timeout.payload,
        json!({
            "call_id": "lf-f8-call",
            "tool": "remote.shell.exec",
            "timeout_ms": 1500,
            "duration_ms": 1512,
            "reason": "runner deadline exceeded",
        })
    );

    assert_eq!(cancel.seq, 42);
    assert_eq!(cancel.kind, MobileEventKind::ToolCancelled);
    assert_eq!(
        cancel.payload,
        json!({
            "call_id": "lf-f8-call",
            "tool": "remote.shell.exec",
            "duration_ms": 223,
            "reason": "user requested cancellation",
        })
    );
}

#[test]
fn lf_f8_runner_tool_request_preserves_timeout_cancel_and_poll_metadata() {
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile/", "runner-secret");

    let request = transport
        .prepare_tool_call_request(&long_running_shell_call())
        .expect("tool call request should be prepared without network access");

    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://runner.example/mobile/tool-call");
    assert!(request.token_present);
    assert_eq!(request.auth_scheme(), Some("Bearer"));
    assert_eq!(
        request.body,
        json!({
            "call_id": "lf-f8-call",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "sleep 30",
                "timeout_ms": 1500,
                "cancel_token": "cancel-lf-f8-call",
                "status_poll_interval_ms": 250,
            },
        })
    );
    assert!(!format!("{request:?}").contains("runner-secret"));
}

#[test]
fn lf_f8_status_polling_http_failure_reports_explicit_status() {
    let error = RunnerHttpResponseSpec::json(
        504,
        json!({
            "error": "runner status polling timed out",
            "call_id": "lf-f8-call",
        }),
    )
    .into_success_json()
    .unwrap_err();

    assert_eq!(
        error,
        TransportError::Failed("runner http backend returned unsuccessful status 504".to_string())
    );
}

#[test]
fn lf_f8_disconnected_runner_transports_return_explicit_errors_without_network() {
    let config = RunnerTransportConfig {
        endpoint: "https://runner.example/mobile".to_string(),
        token_present: true,
    };
    let mut http = RunnerHttpTransport::new(config.clone());
    let mut websocket = RunnerWebSocketTransport::new(config);

    let http_error = http.execute(long_running_shell_call()).unwrap_err();
    let websocket_error = websocket.execute(long_running_shell_call()).unwrap_err();

    assert_eq!(
        http_error,
        TransportError::Failed("runner http transport is not connected".to_string())
    );
    assert_eq!(
        websocket_error,
        TransportError::Failed("runner websocket transport is not connected".to_string())
    );
}
