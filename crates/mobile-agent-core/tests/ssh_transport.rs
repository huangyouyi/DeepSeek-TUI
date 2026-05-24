use deepseek_mobile_agent_core::ssh::{
    SshCommandRequest, SshConnectionConfig, SshRunner, SshRunnerOutput, SshToolTransport,
};
use deepseek_mobile_agent_core::{
    RemoteToolCall, RemoteToolName, RemoteToolTransport, RiskLevel, TransportError,
};
use serde_json::json;
use std::time::Duration;

fn shell_call(arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: "call-shell".to_string(),
        name: RemoteToolName::ShellExec,
        arguments,
    }
}

#[derive(Debug, Default)]
struct FakeSshRunner {
    last_command: Option<SshCommandRequest>,
    output: Option<SshRunnerOutput>,
}

impl SshRunner for FakeSshRunner {
    fn run(
        &mut self,
        _spec: &deepseek_mobile_agent_core::ssh::SshInvocationSpec,
        command: &SshCommandRequest,
    ) -> Result<SshRunnerOutput, TransportError> {
        self.last_command = Some(command.clone());
        self.output
            .take()
            .ok_or_else(|| TransportError::Failed("fake runner has no output".to_string()))
    }
}

#[test]
fn ssh_tool_transport_maps_low_risk_success_output() {
    let runner = FakeSshRunner {
        output: Some(SshRunnerOutput {
            stdout: "Linux builder\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            duration: Duration::from_millis(42),
            timed_out: false,
        }),
        ..FakeSshRunner::default()
    };
    let mut transport = SshToolTransport::new(connection(), runner);

    let output = transport
        .execute(shell_call(json!({
            "command": "uname -a",
            "timeout_ms": 1000
        })))
        .expect("fake ssh execution should map output");

    assert!(output.success);
    assert_eq!(output.call_id, "call-shell");
    assert_eq!(output.result["status"], "completed");
    assert_eq!(output.result["exit_code"], 0);
    assert_eq!(output.result["stdout"], "Linux builder\n");
    assert_eq!(output.result["stderr"], "");
    assert_eq!(output.result["duration_ms"], 42);
    assert_eq!(output.result["timeout_ms"], 1000);
    assert_eq!(output.result["risk"]["level"], "low");
    assert_eq!(
        output.result["events"],
        json!([
            {
                "seq": 1,
                "kind": "tool_call_started",
                "payload": {
                    "call_id": "call-shell",
                    "tool": "remote.shell.exec",
                },
            },
            {
                "seq": 2,
                "kind": "tool_stdout",
                "payload": {
                    "call_id": "call-shell",
                    "tool": "remote.shell.exec",
                    "chunk": "Linux builder\n",
                },
            },
            {
                "seq": 3,
                "kind": "tool_call_completed",
                "payload": {
                    "call_id": "call-shell",
                    "tool": "remote.shell.exec",
                    "exit_code": 0,
                    "duration_ms": 42,
                },
            },
        ])
    );
}

#[test]
fn ssh_tool_transport_maps_non_zero_output_without_transport_error() {
    let runner = FakeSshRunner {
        output: Some(SshRunnerOutput {
            stdout: String::new(),
            stderr: "missing\n".to_string(),
            exit_code: Some(127),
            duration: Duration::from_millis(12),
            timed_out: false,
        }),
        ..FakeSshRunner::default()
    };
    let mut transport = SshToolTransport::new(connection(), runner);

    let output = transport
        .execute(shell_call(json!({ "command": "missing-tool" })))
        .expect("remote non-zero exit is a tool output");

    assert!(!output.success);
    assert_eq!(output.result["status"], "completed");
    assert_eq!(output.result["exit_code"], 127);
    assert_eq!(output.result["stderr"], "missing\n");
    assert_eq!(output.result["timed_out"], false);
}

#[test]
fn ssh_tool_transport_maps_timeout_metadata() {
    let runner = FakeSshRunner {
        output: Some(SshRunnerOutput {
            stdout: "partial".to_string(),
            stderr: "timeout".to_string(),
            exit_code: None,
            duration: Duration::from_millis(2500),
            timed_out: true,
        }),
        ..FakeSshRunner::default()
    };
    let mut transport = SshToolTransport::new(connection(), runner);

    let output = transport
        .execute(shell_call(json!({
            "command": "sleep 60",
            "timeout_ms": 2500
        })))
        .expect("timeout maps to remote tool output");

    assert!(!output.success);
    assert_eq!(output.result["status"], "timed_out");
    assert_eq!(output.result["exit_code"], serde_json::Value::Null);
    assert_eq!(output.result["duration_ms"], 2500);
    assert_eq!(output.result["timeout_ms"], 2500);
    assert_eq!(output.result["timed_out"], true);
    assert_eq!(
        output.result["events"],
        json!([
            {
                "seq": 1,
                "kind": "tool_call_started",
                "payload": {
                    "call_id": "call-shell",
                    "tool": "remote.shell.exec",
                },
            },
            {
                "seq": 2,
                "kind": "tool_stdout",
                "payload": {
                    "call_id": "call-shell",
                    "tool": "remote.shell.exec",
                    "chunk": "partial",
                },
            },
            {
                "seq": 3,
                "kind": "tool_stderr",
                "payload": {
                    "call_id": "call-shell",
                    "tool": "remote.shell.exec",
                    "chunk": "timeout",
                },
            },
            {
                "seq": 4,
                "kind": "tool_timed_out",
                "payload": {
                    "call_id": "call-shell",
                    "tool": "remote.shell.exec",
                    "timeout_ms": 2500,
                    "duration_ms": 2500,
                    "reason": "command timed out",
                },
            },
        ])
    );
}

#[test]
fn ssh_tool_transport_rejects_non_shell_tool() {
    let mut transport = SshToolTransport::new(connection(), FakeSshRunner::default());

    let err = transport
        .execute(RemoteToolCall {
            call_id: "call-file".to_string(),
            name: RemoteToolName::FileRead,
            arguments: json!({ "path": "/tmp/example.txt" }),
        })
        .unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed(
            "SSH command requests only support remote.shell.exec, got remote.file.read".to_string()
        )
    );
}

#[test]
fn ssh_tool_transport_rejects_missing_command() {
    let mut transport = SshToolTransport::new(connection(), FakeSshRunner::default());

    let err = transport
        .execute(shell_call(json!({ "cwd": "/tmp" })))
        .unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed("SSH command request is missing a command".to_string())
    );
}

#[test]
fn ssh_tool_transport_preserves_high_risk_assessment() {
    let runner = FakeSshRunner {
        output: Some(SshRunnerOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
            duration: Duration::from_millis(1),
            timed_out: false,
        }),
        ..FakeSshRunner::default()
    };
    let mut transport = SshToolTransport::new(connection(), runner);

    let output = transport
        .execute(shell_call(json!({ "command": "sudo true" })))
        .expect("fake ssh execution should map risk");

    assert_eq!(output.result["risk"]["level"], "high");
    assert_eq!(
        output.result["risk"]["reason"],
        "command can make privileged, destructive, or security changes"
    );
    assert_eq!(
        transport.runner().last_command.as_ref().unwrap().risk.level,
        RiskLevel::High
    );
}

fn connection() -> SshConnectionConfig {
    SshConnectionConfig {
        host: "linux-builder.local".to_string(),
        user: "mobile".to_string(),
        port: 2222,
        token_present: false,
        key_present: false,
    }
}
