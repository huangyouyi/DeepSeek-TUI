use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName};
use kai_runner::{CommandLeaseAction, CommandLeaseEnvelope, KaiRunner};
use pretty_assertions::assert_eq;
use serde_json::json;

fn call(name: RemoteToolName, arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: format!("call-{name}"),
        name,
        arguments,
    }
}

#[test]
fn powershell_scaffold_plans_script_without_executing() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::PowerShellExec,
        json!({
            "script": "Get-Command winget; $PSVersionTable.PSVersion",
            "working_directory": "C:\\Users\\Alice",
            "timeout_ms": 60000,
            "execution_policy": "bypass_not_allowed",
            "idempotency_key": "diag-winget-1",
            "risk_hint": "low"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.powershell.exec"));
    assert_eq!(output.result["status"], json!("planned"));
    assert_eq!(output.result["blocked"], json!(true));
    assert_eq!(
        output.result["script"],
        json!("Get-Command winget; $PSVersionTable.PSVersion")
    );
    assert_eq!(
        output.result["working_directory"],
        json!("C:\\Users\\Alice")
    );
    assert_eq!(output.result["idempotency_key"], json!("diag-winget-1"));
    assert_eq!(output.result["risk"], json!("low"));
    assert_eq!(output.result["requires_approval"], json!(false));
    assert_eq!(output.result["would_execute"], json!(false));
    assert_eq!(output.result["error"]["code"], json!("blocked"));
}

#[test]
fn powershell_scaffold_requires_approval_for_high_risk_repairs() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::PowerShellExec,
        json!({
            "script": "Set-ExecutionPolicy RemoteSigned -Scope LocalMachine; Start-Process powershell -Verb RunAs",
            "idempotency_key": "fix-execution-policy-1"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.powershell.exec"));
    assert_eq!(output.result["status"], json!("planned"));
    assert_eq!(output.result["blocked"], json!(true));
    assert_eq!(output.result["risk"], json!("high"));
    assert_eq!(output.result["requires_approval"], json!(true));
    assert_eq!(output.result["approval"]["required"], json!(true));
    assert_eq!(output.result["approval"]["risk"], json!("high"));
    assert_eq!(
        output.result["approval"]["reasons"],
        json!(["uac_elevation", "execution_policy_change"])
    );
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
}

#[test]
fn powershell_scaffold_carries_matching_command_lease_without_executing() {
    let runner = KaiRunner;
    let lease = CommandLeaseEnvelope::new(
        "ps-lease-1",
        "ps-idem-1",
        CommandLeaseAction::new(
            "remote.powershell.exec",
            "Get-Command winget",
            Some("C:\\Users\\Alice"),
        ),
    );

    let output = runner.execute(call(
        RemoteToolName::PowerShellExec,
        json!({
            "script": "Get-Command winget",
            "working_directory": "C:\\Users\\Alice",
            "lease": lease
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["status"], json!("planned"));
    assert_eq!(output.result["would_execute"], json!(false));
    assert_eq!(output.result["lease"]["id"], json!("ps-lease-1"));
    assert_eq!(
        output.result["lease"]["idempotency_key"],
        json!("ps-idem-1")
    );
    assert_eq!(
        output.result["lease"]["approved_action"]["tool"],
        json!("remote.powershell.exec")
    );
}

#[test]
fn powershell_scaffold_marks_mismatched_command_lease_approval_required() {
    let runner = KaiRunner;
    let lease = CommandLeaseEnvelope::new(
        "ps-lease-1",
        "ps-idem-1",
        CommandLeaseAction::new(
            "remote.powershell.exec",
            "Set-ExecutionPolicy RemoteSigned",
            Some("C:\\Users\\Alice"),
        ),
    );

    let output = runner.execute(call(
        RemoteToolName::PowerShellExec,
        json!({
            "script": "Get-Command winget",
            "working_directory": "C:\\Users\\Alice",
            "lease": lease
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["status"], json!("planned"));
    assert_eq!(output.result["would_execute"], json!(false));
    assert_eq!(output.result["requires_approval"], json!(true));
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
    assert_eq!(
        output.result["approval"]["reasons"],
        json!(["lease_action_mismatch"])
    );
}

#[test]
fn windows_rescue_diagnose_returns_structured_plan_for_common_install_failures() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::DiagnoseSystem,
        json!({
            "profile": "windows_rescue",
            "include_package_managers": true,
            "include_permissions": true
        }),
    ));

    assert!(output.success);
    assert_eq!(output.result["tool"], json!("remote.diagnose.system"));
    assert_eq!(output.result["status"], json!("ok"));
    assert_eq!(
        output.result["diagnostic_plan"]["profile"],
        json!("windows_rescue")
    );

    let checks = output.result["diagnostic_plan"]["checks"]
        .as_array()
        .unwrap();
    let codes: Vec<&str> = checks
        .iter()
        .map(|check| check["code"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes,
        vec![
            "path_environment",
            "winget_health",
            "event_log_installer_errors",
            "uac_elevation_state",
            "python_install_failure",
            "node_install_failure",
            "vscode_install_failure",
        ]
    );

    for check in checks {
        assert_eq!(check["tool"], json!("remote.powershell.exec"));
        assert_eq!(check["would_execute"], json!(false));
        assert_eq!(check["status"], json!("planned"));
    }

    assert_eq!(checks[0]["risk"], json!("low"));
    assert_eq!(checks[3]["requires_approval_for_fix"], json!(true));
    assert_eq!(checks[3]["risk_for_fix"], json!("high"));
}
