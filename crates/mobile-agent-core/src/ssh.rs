use std::time::Duration;

use crate::event::ToolEventStreamBuilder;
use crate::remote_schema::{RemoteToolCall, RemoteToolName, RemoteToolOutput};
use crate::risk::{RiskAssessment, assess_tool_call};
use crate::transport::{RemoteToolTransport, TransportError};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshConnectionConfig {
    pub host: String,
    pub user: String,
    pub port: u16,
    pub token_present: bool,
    pub key_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshCommandRequest {
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_ms: Option<u64>,
    pub risk: RiskAssessment,
}

impl SshCommandRequest {
    pub fn from_tool_call(call: &RemoteToolCall) -> Result<Self, SshCommandBuildError> {
        if call.name != RemoteToolName::ShellExec {
            return Err(SshCommandBuildError::UnsupportedTool(call.name));
        }

        let command = command_text(&call.arguments)
            .filter(|command| !command.is_empty())
            .ok_or(SshCommandBuildError::MissingCommand)?;

        Ok(Self {
            command,
            cwd: optional_string(&call.arguments, "cwd"),
            timeout_ms: call.arguments.get("timeout_ms").and_then(Value::as_u64),
            risk: assess_tool_call(call),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SshCommandBuildError {
    #[error("SSH command requests only support remote.shell.exec, got {0}")]
    UnsupportedTool(RemoteToolName),
    #[error("SSH command request is missing a command")]
    MissingCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshCommandBuilder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshInvocationSpec {
    pub program: String,
    pub args: Vec<String>,
    pub timeout_ms: Option<u64>,
}

impl SshCommandBuilder {
    #[must_use]
    pub fn build_invocation(
        config: &SshConnectionConfig,
        request: &SshCommandRequest,
    ) -> SshInvocationSpec {
        SshInvocationSpec {
            program: "ssh".to_string(),
            args: vec![
                "-p".to_string(),
                config.port.to_string(),
                format!("{}@{}", config.user, config.host),
                remote_shell_command(request),
            ],
            timeout_ms: request.timeout_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshRunnerOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub timed_out: bool,
}

pub trait SshRunner {
    fn run(
        &mut self,
        spec: &SshInvocationSpec,
        command: &SshCommandRequest,
    ) -> Result<SshRunnerOutput, TransportError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshToolTransport<R> {
    config: SshConnectionConfig,
    runner: R,
}

impl<R> SshToolTransport<R> {
    #[must_use]
    pub fn new(config: SshConnectionConfig, runner: R) -> Self {
        Self { config, runner }
    }

    #[must_use]
    pub const fn config(&self) -> &SshConnectionConfig {
        &self.config
    }

    #[must_use]
    pub const fn runner(&self) -> &R {
        &self.runner
    }

    #[must_use]
    pub const fn runner_mut(&mut self) -> &mut R {
        &mut self.runner
    }
}

impl<R> RemoteToolTransport for SshToolTransport<R>
where
    R: SshRunner,
{
    fn execute(&mut self, call: RemoteToolCall) -> Result<RemoteToolOutput, TransportError> {
        let call_id = call.call_id.clone();
        let command = SshCommandRequest::from_tool_call(&call)
            .map_err(|err| TransportError::Failed(err.to_string()))?;
        let spec = SshCommandBuilder::build_invocation(&self.config, &command);
        let output = self.runner.run(&spec, &command)?;

        Ok(RemoteToolOutput {
            result: command_result(&call_id, &command, &output),
            call_id,
            success: !output.timed_out && output.exit_code == Some(0),
        })
    }
}

fn command_text(arguments: &Value) -> Option<String> {
    ["command", "cmd", "script"]
        .into_iter()
        .find_map(|key| optional_string(arguments, key))
        .map(|command| command.trim().to_string())
}

fn optional_string(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn remote_shell_command(request: &SshCommandRequest) -> String {
    match &request.cwd {
        Some(cwd) => format!("cd {} && {}", shell_quote(cwd), request.command),
        None => request.command.clone(),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn command_result(call_id: &str, command: &SshCommandRequest, output: &SshRunnerOutput) -> Value {
    let duration_ms = duration_ms(output.duration);

    serde_json::json!({
        "status": if output.timed_out { "timed_out" } else { "completed" },
        "exit_code": output.exit_code,
        "stdout": output.stdout,
        "stderr": output.stderr,
        "duration_ms": duration_ms,
        "timeout_ms": command.timeout_ms,
        "timed_out": output.timed_out,
        "stdout_truncated": false,
        "stderr_truncated": false,
        "events": command_events(call_id, command, output, duration_ms),
        "risk": {
            "level": risk_level(command.risk.level),
            "reason": command.risk.reason,
        },
        "runner": {
            "transport": "ssh",
        },
    })
}

fn command_events(
    call_id: &str,
    command: &SshCommandRequest,
    output: &SshRunnerOutput,
    duration_ms: u64,
) -> Vec<crate::MobileEvent> {
    let mut stream = ToolEventStreamBuilder::new(call_id, RemoteToolName::ShellExec.to_string());
    let mut events = vec![stream.started()];

    if !output.stdout.is_empty() {
        events.push(stream.stdout(output.stdout.clone()));
    }

    if !output.stderr.is_empty() {
        events.push(stream.stderr(output.stderr.clone()));
    }

    if output.timed_out {
        events.push(stream.timed_out(
            command.timeout_ms.unwrap_or(duration_ms),
            duration_ms,
            "command timed out",
        ));
    } else {
        events.push(stream.completed(output.exit_code.unwrap_or(-1), duration_ms));
    }

    events
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn risk_level(level: crate::risk::RiskLevel) -> &'static str {
    match level {
        crate::risk::RiskLevel::Low => "low",
        crate::risk::RiskLevel::Medium => "medium",
        crate::risk::RiskLevel::High => "high",
    }
}
