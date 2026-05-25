use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName};

use crate::diagnostics::preset_diagnostics;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentToolPolicy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentToolDecision {
    RunLowRisk(ShellCommand),
    RequireApproval(ShellCommand),
    Reject { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellCommand {
    pub command: String,
    pub cwd: Option<String>,
}

impl AgentToolPolicy {
    #[must_use]
    pub fn classify(&self, call: &RemoteToolCall) -> AgentToolDecision {
        if call.name != RemoteToolName::ShellExec {
            return AgentToolDecision::Reject {
                reason: format!("unsupported agent tool: {}", call.name),
            };
        }

        let Some(command) = string_argument(call, "command") else {
            return AgentToolDecision::Reject {
                reason: "remote.shell.exec requires a non-empty command".to_string(),
            };
        };
        let shell_command = ShellCommand {
            command,
            cwd: string_argument(call, "cwd"),
        };

        if is_low_risk_diagnostic_command(&shell_command.command) {
            AgentToolDecision::RunLowRisk(shell_command)
        } else {
            AgentToolDecision::RequireApproval(shell_command)
        }
    }
}

#[must_use]
pub fn is_low_risk_diagnostic_command(command: &str) -> bool {
    preset_diagnostics()
        .into_iter()
        .any(|preset| preset.command == command)
}

fn string_argument(call: &RemoteToolCall, key: &str) -> Option<String> {
    call.arguments
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
