use deepseek_mobile_agent_core::uniffi_api::IosMobileAgentCore;
use deepseek_mobile_agent_core::{BootstrapSession, MobileAgentCore, MobileEventKind};

#[test]
fn homebrew_bootstrap_step_contains_diagnostic_commands() {
    let mut bootstrap = BootstrapSession::default();

    let step = bootstrap.homebrew_bootstrap_step();

    assert_eq!(step.step_id, "homebrew-xcode-clt-diagnostics");
    assert!(step.command.contains("uname -a"));
    assert!(step.command.contains("sw_vers"));
    assert!(step.command.contains("command -v brew"));
    assert!(step.command.contains("xcode-select -p"));
    assert!(step.command.contains("echo \"$PATH\""));
    assert_eq!(bootstrap.steps.len(), 1);
}

#[test]
fn bootstrap_session_records_user_submitted_output() {
    let mut bootstrap = BootstrapSession::default();
    let step = bootstrap.homebrew_bootstrap_step();

    bootstrap
        .submit_output(&step.step_id, "Darwin example\n/usr/local/bin/brew")
        .expect("step output should be accepted");

    let recorded = bootstrap.steps.first().expect("step should be recorded");
    assert_eq!(
        recorded.submitted_output.as_deref(),
        Some("Darwin example\n/usr/local/bin/brew")
    );
}

#[test]
fn macos_runner_install_bootstrap_uses_system_shell_and_diagnoses_first() {
    let mut bootstrap = BootstrapSession::default();

    let step = bootstrap.macos_runner_install_step();

    assert_eq!(step.step_id, "macos-runner-install-bootstrap");
    assert_eq!(step.title, "Install the Mac runner");
    assert_eq!(step.commands.len(), 1);
    let command = &step.commands[0];
    assert!(command.contains("# Diagnose first"));
    assert!(command.contains("uname -a"));
    assert!(command.contains("sw_vers"));
    assert!(command.contains("command -v curl"));
    assert!(command.contains("curl -fsSL"));
    assert!(!command.contains("node "));
    assert!(!command.contains("python"));
    assert!(!command.contains("cargo "));
    assert!(step.explanation.contains("diagnostic"));
    assert!(step.risk.contains("installs"));
    assert!(step.approval_required);
    assert!(step.expected_output.contains("runner"));
    assert_eq!(bootstrap.steps.len(), 1);
}

#[test]
fn windows_runner_install_bootstrap_uses_powershell_and_manual_download_path() {
    let mut bootstrap = BootstrapSession::default();

    let step = bootstrap.windows_runner_install_step();

    assert_eq!(step.step_id, "windows-runner-install-bootstrap");
    assert_eq!(step.title, "Install the Windows runner");
    assert_eq!(step.commands.len(), 1);
    let command = &step.commands[0];
    assert!(command.contains("PowerShell"));
    assert!(command.contains("$PSVersionTable.PSVersion"));
    assert!(command.contains("Test-NetConnection"));
    assert!(command.contains("Invoke-WebRequest"));
    assert!(command.contains("Start-Process"));
    assert!(command.contains("Downloads"));
    assert!(!command.contains("node "));
    assert!(!command.contains("python"));
    assert!(!command.contains("cargo "));
    assert!(step.explanation.contains("diagnostic"));
    assert!(step.risk.contains("installer"));
    assert!(step.approval_required);
    assert!(step.expected_output.contains("runner"));
}

#[test]
fn windows_runner_install_bootstrap_guides_offline_winget_policy_and_path_failures() {
    let mut bootstrap = BootstrapSession::default();

    let step = bootstrap.windows_runner_install_step();
    let command = &step.commands[0];

    assert!(command.contains("Test-NetConnection github.com -Port 443"));
    assert!(command.contains("Offline fallback"));
    assert!(command.contains("Get-Command winget -ErrorAction SilentlyContinue"));
    assert!(command.contains("winget --version"));
    assert!(command.contains("winget is missing or broken"));
    assert!(command.contains("PowerShell execution policy"));
    assert!(command.contains("Set-ExecutionPolicy -Scope CurrentUser RemoteSigned"));
    assert!(command.contains("PATH"));
    assert!(command.contains("$env:PATH"));
    assert!(command.contains("Path fallback"));
    assert!(step.explanation.contains("offline fallback"));
    assert!(step.explanation.contains("winget"));
    assert!(step.explanation.contains("execution policy"));
    assert!(step.explanation.contains("PATH"));
    assert!(step.risk.contains("Low: diagnostics"));
    assert!(step.risk.contains("Approval required"));
    assert!(step.expected_output.contains("offline fallback"));
    assert!(step.expected_output.contains("winget"));
    assert!(step.expected_output.contains("execution policy"));
    assert!(step.expected_output.contains("PATH"));
}

#[test]
fn windows_runner_install_bootstrap_separates_diagnostics_from_approved_repairs() {
    let mut bootstrap = BootstrapSession::default();

    let step = bootstrap.windows_runner_install_step();
    let command = &step.commands[0];

    assert!(command.contains("== Low-risk diagnostics =="));
    assert!(command.contains("== Approval required before install or repair =="));
    assert!(command.contains("Read-Host"));
    assert!(command.contains("'YES'"));
    assert!(command.contains("No install or repair was run"));
    assert!(step.approval_required);
}

#[test]
fn runner_install_bootstrap_includes_offline_fallback_steps() {
    let mut bootstrap = BootstrapSession::default();

    let macos = bootstrap.macos_runner_install_step();
    let windows = bootstrap.windows_runner_install_step();

    for step in [macos, windows] {
        assert!(step.explanation.contains("offline"));
        assert!(step.explanation.contains("download"));
        assert!(step.explanation.contains("copy"));
        assert!(step.expected_output.contains("offline"));
    }
}

#[test]
fn mobile_agent_core_appends_bootstrap_steps_outputs_and_events() {
    let mut core = MobileAgentCore::new();
    let session_id = core.create_session("phone rescue");

    let step_event = core
        .homebrew_bootstrap_step(&session_id)
        .expect("step should be appended");
    assert_eq!(step_event.seq, 2);
    assert_eq!(step_event.kind, MobileEventKind::BootstrapStepCreated);

    let output_event = core
        .submit_bootstrap_output(
            &session_id,
            "homebrew-xcode-clt-diagnostics",
            "brew missing",
        )
        .expect("output should be recorded");
    assert_eq!(output_event.seq, 3);

    let session = core.session(&session_id).expect("session should exist");
    assert_eq!(session.events.len(), 3);
    assert_eq!(
        session.bootstrap.steps[0].submitted_output.as_deref(),
        Some("brew missing")
    );
}

#[test]
fn ios_facade_exposes_minimal_bootstrap_flow() {
    let mut core = IosMobileAgentCore::new();
    let session_id = core.create_session("phone rescue".to_string());

    let step = core
        .homebrew_bootstrap_step(session_id.clone())
        .expect("step should be returned to Swift");
    assert_eq!(step.step_id, "homebrew-xcode-clt-diagnostics");
    assert!(step.command.contains("command -v brew"));

    let event = core
        .submit_bootstrap_output(
            session_id,
            "homebrew-xcode-clt-diagnostics".to_string(),
            "pasted OCR output".to_string(),
        )
        .expect("event should be returned to Swift");
    assert_eq!(event.seq, 3);
    assert_eq!(event.kind, "bootstrap_output_submitted");
}
