use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName};
use kai_runner::{
    BrowserClickApproval, BrowserClickRequest, BrowserEngine, BrowserExtractTextRequest,
    BrowserExtractTextResult, BrowserOpenRequest, BrowserOpenResult, BrowserSessionMetadata,
    BrowserSessionRegistry, CommandLeaseAction, CommandLeaseEnvelope, KaiRunner,
    RunnerMaintenanceAction, RunnerMaintenanceOptions, RunnerToolError, ShellApprovalNonceManager,
    ShellExecutionPolicy, ShellExecutionRequest, ShellExecutionResult, ShellExecutor,
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

fn unique_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "kai-runner-lf-f9-{name}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[derive(Debug, Clone)]
struct FakeShellExecutor {
    requests: Arc<Mutex<Vec<ShellExecutionRequest>>>,
}

impl FakeShellExecutor {
    fn requests(&self) -> Vec<ShellExecutionRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl ShellExecutor for FakeShellExecutor {
    fn execute(&self, request: ShellExecutionRequest) -> ShellExecutionResult {
        self.requests.lock().unwrap().push(request);
        ShellExecutionResult {
            stdout: "ok\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            timed_out: false,
        }
    }
}

#[derive(Debug, Default)]
struct FakeBrowserEngine {
    click_requests: Mutex<Vec<BrowserClickRequest>>,
}

impl BrowserEngine for FakeBrowserEngine {
    fn open(&self, request: BrowserOpenRequest) -> Result<BrowserOpenResult, RunnerToolError> {
        Ok(BrowserOpenResult::planned(
            request.url,
            request.profile,
            "planned",
        ))
    }

    fn extract_text(
        &self,
        _request: BrowserExtractTextRequest,
    ) -> Result<BrowserExtractTextResult, RunnerToolError> {
        Ok(BrowserExtractTextResult {
            text: "text".to_string(),
        })
    }

    fn click(&self, request: BrowserClickRequest) -> Result<BrowserClickApproval, RunnerToolError> {
        self.click_requests.lock().unwrap().push(request.clone());
        Ok(BrowserClickApproval::approval_required(request))
    }
}

#[test]
fn lf_f9_runner_shell_exec_binds_command_lease_and_idempotency_key_without_real_shell() {
    let fake = FakeShellExecutor {
        requests: Arc::new(Mutex::new(Vec::new())),
    };
    let approvals = ShellApprovalNonceManager::new();
    let lease = CommandLeaseEnvelope::new(
        "lease-lf-f9-shell",
        "idem-lf-f9-shell",
        CommandLeaseAction::new("remote.shell.exec", "printf ok", Some("/tmp")),
    );
    approvals.create_lease(lease.clone(), Some(Duration::from_secs(30)));

    let runner = KaiRunner::with_shell_executor(fake.clone())
        .with_approval_nonce_manager(approvals)
        .with_shell_execution_policy(ShellExecutionPolicy::default().with_cwd_roots(["/tmp"]));

    let output = runner.execute(call(
        "lf-f9-shell",
        RemoteToolName::ShellExec,
        json!({
            "command": "printf ok",
            "cwd": "/tmp",
            "idempotency_key": "idem-lf-f9-shell",
            "lease": lease
        }),
    ));

    assert!(output.success);
    assert_eq!(
        fake.requests()[0].lease.as_ref().unwrap().id,
        "lease-lf-f9-shell"
    );
    assert_eq!(
        output.result["lease"]["idempotency_key"],
        json!("idem-lf-f9-shell")
    );
}

#[test]
fn lf_f9_runner_file_write_echoes_idempotency_key_without_external_side_effects() {
    let workspace = unique_workspace("file-write");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(call(
        "lf-f9-file-write",
        RemoteToolName::FileWrite,
        json!({
            "path": "notes.txt",
            "content": "hello\n",
            "idempotency_key": "idem-lf-f9-file-write"
        }),
    ));

    assert!(output.success);
    assert_eq!(
        fs::read_to_string(workspace.join("notes.txt")).unwrap(),
        "hello\n"
    );
    assert_eq!(
        output.result["data"]["idempotency_key"],
        json!("idem-lf-f9-file-write")
    );
}

#[test]
fn lf_f9_runner_package_install_plan_echoes_idempotency_key_without_installing() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        "lf-f9-package",
        RemoteToolName::PackageInstall,
        json!({
            "manager": "apt",
            "package": "ripgrep",
            "idempotency_key": "idem-lf-f9-package"
        }),
    ));

    assert!(output.success);
    assert_eq!(output.result["data"]["dry_run"], json!(true));
    assert_eq!(
        output.result["data"]["idempotency_key"],
        json!("idem-lf-f9-package")
    );
}

#[test]
fn lf_f9_runner_browser_click_approval_carries_nonce_and_idempotency_without_real_browser() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.test/form".to_string()),
        profile: Some("test".to_string()),
    });
    let engine = Arc::new(FakeBrowserEngine::default());
    let runner = KaiRunner::with_browser_engine(registry, engine.clone());

    let output = runner.execute(call(
        "lf-f9-browser-click",
        RemoteToolName::BrowserClick,
        json!({
            "session_id": session.id,
            "selector": "button[type=submit]",
            "action": "submit",
            "idempotency_key": "idem-lf-f9-browser-click",
            "approval_nonce": "nonce-lf-f9-browser-click"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
    assert_eq!(
        engine.click_requests.lock().unwrap()[0]
            .idempotency_key
            .as_deref(),
        Some("idem-lf-f9-browser-click")
    );
    assert_eq!(
        engine.click_requests.lock().unwrap()[0]
            .approval_nonce
            .as_deref(),
        Some("nonce-lf-f9-browser-click")
    );
    assert_eq!(
        output.result["data"]["idempotency_key"],
        json!("idem-lf-f9-browser-click")
    );
    assert_eq!(
        output.result["data"]["approval"]["nonce"]["label"],
        json!("approval_nonce")
    );
    assert_eq!(
        output.result["data"]["approval"]["nonce"]["status"],
        json!("provided")
    );
}

#[test]
fn lf_f9_runner_maintenance_plans_self_update_and_uninstall_echo_idempotency_without_execution() {
    for (action, idempotency_key) in [
        (
            RunnerMaintenanceAction::SelfUpdate,
            "idem-lf-f9-maintenance-self-update",
        ),
        (
            RunnerMaintenanceAction::Uninstall,
            "idem-lf-f9-maintenance-uninstall",
        ),
    ] {
        let plan = KaiRunner::plan_maintenance(
            action,
            RunnerMaintenanceOptions {
                idempotency_key: Some(idempotency_key.to_string()),
                ..RunnerMaintenanceOptions::default()
            },
        )
        .expect("maintenance planning should not execute real commands");

        assert!(plan.dry_run);
        assert_eq!(plan.idempotency_key.as_deref(), Some(idempotency_key));
    }
}
