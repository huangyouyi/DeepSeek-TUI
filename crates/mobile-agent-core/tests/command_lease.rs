use deepseek_mobile_agent_core::transport::{RunnerHttpTransport, RunnerTransportConfig};
use deepseek_mobile_agent_core::{
    ApprovalGate, CommandLease, CommandLeaseAction, RemoteToolCall, RemoteToolName, RiskLevel,
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
fn high_risk_shell_and_powershell_approvals_generate_command_leases() {
    let gate = ApprovalGate;

    let shell = gate
        .evaluate(&call(
            "shell-danger",
            RemoteToolName::ShellExec,
            json!({
                "command": "sudo xcode-select --switch /Applications/Xcode.app",
                "cwd": "/Users/dev/project"
            }),
        ))
        .expect("high risk shell command should require approval");
    let shell_lease = shell
        .command_lease("idem-shell-1", Some(4_102_444_800_000))
        .expect("shell approval should produce a command lease");

    assert_eq!(shell.risk.level, RiskLevel::High);
    assert_eq!(shell_lease.id, "lease-approval-shell-danger");
    assert_eq!(shell_lease.idempotency_key, "idem-shell-1");
    assert_eq!(shell_lease.expires_at_unix_ms, Some(4_102_444_800_000));
    assert_eq!(
        shell_lease.approved_action,
        CommandLeaseAction {
            tool: "remote.shell.exec".to_string(),
            command: "sudo xcode-select --switch /Applications/Xcode.app".to_string(),
            cwd: Some("/Users/dev/project".to_string()),
        }
    );

    let powershell = gate
        .evaluate(&call(
            "ps-danger",
            RemoteToolName::PowerShellExec,
            json!({
                "script": "Set-ExecutionPolicy RemoteSigned",
                "working_directory": "C:\\Users\\dev"
            }),
        ))
        .expect("high risk PowerShell command should require approval");
    let powershell_lease = powershell
        .command_lease("idem-ps-1", None)
        .expect("PowerShell approval should produce a command lease");

    assert_eq!(powershell.risk.level, RiskLevel::High);
    assert_eq!(
        powershell_lease.approved_action,
        CommandLeaseAction {
            tool: "remote.powershell.exec".to_string(),
            command: "Set-ExecutionPolicy RemoteSigned".to_string(),
            cwd: Some("C:\\Users\\dev".to_string()),
        }
    );
    assert_eq!(powershell_lease.expires_at_unix_ms, None);
}

#[test]
fn command_lease_metadata_does_not_include_transport_token_or_secret_env_values() {
    let gate = ApprovalGate;
    let approval = gate
        .evaluate(&call(
            "secret-command",
            RemoteToolName::ShellExec,
            json!({
                "command": "sudo launchctl kickstart system/com.example.service",
                "env": {
                    "API_TOKEN": "runner-pairing-secret",
                    "NORMAL": "visible"
                }
            }),
        ))
        .expect("high risk shell command should require approval");

    let lease = approval
        .command_lease("idem-secret-command", Some(4_102_444_800_000))
        .expect("shell approval should produce a command lease");
    let serialized = serde_json::to_string(&lease).expect("lease should serialize");

    assert!(!serialized.contains("runner-pairing-secret"));
    assert!(!serialized.contains("API_TOKEN"));
    assert!(!serialized.contains("NORMAL"));

    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", "bearer-secret");
    let request = transport
        .prepare_tool_call_request(&approval.call.with_command_lease(lease))
        .expect("request should be prepared without network");

    assert!(!format!("{request:?}").contains("bearer-secret"));
    assert!(!request.body.to_string().contains("bearer-secret"));
}

#[test]
fn runner_http_tool_call_request_embeds_lease_in_kai_runner_shape() {
    let transport = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.example/mobile/".to_string(),
        token_present: true,
    });
    let call = call(
        "call-lease",
        RemoteToolName::ShellExec,
        json!({ "command": "sudo xcode-select --switch /Applications/Xcode.app" }),
    )
    .with_command_lease(CommandLease {
        id: "lease-call-lease".to_string(),
        idempotency_key: "idem-call-lease".to_string(),
        approved_action: CommandLeaseAction {
            tool: "remote.shell.exec".to_string(),
            command: "sudo xcode-select --switch /Applications/Xcode.app".to_string(),
            cwd: None,
        },
        expires_at_unix_ms: Some(4_102_444_800_000),
    });

    let request = transport
        .prepare_tool_call_request(&call)
        .expect("request should be prepared without network");

    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://runner.example/mobile/tool-call");
    assert_eq!(
        request.body,
        json!({
            "call_id": "call-lease",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "sudo xcode-select --switch /Applications/Xcode.app",
                "idempotency_key": "idem-call-lease",
                "lease": {
                    "id": "lease-call-lease",
                    "idempotency_key": "idem-call-lease",
                    "approved_action": {
                        "tool": "remote.shell.exec",
                        "command": "sudo xcode-select --switch /Applications/Xcode.app"
                    },
                    "expires_at_unix_ms": 4102444800000u64
                }
            }
        })
    );
}
