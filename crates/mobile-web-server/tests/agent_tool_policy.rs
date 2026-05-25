use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName};
use deepseek_mobile_web_server::agent_tool_policy::{
    AgentToolDecision, AgentToolPolicy, ShellCommand,
};
use serde_json::json;

fn shell_call(command: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: "call-1".to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": command }),
    }
}

#[test]
fn agent_tool_policy_allows_known_read_only_diagnostic_commands() {
    let decision = AgentToolPolicy.classify(&shell_call("uname -a"));

    assert_eq!(
        decision,
        AgentToolDecision::RunLowRisk(ShellCommand {
            command: "uname -a".to_string(),
            cwd: None,
        })
    );
}

#[test]
fn agent_tool_policy_requires_approval_for_arbitrary_shell_commands() {
    let decision = AgentToolPolicy.classify(&shell_call("opkg update"));

    assert_eq!(
        decision,
        AgentToolDecision::RequireApproval(ShellCommand {
            command: "opkg update".to_string(),
            cwd: None,
        })
    );
}

#[test]
fn agent_tool_policy_preserves_optional_cwd_for_approval() {
    let decision = AgentToolPolicy.classify(&RemoteToolCall {
        call_id: "call-1".to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": "cat config", "cwd": "/etc" }),
    });

    assert_eq!(
        decision,
        AgentToolDecision::RequireApproval(ShellCommand {
            command: "cat config".to_string(),
            cwd: Some("/etc".to_string()),
        })
    );
}

#[test]
fn agent_tool_policy_rejects_unknown_or_malformed_tool_calls_without_execution() {
    let policy = AgentToolPolicy;

    assert!(matches!(
        policy.classify(&RemoteToolCall {
            call_id: "call-1".to_string(),
            name: RemoteToolName::FileRead,
            arguments: json!({ "path": "/etc/passwd" }),
        }),
        AgentToolDecision::Reject { reason } if reason.contains("unsupported")
    ));
    assert!(matches!(
        policy.classify(&RemoteToolCall {
            call_id: "call-2".to_string(),
            name: RemoteToolName::ShellExec,
            arguments: json!({ "cmd": "uname -a" }),
        }),
        AgentToolDecision::Reject { reason } if reason.contains("command")
    ));
}
