use std::sync::{Arc, Mutex};
use std::time::Duration;

use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName};
use kai_runner::{
    CommandLeaseAction, CommandLeaseEnvelope, KaiRunner, ShellApprovalNonceManager,
    ShellExecutionPolicy, ShellExecutionRequest, ShellExecutionResult, ShellExecutor, ShellKind,
    SystemShellExecutor,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn call(name: RemoteToolName, arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: format!("call-{name}"),
        name,
        arguments,
    }
}

#[derive(Debug, Clone)]
struct FakeShellExecutor {
    requests: Arc<Mutex<Vec<ShellExecutionRequest>>>,
    result: ShellExecutionResult,
}

impl FakeShellExecutor {
    fn new(result: ShellExecutionResult) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            result,
        }
    }

    fn requests(&self) -> Vec<ShellExecutionRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl ShellExecutor for FakeShellExecutor {
    fn execute(&self, request: ShellExecutionRequest) -> ShellExecutionResult {
        self.requests.lock().unwrap().push(request);
        self.result.clone()
    }
}

#[test]
fn default_runner_still_blocks_shell_execution() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo should-not-run" }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.shell.exec"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("blocked"));
}

#[test]
fn shell_enabled_runner_uses_fake_executor_for_success() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let policy = ShellExecutionPolicy::default().with_cwd_roots(["/tmp"]);
    let runner = KaiRunner::with_shell_executor(fake.clone())
        .with_approval_nonce("nonce-123")
        .with_shell_execution_policy(policy);

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "echo hello",
            "cwd": "/tmp/example",
            "timeout_ms": 1500,
            "approval_nonce": "nonce-123"
        }),
    ));

    assert!(output.success);
    assert_eq!(
        fake.requests(),
        vec![ShellExecutionRequest {
            kind: ShellKind::Shell,
            command: "echo hello".to_string(),
            cwd: Some("/tmp/example".to_string()),
            timeout_ms: Some(1500),
            env: Default::default(),
            path: Vec::new(),
            clear_env: true,
            lease: None,
        }]
    );
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.shell.exec",
            "status": "ok",
            "stdout": "hello\n",
            "stderr": "",
            "exit_code": 0,
            "timed_out": false,
            "command": "echo hello",
            "cwd": "/tmp/example"
        })
    );
}

#[test]
fn shell_enabled_runner_carries_bound_command_lease_to_executor_and_result() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    let lease = CommandLeaseEnvelope::new(
        "lease-1",
        "idem-1",
        CommandLeaseAction::new("remote.shell.exec", "echo hello", Some("/tmp/example")),
    );
    approvals.create_lease(lease.clone(), Some(Duration::from_secs(30)));
    let policy = ShellExecutionPolicy::default().with_cwd_roots(["/tmp"]);
    let runner = KaiRunner::with_shell_executor(fake.clone())
        .with_approval_nonce_manager(approvals)
        .with_shell_execution_policy(policy);

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "echo hello",
            "cwd": "/tmp/example",
            "lease": lease
        }),
    ));

    assert!(output.success);
    assert_eq!(
        fake.requests(),
        vec![ShellExecutionRequest {
            kind: ShellKind::Shell,
            command: "echo hello".to_string(),
            cwd: Some("/tmp/example".to_string()),
            timeout_ms: None,
            env: Default::default(),
            path: Vec::new(),
            clear_env: true,
            lease: Some(lease.clone()),
        }]
    );
    assert_eq!(output.result["lease"]["id"], json!("lease-1"));
    assert_eq!(output.result["lease"]["idempotency_key"], json!("idem-1"));
    assert_eq!(
        output.result["lease"]["approved_action"]["command"],
        json!("echo hello")
    );
}

#[test]
fn shell_enabled_runner_rejects_mismatched_command_lease_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    let lease = CommandLeaseEnvelope::new(
        "lease-1",
        "idem-1",
        CommandLeaseAction::new("remote.shell.exec", "echo approved", Some("/tmp/example")),
    );
    approvals.create_lease(lease.clone(), Some(Duration::from_secs(30)));
    let policy = ShellExecutionPolicy::default().with_cwd_roots(["/tmp"]);
    let runner = KaiRunner::with_shell_executor(fake.clone())
        .with_approval_nonce_manager(approvals)
        .with_shell_execution_policy(policy);

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "echo different",
            "cwd": "/tmp/example",
            "lease": lease
        }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
}

