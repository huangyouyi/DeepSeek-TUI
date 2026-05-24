use kai_runner::{
    KaiRunner, RunnerMaintenanceAction, RunnerMaintenanceOptions, RunnerMaintenanceRisk,
};
use pretty_assertions::assert_eq;

#[test]
fn self_update_returns_high_risk_dry_run_plan() {
    let plan = KaiRunner::plan_maintenance(
        RunnerMaintenanceAction::SelfUpdate,
        RunnerMaintenanceOptions {
            idempotency_key: Some("self-update-2026-05-24".to_string()),
            ..RunnerMaintenanceOptions::default()
        },
    )
    .unwrap();

    assert_eq!(plan.action, RunnerMaintenanceAction::SelfUpdate);
    assert_eq!(plan.risk, RunnerMaintenanceRisk::High);
    assert!(plan.requires_approval);
    assert!(plan.dry_run);
    assert_eq!(
        plan.idempotency_key.as_deref(),
        Some("self-update-2026-05-24")
    );
    assert_eq!(
        plan.preflight_steps,
        vec![
            "confirm installed deepseek and deepseek-tui binary paths",
            "record current versions and modification times",
            "resolve the trusted release source without using stored credentials",
            "capture rollback location for the currently installed binaries",
        ]
    );
    assert_eq!(
        plan.maintenance_steps,
        vec![
            "download candidate kai-runner, deepseek, and deepseek-tui artifacts into a temporary staging directory",
            "verify checksums or signatures before any replacement is allowed",
            "stage replacement binaries next to the current install without mutating the active install",
            "replace binaries only after explicit local approval, preserving previous binaries for rollback",
            "run post-update version and health checks before declaring the update complete",
        ]
    );
    assert_eq!(
        plan.rollback_steps,
        vec![
            "restore the preserved previous binaries if verification or post-update checks fail",
            "leave the temporary staging directory available for inspection until cleanup is approved",
        ]
    );
    assert_eq!(
        plan.safety_guidance,
        vec![
            "dry-run only: no download, install, replacement, or deletion is executed by this plan",
            "high risk: requires explicit local approval before any future maintenance executor may mutate binaries",
            "exclude credentials, pairing material, and authorization headers from maintenance output",
        ]
    );
    assert_eq!(
        plan.commands,
        vec![
            "deepseek --version",
            "deepseek-tui --version",
            "download kai-runner release artifact to a temporary staging directory",
            "verify release checksum or signature",
            "replace deepseek and deepseek-tui from verified staging after explicit approval",
            "rollback to preserved binaries if health checks fail",
        ]
    );
}

#[test]
fn uninstall_returns_high_risk_dry_run_plan() {
    let plan = KaiRunner::plan_maintenance(
        RunnerMaintenanceAction::Uninstall,
        RunnerMaintenanceOptions::default(),
    )
    .unwrap();

    assert_eq!(plan.action, RunnerMaintenanceAction::Uninstall);
    assert_eq!(plan.risk, RunnerMaintenanceRisk::High);
    assert!(plan.requires_approval);
    assert!(plan.dry_run);
    assert_eq!(plan.idempotency_key, None);
    assert_eq!(
        plan.preflight_steps,
        vec![
            "confirm installed deepseek and deepseek-tui binary paths",
            "record current versions and modification times",
            "confirm user data and config retention policy before removing binaries",
            "capture reinstall instructions and current binary metadata for rollback",
        ]
    );
    assert_eq!(
        plan.maintenance_steps,
        vec![
            "stop any active kai-runner or deepseek-tui process after explicit local approval",
            "remove deepseek and deepseek-tui binaries only after confirming the resolved install paths",
            "cleanup runner launch agents, shims, or temporary installer files without touching user data",
            "check that removed binaries are absent and retained config paths still exist",
        ]
    );
    assert_eq!(
        plan.rollback_steps,
        vec![
            "reinstall the previously recorded deepseek and deepseek-tui versions if removal was unintended",
            "restore launch metadata from the recorded preflight state when available",
        ]
    );
    assert_eq!(
        plan.safety_guidance,
        vec![
            "dry-run only: no process stop, file removal, cleanup, or uninstall is executed by this plan",
            "high risk: requires explicit local approval before any future maintenance executor may remove files",
            "exclude credentials, pairing material, and authorization headers from maintenance output",
        ]
    );
    assert_eq!(
        plan.commands,
        vec![
            "deepseek --version",
            "deepseek-tui --version",
            "stop active kai-runner or deepseek-tui process after explicit approval",
            "remove resolved deepseek and deepseek-tui binaries after explicit approval",
            "cleanup runner shims and temporary installer files",
            "check resolved binary paths are absent while user data remains",
        ]
    );
}

#[test]
fn execute_request_is_rejected_without_maintenance_commands() {
    let error = KaiRunner::plan_maintenance(
        RunnerMaintenanceAction::SelfUpdate,
        RunnerMaintenanceOptions {
            execute: true,
            ..RunnerMaintenanceOptions::default()
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "approval_required");
    assert!(
        error
            .message
            .contains("maintenance actions are not executable")
    );
}

#[test]
fn non_dry_run_request_is_rejected_without_maintenance_commands() {
    let error = KaiRunner::plan_maintenance(
        RunnerMaintenanceAction::Uninstall,
        RunnerMaintenanceOptions {
            dry_run: false,
            ..RunnerMaintenanceOptions::default()
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "not_executable");
    assert!(
        error
            .message
            .contains("maintenance actions are dry-run only")
    );
}

#[test]
fn maintenance_plan_serialization_does_not_log_or_echo_secrets() {
    let plan = KaiRunner::plan_maintenance(
        RunnerMaintenanceAction::SelfUpdate,
        RunnerMaintenanceOptions {
            idempotency_key: Some("safe-key".to_string()),
            secret: Some("super-secret-token".to_string()),
            ..RunnerMaintenanceOptions::default()
        },
    )
    .unwrap();

    let encoded = serde_json::to_string(&plan).unwrap();

    assert!(!encoded.contains("super-secret-token"));
    assert!(!encoded.contains("secret"));
    assert!(!encoded.contains("token"));
    assert!(!encoded.contains("log"));
}
