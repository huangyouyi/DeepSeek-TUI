use deepseek_mobile_agent_core::{CapabilitySet, ExecutionMode, RemoteToolName};
use pretty_assertions::assert_eq;

fn tool_names(capabilities: &CapabilitySet) -> Vec<RemoteToolName> {
    capabilities.tools.clone()
}

#[test]
fn bootstrap_capabilities_only_allow_bootstrap_guide() {
    let capabilities = CapabilitySet::bootstrap();

    assert_eq!(capabilities.mode, ExecutionMode::Bootstrap);
    assert_eq!(
        tool_names(&capabilities),
        vec![RemoteToolName::BootstrapGuide]
    );
    assert!(capabilities.allows(RemoteToolName::BootstrapGuide));
    assert!(!capabilities.allows(RemoteToolName::ShellExec));
    assert!(!capabilities.allows(RemoteToolName::McpCall));
}

#[test]
fn ssh_capabilities_allow_shell_diagnose_and_bootstrap() {
    let capabilities = CapabilitySet::ssh();

    assert_eq!(capabilities.mode, ExecutionMode::Ssh);
    assert!(capabilities.allows(RemoteToolName::ShellExec));
    assert!(capabilities.allows(RemoteToolName::DiagnoseSystem));
    assert!(capabilities.allows(RemoteToolName::BootstrapGuide));
    assert!(!capabilities.allows(RemoteToolName::PowerShellExec));
}

#[test]
fn powershell_capabilities_allow_powershell_diagnose_and_bootstrap() {
    let capabilities = CapabilitySet::powershell();

    assert_eq!(capabilities.mode, ExecutionMode::PowerShell);
    assert!(capabilities.allows(RemoteToolName::PowerShellExec));
    assert!(capabilities.allows(RemoteToolName::DiagnoseSystem));
    assert!(capabilities.allows(RemoteToolName::BootstrapGuide));
    assert!(!capabilities.allows(RemoteToolName::ShellExec));
}

#[test]
fn runner_capabilities_allow_full_remote_tool_surface() {
    let capabilities = CapabilitySet::runner();

    assert_eq!(capabilities.mode, ExecutionMode::Runner);
    for tool in [
        RemoteToolName::ShellExec,
        RemoteToolName::PowerShellExec,
        RemoteToolName::FileRead,
        RemoteToolName::FileWrite,
        RemoteToolName::DiagnoseSystem,
        RemoteToolName::PackageInstall,
        RemoteToolName::BrowserOpen,
        RemoteToolName::BrowserExtractText,
        RemoteToolName::BrowserClick,
        RemoteToolName::BootstrapGuide,
        RemoteToolName::McpCall,
    ] {
        assert!(capabilities.allows(tool), "{tool} should be allowed");
    }
}

#[test]
fn remote_mcp_capabilities_allow_mcp_and_bootstrap() {
    let capabilities = CapabilitySet::remote_mcp();

    assert_eq!(capabilities.mode, ExecutionMode::RemoteMcp);
    assert!(capabilities.allows(RemoteToolName::McpCall));
    assert!(capabilities.allows(RemoteToolName::BootstrapGuide));
    assert!(!capabilities.allows(RemoteToolName::ShellExec));
}

#[test]
fn runner_reported_tool_names_ignore_unknown_tools_and_deduplicate_known_tools() {
    let capabilities = CapabilitySet::from_runner_reported_tools([
        "remote.shell.exec",
        "remote.file.delete",
        "remote.shell.exec",
        "remote.mcp.call",
        "remote.browser.open",
    ]);

    assert_eq!(capabilities.mode, ExecutionMode::Runner);
    assert_eq!(
        tool_names(&capabilities),
        vec![
            RemoteToolName::ShellExec,
            RemoteToolName::McpCall,
            RemoteToolName::BrowserOpen,
        ]
    );
    assert!(capabilities.allows(RemoteToolName::ShellExec));
    assert!(capabilities.allows(RemoteToolName::McpCall));
    assert!(capabilities.allows(RemoteToolName::BrowserOpen));
    assert!(!capabilities.allows(RemoteToolName::FileWrite));
}