#[test]
fn shell_enabled_runner_rejects_replayed_command_lease_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    let lease = CommandLeaseEnvelope::new(
        "lease-1",
        "idem-1",
        CommandLeaseAction::new("remote.shell.exec", "echo hello", None::<String>),
    );
    approvals.create_lease(lease.clone(), Some(Duration::from_secs(30)));
    let runner =
        KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce_manager(approvals);

    let first = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo hello", "lease": lease.clone() }),
    ));
    let second = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo hello", "lease": lease }),
    ));

    assert!(first.success);
    assert!(!second.success);
    assert_eq!(second.result["error"]["code"], json!("approval_replayed"));
    assert_eq!(fake.requests().len(), 1);
}

#[test]
fn shell_enabled_runner_rejects_expired_command_lease_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    let lease = CommandLeaseEnvelope::new(
        "lease-expired",
        "idem-expired",
        CommandLeaseAction::new("remote.shell.exec", "echo hello", None::<String>),
    );
    approvals.create_lease(lease.clone(), Some(Duration::from_secs(30)));
    approvals.expire("lease-expired");
    let runner =
        KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce_manager(approvals);

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo hello", "lease": lease }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["error"]["code"], json!("approval_expired"));
}

#[test]
fn powershell_enabled_runner_carries_bound_command_lease_to_executor_and_result() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello\r\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    let lease = CommandLeaseEnvelope::new(
        "ps-lease-1",
        "ps-idem-1",
        CommandLeaseAction::new(
            "remote.powershell.exec",
            "Write-Output hello",
            None::<String>,
        ),
    );
    approvals.create_lease(lease.clone(), Some(Duration::from_secs(30)));
    let runner =
        KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce_manager(approvals);

    let output = runner.execute(call(
        RemoteToolName::PowerShellExec,
        json!({ "script": "Write-Output hello", "lease": lease }),
    ));

    assert!(output.success);
    assert_eq!(fake.requests().len(), 1);
    assert_eq!(fake.requests()[0].kind, ShellKind::PowerShell);
    assert_eq!(fake.requests()[0].lease.as_ref().unwrap().id, "ps-lease-1");
    assert_eq!(output.result["lease"]["id"], json!("ps-lease-1"));
    assert_eq!(
        output.result["lease"]["idempotency_key"],
        json!("ps-idem-1")
    );
}

#[test]
fn shell_enabled_runner_requires_approval_nonce_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let runner = KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce("nonce-123");

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo should-not-run" }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["tool"], json!("remote.shell.exec"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
}

#[test]
fn shell_enabled_runner_rejects_wrong_approval_nonce_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let runner = KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce("nonce-123");

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "echo should-not-run",
            "approval_nonce": "wrong"
        }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["tool"], json!("remote.shell.exec"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("approval_denied"));
}

#[test]
fn shell_lifecycle_approval_nonce_executes_once_then_rejects_replay() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    approvals.create("nonce-once", None);
    let runner =
        KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce_manager(approvals);

    let first = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo hello", "approval_nonce": "nonce-once" }),
    ));
    let second = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo hello again", "approval_nonce": "nonce-once" }),
    ));

    assert!(first.success);
    assert!(!second.success);
    assert_eq!(second.result["tool"], json!("remote.shell.exec"));
    assert_eq!(second.result["status"], json!("error"));
    assert_eq!(second.result["error"]["code"], json!("approval_replayed"));
    assert_eq!(
        fake.requests(),
        vec![ShellExecutionRequest {
            kind: ShellKind::Shell,
            command: "echo hello".to_string(),
            cwd: None,
            timeout_ms: None,
            env: Default::default(),
            path: Vec::new(),
            clear_env: true,
            lease: None,
        }]
    );
}

