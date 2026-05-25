use deepseek_mobile_agent_core::ssh::{
    SshCommandBuildError, SshCommandBuilder, SshCommandRequest, SshConnectionConfig,
};
use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName, RiskLevel};
use serde_json::json;

fn call(call_id: &str, name: RemoteToolName, arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name,
        arguments,
    }
}

#[test]
fn connection_config_records_endpoint_and_auth_presence_without_secret_material() {
    let config = SshConnectionConfig {
        host: "mac.local".to_string(),
        user: "dee".to_string(),
        port: 2222,
        token_present: true,
        key_present: false,
    };

    assert_eq!(config.host, "mac.local");
    assert_eq!(config.user, "dee");
    assert_eq!(config.port, 2222);
    assert!(config.token_present);
    assert!(!config.key_present);
}

#[test]
fn shell_exec_tool_call_builds_ssh_command_request_with_low_risk_for_read_only_command() {
    let request = SshCommandRequest::from_tool_call(&call(
        "call-1",
        RemoteToolName::ShellExec,
        json!({
            "command": "uname -a",
            "cwd": "/tmp",
            "timeout_ms": 5000
        }),
    ))
    .expect("shell exec should build");

    assert_eq!(request.command, "uname -a");
    assert_eq!(request.cwd.as_deref(), Some("/tmp"));
    assert_eq!(request.timeout_ms, Some(5000));
    assert_eq!(request.risk.level, RiskLevel::Low);
}

#[test]
fn shell_exec_tool_call_marks_sudo_as_high_risk() {
    let request = SshCommandRequest::from_tool_call(&call(
        "call-2",
        RemoteToolName::ShellExec,
        json!({
            "cmd": "sudo xcode-select --switch /Applications/Xcode.app"
        }),
    ))
    .expect("shell exec should build");

    assert_eq!(
        request.command,
        "sudo xcode-select --switch /Applications/Xcode.app"
    );
    assert_eq!(request.cwd, None);
    assert_eq!(request.timeout_ms, None);
    assert_eq!(request.risk.level, RiskLevel::High);
}

#[test]
fn non_shell_exec_tool_call_returns_clear_error() {
    let err = SshCommandRequest::from_tool_call(&call(
        "call-3",
        RemoteToolName::FileRead,
        json!({ "path": "/tmp/example.txt" }),
    ))
    .unwrap_err();

    assert_eq!(
        err,
        SshCommandBuildError::UnsupportedTool(RemoteToolName::FileRead)
    );
    assert_eq!(
        err.to_string(),
        "SSH command requests only support remote.shell.exec, got remote.file.read"
    );
}

#[test]
fn ssh_invocation_spec_wraps_cwd_and_timeout_without_secrets() {
    let config = SshConnectionConfig {
        host: "linux-builder.local".to_string(),
        user: "mobile".to_string(),
        port: 2222,
        token_present: true,
        key_present: true,
    };
    let request = SshCommandRequest::from_tool_call(&call(
        "call-4",
        RemoteToolName::ShellExec,
        json!({
            "command": "printf ok",
            "cwd": "/tmp/with 'quote'",
            "timeout_ms": 1500
        }),
    ))
    .expect("shell exec should build");

    let spec = SshCommandBuilder::build_invocation(&config, &request);

    assert_eq!(spec.program, "ssh");
    assert_eq!(
        spec.args,
        vec![
            "-p",
            "2222",
            "mobile@linux-builder.local",
            "cd '/tmp/with '\\''quote'\\''' && printf ok"
        ]
    );
    assert_eq!(spec.timeout_ms, Some(1500));
    assert!(!format!("{spec:?}").contains("token"));
    assert!(!format!("{spec:?}").contains("key"));
}
