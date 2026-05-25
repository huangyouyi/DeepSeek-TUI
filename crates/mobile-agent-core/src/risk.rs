use crate::remote_schema::{RemoteToolCall, RemoteToolName};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub reason: String,
}

impl RiskAssessment {
    #[must_use]
    pub fn low(reason: impl Into<String>) -> Self {
        Self {
            level: RiskLevel::Low,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn medium(reason: impl Into<String>) -> Self {
        Self {
            level: RiskLevel::Medium,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn high(reason: impl Into<String>) -> Self {
        Self {
            level: RiskLevel::High,
            reason: reason.into(),
        }
    }
}

#[must_use]
pub fn assess_tool_call(call: &RemoteToolCall) -> RiskAssessment {
    match call.name {
        RemoteToolName::FileRead
        | RemoteToolName::DiagnoseSystem
        | RemoteToolName::BrowserExtractText
        | RemoteToolName::BootstrapGuide => RiskAssessment::low("read-only tool"),
        RemoteToolName::ShellExec => assess_shell_command(&call.arguments),
        RemoteToolName::PowerShellExec => assess_powershell_command(&call.arguments),
        RemoteToolName::PackageInstall => assess_package_install(&call.arguments),
        RemoteToolName::FileWrite | RemoteToolName::BrowserOpen | RemoteToolName::BrowserClick => {
            RiskAssessment::medium("tool may change local state or user-visible context")
        }
        RemoteToolName::McpCall => RiskAssessment::medium("MCP call needs user review"),
    }
}

fn assess_shell_command(arguments: &Value) -> RiskAssessment {
    let command = command_text(arguments);

    if is_high_risk_shell(&command) {
        return RiskAssessment::high(
            "command can make privileged, destructive, or security changes",
        );
    }

    if is_read_only_diagnostic_shell(&command) {
        return RiskAssessment::low("read-only diagnostic command");
    }

    RiskAssessment::medium("shell command needs user review")
}

fn assess_powershell_command(arguments: &Value) -> RiskAssessment {
    let command = command_text(arguments);

    if is_high_risk_powershell(&command) {
        return RiskAssessment::high(
            "PowerShell command can change registry, UAC, services, or policy",
        );
    }

    if is_read_only_diagnostic_powershell(&command) {
        return RiskAssessment::low("read-only PowerShell diagnostic command");
    }

    RiskAssessment::medium("PowerShell command needs user review")
}

fn assess_package_install(arguments: &Value) -> RiskAssessment {
    if arguments
        .get("dry_run")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return RiskAssessment::low("package install dry-run");
    }

    RiskAssessment::medium("package install changes local system state")
}

fn command_text(arguments: &Value) -> String {
    for key in ["command", "cmd", "script"] {
        if let Some(command) = arguments.get(key).and_then(Value::as_str) {
            return command.trim().to_string();
        }
    }

    arguments.to_string()
}

fn is_read_only_diagnostic_shell(command: &str) -> bool {
    let trimmed = command.trim();
    let lower = trimmed.to_ascii_lowercase();

    if has_shell_control(trimmed) && !matches_path_echo(&lower) {
        return false;
    }

    lower == "sw_vers"
        || lower == "xcode-select -p"
        || lower == "brew doctor"
        || lower == "echo \"$path\""
        || lower == "echo '$path'"
        || lower == "echo $path"
        || lower == "uname"
        || lower.starts_with("uname -")
        || lower.starts_with("command -v ")
}

fn matches_path_echo(lower: &str) -> bool {
    matches!(lower, "echo \"$path\"" | "echo '$path'" | "echo $path")
}

fn has_shell_control(command: &str) -> bool {
    command
        .chars()
        .any(|ch| matches!(ch, ';' | '&' | '|' | '>' | '<' | '`'))
}

fn is_high_risk_shell(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();

    contains_token(&lower, "sudo")
        || lower.contains("rm -rf")
        || lower.contains("rm -fr")
        || lower.contains("chmod -r")
        || lower.contains("chown -r")
        || writes_shell_profile(&lower)
        || executes_downloaded_installer(&lower)
        || changes_network_or_security(&lower)
}

fn writes_shell_profile(lower: &str) -> bool {
    let writes = lower.contains(">>")
        || lower.contains(" >")
        || lower.contains("tee -a")
        || lower.contains("tee ")
        || lower.contains("sed -i");
    let profile = lower.contains(".zshrc")
        || lower.contains(".bashrc")
        || lower.contains(".bash_profile")
        || lower.contains(".profile")
        || lower.contains("config.fish");

    writes && profile
}

fn executes_downloaded_installer(lower: &str) -> bool {
    ((lower.contains("curl ") || lower.contains("wget "))
        && (lower.contains("| sh")
            || lower.contains("| bash")
            || lower.contains("| zsh")
            || lower.contains("| fish")
            || lower.contains("bash <(")
            || lower.contains("sh <(")))
        || lower.contains("installer -pkg")
}

fn changes_network_or_security(lower: &str) -> bool {
    lower.contains("networksetup ")
        || lower.contains("scutil --set")
        || lower.contains("spctl ")
        || lower.contains("csrutil ")
        || lower.contains("pfctl ")
        || lower.contains("iptables ")
        || lower.contains("nft ")
        || lower.contains("ufw ")
        || lower.contains("firewall-cmd ")
        || lower.contains("sysctl -w")
        || lower.contains("setenforce ")
}

fn is_high_risk_powershell(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();

    lower.contains("set-executionpolicy")
        || lower.contains("winget install")
        || lower.contains("enablelua")
        || lower.contains("hklm:")
        || lower.contains("hkcu:")
        || lower.contains("reg add")
        || lower.contains("reg delete")
        || lower.contains("set-itemproperty")
        || lower.contains("new-itemproperty")
        || lower.contains("remove-itemproperty")
        || lower.contains("set-service")
        || lower.contains("new-service")
        || lower.contains("stop-service")
        || lower.contains("start-service")
        || lower.contains("sc.exe config")
        || lower.contains("sc.exe delete")
        || lower.contains("netsh advfirewall")
        || changes_windows_path(&lower)
        || requests_windows_admin(&lower)
}

fn is_read_only_diagnostic_powershell(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();

    lower == "$psversiontable.psversion.tostring()"
        || lower == "python --version"
        || lower == "py --version"
        || lower == "node --version"
        || lower == "npm --version"
        || lower == "winget --version"
        || lower == "get-command python, py, node, npm, winget, pwsh -erroraction silentlycontinue"
        || lower == "[environment]::getenvironmentvariable('path', 'user')"
        || lower == "[environment]::getenvironmentvariable('path', 'machine')"
        || lower == "[environment]::getenvironmentvariable('path', 'process')"
}

fn changes_windows_path(lower: &str) -> bool {
    (lower.contains("setx") && lower.contains("path"))
        || (lower.contains("[environment]::setenvironmentvariable") && lower.contains("path"))
        || lower.contains("$env:path =")
        || lower.contains("$env:path=")
}

fn requests_windows_admin(lower: &str) -> bool {
    lower.contains("-verb runas")
        || lower.contains("runas.exe")
        || lower.contains("start-process powershell") && lower.contains("runas")
}

fn contains_token(lower: &str, token: &str) -> bool {
    lower
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
        .any(|part| part == token)
}
