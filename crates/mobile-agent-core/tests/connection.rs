use deepseek_mobile_agent_core::{
    CapabilitySet, ConnectionProfile, ConnectionStatus, ExecutionMode, RemoteToolName,
};
use pretty_assertions::assert_eq;

#[test]
fn profiles_express_supported_connection_modes() {
    let bootstrap = ConnectionProfile::bootstrap("local", "Local bootstrap");
    let ssh = ConnectionProfile::ssh("mac", "SSH Mac");
    let powershell = ConnectionProfile::powershell("win", "Windows PowerShell");
    let runner = ConnectionProfile::runner("runner", "Mobile runner");
    let remote_mcp = ConnectionProfile::remote_mcp("mcp", "Remote MCP");

    assert_eq!(bootstrap.mode(), ExecutionMode::Bootstrap);
    assert_eq!(bootstrap.status, ConnectionStatus::ManualBootstrap);
    assert_eq!(ssh.mode(), ExecutionMode::Ssh);
    assert_eq!(ssh.status, ConnectionStatus::Connected);
    assert_eq!(powershell.mode(), ExecutionMode::PowerShell);
    assert_eq!(powershell.status, ConnectionStatus::Connected);
    assert_eq!(runner.mode(), ExecutionMode::Runner);
    assert_eq!(runner.status, ConnectionStatus::Connected);
    assert_eq!(remote_mcp.mode(), ExecutionMode::RemoteMcp);
    assert_eq!(remote_mcp.status, ConnectionStatus::Connected);
}

#[test]
fn tool_availability_delegates_to_current_capabilities_when_reachable() {
    let bootstrap = ConnectionProfile::bootstrap("local", "Local bootstrap");
    let ssh = ConnectionProfile::ssh("mac", "SSH Mac");
    let remote_mcp = ConnectionProfile::remote_mcp("mcp", "Remote MCP");
    let unreachable = ConnectionProfile::unreachable("offline", "Offline Mac");

    assert!(bootstrap.allows_tool(RemoteToolName::BootstrapGuide));
    assert!(!bootstrap.allows_tool(RemoteToolName::ShellExec));
    assert!(ssh.allows_tool(RemoteToolName::ShellExec));
    assert!(!ssh.allows_tool(RemoteToolName::PowerShellExec));
    assert!(remote_mcp.allows_tool(RemoteToolName::McpCall));
    assert!(!remote_mcp.allows_tool(RemoteToolName::ShellExec));
    assert!(!unreachable.allows_tool(RemoteToolName::BootstrapGuide));
}

#[test]
fn bootstrap_profile_upgrades_from_runner_capability_report() {
    let profile = ConnectionProfile::bootstrap("phone-1", "Phone runner").with_runner_capabilities(
        CapabilitySet::from_runner_reported_tools([
            "remote.shell.exec",
            "remote.mcp.call",
            "remote.unknown",
        ]),
    );

    assert_eq!(profile.id, "phone-1");
    assert_eq!(profile.label, "Phone runner");
    assert_eq!(profile.status, ConnectionStatus::Connected);
    assert_eq!(profile.mode(), ExecutionMode::Runner);
    assert!(profile.allows_tool(RemoteToolName::ShellExec));
    assert!(profile.allows_tool(RemoteToolName::McpCall));
    assert!(!profile.allows_tool(RemoteToolName::FileWrite));
}

#[test]
fn next_step_suggestion_reflects_status_and_mode() {
    assert_eq!(
        ConnectionProfile::unreachable("offline", "Offline Mac").next_step_suggestion(),
        "Bootstrap the device to establish a connection."
    );
    assert_eq!(
        ConnectionProfile::bootstrap("local", "Local bootstrap").next_step_suggestion(),
        "Ask the user to manually execute the bootstrap step and paste the output."
    );

    for profile in [
        ConnectionProfile::ssh("mac", "SSH Mac"),
        ConnectionProfile::powershell("win", "Windows PowerShell"),
        ConnectionProfile::runner("runner", "Mobile runner"),
        ConnectionProfile::remote_mcp("mcp", "Remote MCP"),
    ] {
        assert_eq!(
            profile.next_step_suggestion(),
            "Remote execution is available for this connection."
        );
    }
}