#[test]
fn shell_lifecycle_approval_nonce_rejects_expired_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    approvals.create("expired-nonce", Some(Duration::from_secs(30)));
    approvals.expire("expired-nonce");
    let runner =
        KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce_manager(approvals);

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo should-not-run", "approval_nonce": "expired-nonce" }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["tool"], json!("remote.shell.exec"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("approval_expired"));
}

#[test]
fn shell_lifecycle_approval_nonce_rejects_wrong_and_missing_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let approvals = ShellApprovalNonceManager::new();
    approvals.create("nonce-123", None);
    let runner =
        KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce_manager(approvals);

    let wrong = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo should-not-run", "approval_nonce": "wrong" }),
    ));
    let missing = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "echo should-not-run" }),
    ));

    assert!(!wrong.success);
    assert!(!missing.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(wrong.result["error"]["code"], json!("approval_denied"));
    assert_eq!(missing.result["error"]["code"], json!("approval_required"));
}

#[test]
fn shell_enabled_runner_allows_non_shell_tools_without_approval_nonce() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let runner = KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce("nonce-123");

    let output = runner.execute(call(RemoteToolName::DiagnoseSystem, json!({})));

    assert!(output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["tool"], json!("remote.diagnose.system"));
    assert_eq!(output.result["status"], json!("ok"));
}

#[test]
fn shell_enabled_runner_passes_only_allowed_env_and_fixed_path_to_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "ok\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let policy = ShellExecutionPolicy::default()
        .with_allowed_env_keys(["TERM"])
        .with_path_entries(["/usr/bin", "/bin"]);
    let runner = KaiRunner::with_shell_executor(fake.clone())
        .with_approval_nonce("nonce-123")
        .with_shell_execution_policy(policy);

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "printf ok",
            "env": {
                "TERM": "xterm-256color"
            },
            "approval_nonce": "nonce-123"
        }),
    ));

    assert!(output.success);
    assert_eq!(
        fake.requests(),
        vec![ShellExecutionRequest {
            kind: ShellKind::Shell,
            command: "printf ok".to_string(),
            cwd: None,
            timeout_ms: None,
            env: [("TERM".to_string(), "xterm-256color".to_string())].into(),
            path: vec!["/usr/bin".to_string(), "/bin".to_string()],
            clear_env: true,
            lease: None,
        }]
    );
    assert_eq!(
        output.result["policy"],
        json!({
            "clear_env": true,
            "allowed_env": ["TERM"],
            "path": ["/usr/bin", "/bin"]
        })
    );
    assert!(
        !output.result.to_string().contains("xterm-256color"),
        "tool result must not echo request environment values"
    );
}

#[test]
fn shell_enabled_runner_rejects_sensitive_env_without_leaking_value() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let runner = KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce("nonce-123");

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "env",
            "env": {
                "OPENAI_API_KEY": "sk-secret-value"
            },
            "approval_nonce": "nonce-123"
        }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["error"]["code"], json!("env_denied"));
    assert!(!output.result.to_string().contains("sk-secret-value"));
}

#[test]
fn shell_enabled_runner_rejects_cwd_outside_policy_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let policy = ShellExecutionPolicy::default().with_cwd_roots(["/workspace/project"]);
    let runner = KaiRunner::with_shell_executor(fake.clone())
        .with_approval_nonce("nonce-123")
        .with_shell_execution_policy(policy);

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "pwd",
            "cwd": "/etc",
            "approval_nonce": "nonce-123"
        }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["error"]["code"], json!("cwd_denied"));
}

#[test]
fn shell_enabled_runner_rejects_request_supplied_path_before_executor() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let runner = KaiRunner::with_shell_executor(fake.clone()).with_approval_nonce("nonce-123");

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({
            "command": "echo path",
            "path": ["/tmp/bin"],
            "approval_nonce": "nonce-123"
        }),
    ));

    assert!(!output.success);
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
    assert_eq!(output.result["error"]["code"], json!("path_denied"));
}

