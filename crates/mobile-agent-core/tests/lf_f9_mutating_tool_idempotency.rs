use deepseek_mobile_agent_core::transport::{
    RunnerHttpTransport, RunnerMaintenanceApproval, RunnerMaintenancePlanRequest,
};
use deepseek_mobile_agent_core::{
    CommandLease, CommandLeaseAction, RemoteToolCall, RemoteToolName,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn call(call_id: &str, name: RemoteToolName, arguments: Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name,
        arguments,
    }
}

#[test]
fn lf_f9_mobile_core_mutating_tool_requests_carry_replay_safe_metadata_without_network() {
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", "pairing-secret");

    let shell = transport
        .prepare_tool_call_request(
            &call(
                "lf-f9-shell",
                RemoteToolName::ShellExec,
                json!({ "command": "printf ok" }),
            )
            .with_command_lease(CommandLease::new(
                "lease-lf-f9-shell",
                "idem-lf-f9-shell",
                CommandLeaseAction::new("remote.shell.exec", "printf ok", None::<String>),
            )),
        )
        .expect("request should be prepared without network");
    assert_eq!(
        shell.body["arguments"]["lease"]["id"],
        json!("lease-lf-f9-shell")
    );
    assert_eq!(
        shell.body["arguments"]["idempotency_key"],
        json!("idem-lf-f9-shell")
    );

    let file_write = transport
        .prepare_tool_call_request(&call(
            "lf-f9-file-write",
            RemoteToolName::FileWrite,
            json!({
                "path": "notes.txt",
                "content": "hello\n",
                "idempotency_key": "idem-lf-f9-file-write"
            }),
        ))
        .expect("request should be prepared without network");
    assert_eq!(
        file_write.body["arguments"]["idempotency_key"],
        json!("idem-lf-f9-file-write")
    );

    let package_install = transport
        .prepare_tool_call_request(&call(
            "lf-f9-package",
            RemoteToolName::PackageInstall,
            json!({
                "manager": "apt",
                "package": "ripgrep",
                "idempotency_key": "idem-lf-f9-package"
            }),
        ))
        .expect("request should be prepared without network");
    assert_eq!(
        package_install.body["arguments"]["idempotency_key"],
        json!("idem-lf-f9-package")
    );

    let browser_click = transport
        .prepare_tool_call_request(&call(
            "lf-f9-browser-click",
            RemoteToolName::BrowserClick,
            json!({
                "session_id": "browser-session-1",
                "selector": "button[type=submit]",
                "idempotency_key": "idem-lf-f9-browser-click",
                "approval_nonce": "nonce-lf-f9-browser-click"
            }),
        ))
        .expect("request should be prepared without network");
    assert_eq!(
        browser_click.body["arguments"]["idempotency_key"],
        json!("idem-lf-f9-browser-click")
    );
    assert_eq!(
        browser_click.body["arguments"]["approval_nonce"],
        json!("nonce-lf-f9-browser-click")
    );
}

#[test]
fn lf_f9_mobile_core_maintenance_plan_requests_bind_idempotency_and_nonce_without_network() {
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", "pairing-secret");

    for (action, idempotency_key, nonce) in [
        (
            "self_update",
            "idem-lf-f9-maintenance-self-update",
            "nonce-lf-f9-maintenance-self-update",
        ),
        (
            "uninstall",
            "idem-lf-f9-maintenance-uninstall",
            "nonce-lf-f9-maintenance-uninstall",
        ),
    ] {
        let request = transport
            .prepare_maintenance_plan_request_with_options(
                RunnerMaintenancePlanRequest::new(action)
                    .with_idempotency_key(idempotency_key)
                    .with_approval(RunnerMaintenanceApproval::new().with_nonce(nonce)),
            )
            .expect("maintenance request should be prepared without network");

        assert_eq!(
            request.url,
            "https://runner.example/mobile/maintenance/plan"
        );
        assert_eq!(request.body["action"], json!(action));
        assert_eq!(request.body["idempotency_key"], json!(idempotency_key));
        assert_eq!(request.body["approval"]["nonce"], json!(nonce));
    }
}
