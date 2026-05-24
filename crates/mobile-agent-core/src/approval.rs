use crate::remote_schema::{CommandLease, CommandLeaseAction, RemoteToolCall, RemoteToolName};
use crate::risk::{RiskAssessment, RiskLevel, assess_tool_call};

#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalRequest {
    pub approval_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub call: RemoteToolCall,
    pub risk: RiskAssessment,
}

impl ApprovalRequest {
    #[must_use]
    pub fn command_lease(
        &self,
        idempotency_key: impl Into<String>,
        expires_at_unix_ms: Option<u64>,
    ) -> Option<CommandLease> {
        let approved_action = command_lease_action(&self.call)?;
        let lease = CommandLease::new(
            format!("lease-{}", self.approval_id),
            idempotency_key,
            approved_action,
        );

        Some(match expires_at_unix_ms {
            Some(expires_at_unix_ms) => lease.with_expires_at_unix_ms(expires_at_unix_ms),
            None => lease,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

#[derive(Debug, Clone, Default)]
pub struct ApprovalGate;

impl ApprovalGate {
    #[must_use]
    pub fn evaluate(&self, call: &RemoteToolCall) -> Option<ApprovalRequest> {
        let risk = if is_runner_maintenance_call(call) {
            RiskAssessment::high("runner maintenance can update or remove the active runner")
        } else {
            assess_tool_call(call)
        };

        Self::requires_approval(risk.level).then(|| ApprovalRequest {
            approval_id: format!("approval-{}", call.call_id),
            call_id: call.call_id.clone(),
            tool_name: tool_name(&call.name).to_string(),
            call: call.clone(),
            risk,
        })
    }

    #[must_use]
    pub fn requires_approval(risk: RiskLevel) -> bool {
        !matches!(risk, RiskLevel::Low)
    }
}

fn is_runner_maintenance_call(call: &RemoteToolCall) -> bool {
    if call.name != RemoteToolName::McpCall {
        return false;
    }

    let server = call
        .arguments
        .get("server")
        .and_then(serde_json::Value::as_str);
    let tool = call
        .arguments
        .get("tool")
        .and_then(serde_json::Value::as_str);

    server == Some("mobile-maintenance")
        && matches!(
            tool,
            Some("runner.maintenance.plan")
                | Some("runner.maintenance.self_update")
                | Some("runner.maintenance.uninstall")
        )
}

fn command_lease_action(call: &RemoteToolCall) -> Option<CommandLeaseAction> {
    match call.name {
        RemoteToolName::ShellExec => {
            let command = string_argument(&call.arguments, &["command", "cmd", "script"])?;
            Some(CommandLeaseAction::new(
                call.name.as_str(),
                command,
                string_argument(&call.arguments, &["cwd"]),
            ))
        }
        RemoteToolName::PowerShellExec => {
            let command = string_argument(&call.arguments, &["script", "command"])?;
            Some(CommandLeaseAction::new(
                call.name.as_str(),
                command,
                string_argument(&call.arguments, &["working_directory", "cwd"]),
            ))
        }
        _ => None,
    }
}

fn string_argument(arguments: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        arguments
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn tool_name(name: &RemoteToolName) -> &'static str {
    match name {
        RemoteToolName::ShellExec => "shell_exec",
        RemoteToolName::PowerShellExec => "power_shell_exec",
        RemoteToolName::FileRead => "file_read",
        RemoteToolName::FileWrite => "file_write",
        RemoteToolName::DiagnoseSystem => "diagnose_system",
        RemoteToolName::PackageInstall => "package_install",
        RemoteToolName::BrowserOpen => "browser_open",
        RemoteToolName::BrowserExtractText => "browser_extract_text",
        RemoteToolName::BrowserClick => "browser_click",
        RemoteToolName::BootstrapGuide => "bootstrap_guide",
        RemoteToolName::McpCall => "mcp_call",
    }
}