#[test]
fn shell_enabled_runner_returns_structured_nonzero_result() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: String::new(),
        stderr: "nope\n".to_string(),
        exit_code: Some(2),
        timed_out: false,
    });
    let runner = KaiRunner::with_shell_executor(fake).with_approval_nonce("nonce-123");

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "exit 2", "approval_nonce": "nonce-123" }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.shell.exec"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["stdout"], json!(""));
    assert_eq!(output.result["stderr"], json!("nope\n"));
    assert_eq!(output.result["exit_code"], json!(2));
    assert_eq!(output.result["timed_out"], json!(false));
    assert_eq!(output.result["command"], json!("exit 2"));
    assert!(output.result.get("cwd").is_none());
}

#[test]
fn shell_enabled_runner_returns_structured_timeout_result() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "partial\n".to_string(),
        stderr: "timed out\n".to_string(),
        exit_code: None,
        timed_out: true,
    });
    let runner = KaiRunner::with_shell_executor(fake).with_approval_nonce("nonce-123");

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "sleep 60", "timeout_ms": 5, "approval_nonce": "nonce-123" }),
    ));

    assert!(!output.success);
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.shell.exec",
            "status": "error",
            "stdout": "partial\n",
            "stderr": "timed out\n",
            "exit_code": null,
            "timed_out": true,
            "command": "sleep 60"
        })
    );
}

#[test]
fn system_shell_executor_rejects_real_shell_until_explicitly_enabled() {
    let runner = KaiRunner::with_system_shell_executor()
        .with_approval_nonce("nonce-123")
        .with_shell_execution_policy(ShellExecutionPolicy::default().with_clear_env(true));

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "printf ok", "approval_nonce": "nonce-123" }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.shell.exec"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["stdout"], json!(""));
    assert_eq!(output.result["exit_code"], json!(null));
    assert_eq!(output.result["timed_out"], json!(false));
    assert!(
        output.result["stderr"]
            .as_str()
            .unwrap()
            .contains("real shell execution is not enabled"),
        "default system executor must reject without spawning a real shell"
    );
}

#[test]
fn system_shell_executor_runs_harmless_shell_only_when_enabled_for_tests() {
    let runner = KaiRunner::with_shell_executor(SystemShellExecutor::enabled_for_tests())
        .with_approval_nonce("nonce-123")
        .with_shell_execution_policy(
            ShellExecutionPolicy::default().with_path_entries(["/usr/bin", "/bin"]),
        );

    let output = runner.execute(call(
        RemoteToolName::ShellExec,
        json!({ "command": "printf ok", "approval_nonce": "nonce-123" }),
    ));

    assert!(output.success);
    assert_eq!(output.result["tool"], json!("remote.shell.exec"));
    assert_eq!(output.result["status"], json!("ok"));
    assert_eq!(output.result["stdout"], json!("ok"));
    assert_eq!(output.result["stderr"], json!(""));
    assert_eq!(output.result["exit_code"], json!(0));
    assert_eq!(output.result["timed_out"], json!(false));
}

#[test]
fn powershell_default_runner_is_blocked() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::PowerShellExec,
        json!({ "command": "Write-Output hello" }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.powershell.exec"));
    assert_eq!(output.result["error"]["code"], json!("blocked"));
}

#[cfg(not(windows))]
#[test]
fn powershell_without_fake_executor_is_structured_not_available_on_non_windows() {
    let runner = KaiRunner::with_system_shell_executor()
        .with_approval_nonce("nonce-123")
        .with_shell_execution_policy(ShellExecutionPolicy::default().with_clear_env(true));

    let output = runner.execute(call(
        RemoteToolName::PowerShellExec,
        json!({ "command": "Write-Output hello", "approval_nonce": "nonce-123" }),
    ));

    assert!(!output.success);
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.powershell.exec",
            "status": "error",
            "stdout": "",
            "stderr": "PowerShell execution is not available on this platform",
            "exit_code": null,
            "timed_out": false,
            "command": "Write-Output hello",
            "error": {
                "code": "not_available",
                "message": "PowerShell execution is not available on this platform"
            }
        })
    );
}
