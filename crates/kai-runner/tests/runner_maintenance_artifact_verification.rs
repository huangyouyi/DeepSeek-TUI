use kai_runner::{KaiRunner, RunnerMaintenanceAction, RunnerMaintenanceOptions};

fn self_update_plan() -> kai_runner::RunnerMaintenancePlan {
    KaiRunner::plan_maintenance(
        RunnerMaintenanceAction::SelfUpdate,
        RunnerMaintenanceOptions::default(),
    )
    .unwrap()
}

#[test]
fn self_update_includes_macos_artifact_verification_plan() {
    let plan = self_update_plan();

    let artifact = plan
        .artifact_verification
        .iter()
        .find(|artifact| artifact.artifact_name == "kai-runner-aarch64-apple-darwin.tar.gz")
        .expect("macOS kai-runner artifact verification plan");

    assert_eq!(artifact.platform, "macos");
    assert_eq!(
        artifact.expected_checksum_source,
        "release manifest sha256 entry for kai-runner-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        artifact.expected_signature_source,
        "release manifest signature or detached .sig published beside the artifact"
    );
    assert_eq!(
        artifact.verify_step,
        "verify sha256 checksum, then verify signature before staging kai-runner-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        artifact.verify_command,
        "shasum -a 256 -c SHA256SUMS && cosign verify-blob --signature kai-runner-aarch64-apple-darwin.tar.gz.sig kai-runner-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        artifact.failure_rollback_guidance,
        "abort self-update, keep active binaries unchanged, preserve staged artifact and verification output for inspection"
    );
}

#[test]
fn self_update_includes_windows_artifact_verification_plan() {
    let plan = self_update_plan();

    let artifact = plan
        .artifact_verification
        .iter()
        .find(|artifact| artifact.artifact_name == "kai-runner-x86_64-pc-windows-msvc.zip")
        .expect("Windows kai-runner artifact verification plan");

    assert_eq!(artifact.platform, "windows");
    assert_eq!(
        artifact.expected_checksum_source,
        "release manifest sha256 entry for kai-runner-x86_64-pc-windows-msvc.zip"
    );
    assert_eq!(
        artifact.expected_signature_source,
        "release manifest signature or detached .sig published beside the artifact"
    );
    assert_eq!(
        artifact.verify_step,
        "verify sha256 checksum, then verify signature before staging kai-runner-x86_64-pc-windows-msvc.zip"
    );
    assert_eq!(
        artifact.verify_command,
        "powershell -NoProfile -Command \"Get-FileHash kai-runner-x86_64-pc-windows-msvc.zip -Algorithm SHA256\"; cosign verify-blob --signature kai-runner-x86_64-pc-windows-msvc.zip.sig kai-runner-x86_64-pc-windows-msvc.zip"
    );
    assert_eq!(
        artifact.failure_rollback_guidance,
        "abort self-update, keep active binaries unchanged, preserve staged artifact and verification output for inspection"
    );
}

#[test]
fn artifact_verification_plan_redacts_secrets_from_serialized_output() {
    let plan = KaiRunner::plan_maintenance(
        RunnerMaintenanceAction::SelfUpdate,
        RunnerMaintenanceOptions {
            idempotency_key: Some("safe-key".to_string()),
            secret: Some("github_pat_runner_secret_123".to_string()),
            ..RunnerMaintenanceOptions::default()
        },
    )
    .unwrap();

    let encoded = serde_json::to_string(&plan).unwrap();

    assert!(!encoded.contains("github_pat_runner_secret_123"));
    assert!(!encoded.contains("Authorization"));
    assert!(!encoded.contains("Bearer"));
    assert!(!encoded.contains("token"));
    assert!(!encoded.contains("secret"));
}
