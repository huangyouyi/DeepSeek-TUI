use deepseek_mobile_agent_core::{
    BootstrapSession, ConnectionProfile, ConnectionStatus, ExecutionMode, RemoteToolName,
};
use pretty_assertions::assert_eq;

fn runner_capability_report() -> &'static str {
    r#"
== kai-runner capability report ==
{
  "runner": {
    "version": "0.1.0",
    "endpoint": "http://127.0.0.1:39117/mobile"
  },
  "capabilities": {
    "mode": "runner",
    "tools": [
      "remote.shell.exec",
      "remote.file.read",
      "remote.mcp.call"
    ]
  }
}
"#
}

#[test]
fn macos_bootstrap_install_report_upgrades_mobile_core_to_runner_mode_without_network() {
    let mut bootstrap = BootstrapSession::default();
    let initial = ConnectionProfile::bootstrap("mac", "User Mac");

    assert!(
        bootstrap.steps.is_empty(),
        "new phone-guided session starts without a runner"
    );
    assert_eq!(initial.status, ConnectionStatus::ManualBootstrap);
    assert_eq!(initial.mode(), ExecutionMode::Bootstrap);
    assert!(initial.allows_tool(RemoteToolName::BootstrapGuide));
    assert!(!initial.allows_tool(RemoteToolName::ShellExec));

    let install_step = bootstrap.macos_runner_install_step();
    assert_eq!(install_step.step_id, "macos-runner-install-bootstrap");
    assert!(install_step.command.contains("curl -fsSLO"));
    assert!(install_step.command.contains("./kai-runner pair"));

    bootstrap
        .submit_output(&install_step.step_id, runner_capability_report())
        .expect("phone should record the user-pasted runner report");

    let upgraded = initial
        .upgrade_to_runner_from_pasted_report(runner_capability_report())
        .expect("pasted runner capability report should upgrade the connection");

    assert_eq!(upgraded.status, ConnectionStatus::Connected);
    assert_eq!(upgraded.mode(), ExecutionMode::Runner);
    assert!(upgraded.allows_tool(RemoteToolName::ShellExec));
    assert!(upgraded.allows_tool(RemoteToolName::FileRead));
    assert!(upgraded.allows_tool(RemoteToolName::McpCall));
    assert!(!upgraded.allows_tool(RemoteToolName::FileWrite));
}

#[test]
fn windows_bootstrap_install_report_upgrades_mobile_core_to_runner_mode_without_network() {
    let mut bootstrap = BootstrapSession::default();
    let initial = ConnectionProfile::bootstrap("windows", "User Windows PC");
    assert_eq!(initial.mode(), ExecutionMode::Bootstrap);

    let install_step = bootstrap.windows_runner_install_step();
    assert_eq!(install_step.step_id, "windows-runner-install-bootstrap");
    assert!(install_step.command.contains("Invoke-WebRequest"));
    assert!(install_step.command.contains("pair"));

    bootstrap
        .submit_output(&install_step.step_id, runner_capability_report())
        .expect("phone should record the user-pasted runner report");

    let upgraded = initial
        .upgrade_to_runner_from_pasted_report(runner_capability_report())
        .expect("pasted runner capability report should upgrade the connection");

    assert_eq!(upgraded.status, ConnectionStatus::Connected);
    assert_eq!(upgraded.mode(), ExecutionMode::Runner);
    assert_eq!(
        upgraded.capabilities.tools,
        vec![
            RemoteToolName::ShellExec,
            RemoteToolName::FileRead,
            RemoteToolName::McpCall,
        ]
    );
}

#[test]
fn offline_fallback_output_keeps_mobile_core_in_bootstrap_mode_without_fake_runner() {
    let mut bootstrap = BootstrapSession::default();
    let initial = ConnectionProfile::bootstrap("offline-mac", "Offline Mac");

    let install_step = bootstrap.macos_runner_install_step();
    let offline_output = "Network unavailable. Offline fallback: copy the macOS kai-runner release to ~/Downloads, then run the installer locally.";

    bootstrap
        .submit_output(&install_step.step_id, offline_output)
        .expect("offline fallback output should be recorded");

    let error = initial
        .clone()
        .upgrade_to_runner_from_pasted_report(offline_output)
        .expect_err("offline fallback is not a runner capability report");

    assert_eq!(
        error.to_string(),
        "runner capability report did not include any recognized tools"
    );
    assert_eq!(initial.status, ConnectionStatus::ManualBootstrap);
    assert_eq!(initial.mode(), ExecutionMode::Bootstrap);
    assert!(initial.allows_tool(RemoteToolName::BootstrapGuide));
    assert!(!initial.allows_tool(RemoteToolName::ShellExec));

    assert_eq!(
        bootstrap.steps[0].submitted_output.as_deref(),
        Some(offline_output)
    );
}
