use deepseek_mobile_agent_core::{ExecutionMode, RemoteToolCall, RemoteToolName};
use kai_runner::KaiRunner;
use pretty_assertions::assert_eq;
use serde_json::json;

fn call(name: RemoteToolName) -> RemoteToolCall {
    RemoteToolCall {
        call_id: format!("call-{name}"),
        name,
        arguments: json!({ "command": "echo should-not-run", "path": "/tmp/example" }),
    }
}

fn call_with_arguments(name: RemoteToolName, arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: format!("call-{name}"),
        name,
        arguments,
    }
}

#[test]
fn capabilities_report_runner_mode_with_documented_tool_names() {
    let runner = KaiRunner;
    let capabilities = runner.capabilities();

    assert_eq!(capabilities.tools.mode, ExecutionMode::Runner);

    let encoded = serde_json::to_value(&capabilities).unwrap();
    assert_eq!(
        encoded,
        json!({
            "mode": "runner",
            "tools": [
                "remote.shell.exec",
                "remote.powershell.exec",
                "remote.file.read",
                "remote.file.write",
                "remote.diagnose.system",
                "remote.package.install",
                "remote.browser.open",
                "remote.browser.extract_text",
                "remote.browser.click",
                "remote.bootstrap.guide",
                "remote.mcp.call"
            ]
        })
    );
}

#[test]
fn diagnose_system_returns_successful_system_and_capability_report() {
    let runner = KaiRunner;
    let output = runner.execute(call(RemoteToolName::DiagnoseSystem));

    assert_eq!(output.call_id, "call-remote.diagnose.system");
    assert!(output.success);
    assert_eq!(output.result["tool"], json!("remote.diagnose.system"));
    assert_eq!(output.result["system"]["os"], json!(std::env::consts::OS));
    assert_eq!(
        output.result["system"]["arch"],
        json!(std::env::consts::ARCH)
    );
    assert_eq!(output.result["capabilities"]["mode"], json!("runner"));
    assert_eq!(
        output.result["capabilities"]["tools"][0],
        json!("remote.shell.exec")
    );
}

#[test]
fn shell_tools_are_blocked_in_the_scaffold() {
    let runner = KaiRunner;

    for name in [RemoteToolName::ShellExec, RemoteToolName::PowerShellExec] {
        let output = runner.execute(call(name));

        assert_eq!(output.call_id, format!("call-{name}"));
        assert!(!output.success);
        assert_eq!(output.result["tool"], json!(name.as_str()));
        assert_eq!(output.result["error"]["code"], json!("blocked"));
        assert!(
            output.result["error"]["message"]
                .as_str()
                .unwrap()
                .contains("does not execute local shell commands")
        );
    }
}

#[test]
fn unimplemented_runner_tools_return_structured_unsupported_errors() {
    let runner = KaiRunner;

    for name in [
        RemoteToolName::FileRead,
        RemoteToolName::FileWrite,
        RemoteToolName::BootstrapGuide,
        RemoteToolName::McpCall,
    ] {
        let output = runner.execute(call(name));

        assert_eq!(output.call_id, format!("call-{name}"));
        assert!(!output.success);
        assert_eq!(output.result["tool"], json!(name.as_str()));
        assert_eq!(output.result["error"]["code"], json!("unsupported"));
        assert!(
            output.result["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not implemented")
        );
    }
}

#[test]
fn package_install_defaults_to_safe_dry_run_preview() {
    let runner = KaiRunner;

    let output = runner.execute(call_with_arguments(
        RemoteToolName::PackageInstall,
        json!({
            "manager": "brew",
            "package": "git",
            "version": "2.45.0",
            "allow_sudo": false,
            "idempotency_key": "install-git-once"
        }),
    ));

    assert_eq!(output.call_id, "call-remote.package.install");
    assert!(output.success);
    assert_eq!(output.result["tool"], json!("remote.package.install"));
    assert_eq!(output.result["status"], json!("ok"));
    assert_eq!(output.result["data"]["status"], json!("planned"));
    assert_eq!(
        output.result["data"]["commands"],
        json!(["brew install git"])
    );
    assert_eq!(output.result["data"]["requires_sudo"], json!(false));
    assert!(
        output.result["data"]["log"]
            .as_str()
            .unwrap()
            .contains("brew install git")
    );
    assert_eq!(output.result["data"]["next_approval_required"], json!(true));
    assert_eq!(output.result["data"]["manager"], json!("brew"));
    assert_eq!(output.result["data"]["packages"], json!(["git"]));
    assert_eq!(output.result["data"]["operation"], json!("install"));
    assert_eq!(output.result["data"]["dry_run"], json!(true));
    assert_eq!(
        output.result["data"]["idempotency_key"],
        json!("install-git-once")
    );
    assert_eq!(
        output.result["data"]["command_preview"],
        json!("brew install git")
    );
}

#[test]
fn package_install_execute_is_rejected_without_running_command() {
    let runner = KaiRunner;

    let output = runner.execute(call_with_arguments(
        RemoteToolName::PackageInstall,
        json!({
            "manager": "apt",
            "packages": ["ripgrep", "fd-find"],
            "dry_run": false
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.package.install"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
    assert!(
        output.result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not execute package installs")
    );
}

#[test]
fn package_install_rejects_missing_manager_or_packages() {
    let runner = KaiRunner;

    for arguments in [
        json!({ "package": "ripgrep" }),
        json!({ "manager": "apt" }),
        json!({ "manager": "apt", "packages": [] }),
        json!({ "manager": "", "package": "ripgrep" }),
        json!({ "manager": "apt", "package": "" }),
    ] {
        let output = runner.execute(call_with_arguments(
            RemoteToolName::PackageInstall,
            arguments,
        ));

        assert!(!output.success);
        assert_eq!(output.result["tool"], json!("remote.package.install"));
        assert_eq!(output.result["status"], json!("error"));
        assert_eq!(output.result["error"]["code"], json!("invalid_arguments"));
    }
}
