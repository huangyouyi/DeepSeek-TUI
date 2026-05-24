use deepseek_mobile_agent_core::{
    FakeTransport, RemoteToolCall, RemoteToolName, RemoteToolOutput, RemoteToolTransport,
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
fn fake_transport_returns_preconfigured_shell_exec_output() {
    let expected = RemoteToolOutput {
        call_id: "call-1".to_string(),
        success: true,
        result: json!({
            "stdout": "hello\n",
            "stderr": "",
            "exit_code": 0
        }),
    };
    let mut transport = FakeTransport::default();
    transport.push_output(expected.clone());

    let output = transport
        .execute(shell_call("call-1", "printf hello"))
        .unwrap();

    assert_eq!(output, expected);
}

#[test]
fn fake_transport_returns_preconfigured_failure() {
    let mut transport = FakeTransport::default();
    transport.push_error("remote shell exited with 127");

    let err = transport
        .execute(shell_call("call-2", "missing-command"))
        .unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed("remote shell exited with 127".to_string())
    );
}

#[test]
fn fake_transport_records_executed_calls_in_order() {
    let mut transport = FakeTransport::default();
    transport.push_output(RemoteToolOutput {
        call_id: "call-1".to_string(),
        success: true,
        result: json!({ "stdout": "one\n", "exit_code": 0 }),
    });
    transport.push_output(RemoteToolOutput {
        call_id: "call-2".to_string(),
        success: true,
        result: json!({ "stdout": "two\n", "exit_code": 0 }),
    });

    let _ = transport.execute(shell_call("call-1", "echo one")).unwrap();
    let _ = transport.execute(shell_call("call-2", "echo two")).unwrap();

    let calls = transport.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].call_id, "call-1");
    assert_eq!(calls[0].name, RemoteToolName::ShellExec);
    assert_eq!(calls[0].arguments, json!({ "command": "echo one" }));
    assert_eq!(calls[1].call_id, "call-2");
    assert_eq!(calls[1].name, RemoteToolName::ShellExec);
    assert_eq!(calls[1].arguments, json!({ "command": "echo two" }));
}
