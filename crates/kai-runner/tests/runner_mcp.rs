use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName};
use kai_runner::{KaiRunner, McpToolHandler, McpToolRequest, RunnerToolError};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn mcp_call(arguments: Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: "mcp-call-1".to_string(),
        name: RemoteToolName::McpCall,
        arguments,
    }
}

#[derive(Debug, Clone)]
struct FakeMcpHandler;

impl McpToolHandler for FakeMcpHandler {
    fn call(&self, request: McpToolRequest) -> Result<Value, RunnerToolError> {
        Ok(json!({
            "server": request.server,
            "tool": request.tool,
            "echo": request.arguments,
        }))
    }
}

#[test]
fn default_runner_keeps_mcp_call_unsupported() {
    let runner = KaiRunner;

    let output = runner.execute(mcp_call(json!({
        "server": "local",
        "tool": "echo",
        "arguments": { "text": "hello" }
    })));

    assert_eq!(output.call_id, "mcp-call-1");
    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.mcp.call"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("unsupported"));
}

#[test]
fn registered_mcp_call_succeeds_and_preserves_call_id() {
    let runner = KaiRunner::with_mcp_handler("local", "echo", FakeMcpHandler);

    let output = runner.execute(mcp_call(json!({
        "server": "local",
        "tool": "echo",
        "arguments": { "text": "hello" }
    })));

    assert_eq!(output.call_id, "mcp-call-1");
    assert!(output.success);
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.mcp.call",
            "status": "ok",
            "data": {
                "server": "local",
                "tool": "echo",
                "echo": { "text": "hello" }
            }
        })
    );
}

#[test]
fn unknown_mcp_server_or_tool_is_rejected() {
    let runner = KaiRunner::with_mcp_handler("local", "echo", FakeMcpHandler);

    for arguments in [
        json!({ "server": "missing", "tool": "echo", "arguments": {} }),
        json!({ "server": "local", "tool": "missing", "arguments": {} }),
    ] {
        let output = runner.execute(mcp_call(arguments));

        assert_eq!(output.call_id, "mcp-call-1");
        assert!(!output.success);
        assert_eq!(output.result["tool"], json!("remote.mcp.call"));
        assert_eq!(output.result["status"], json!("error"));
        assert_eq!(output.result["error"]["code"], json!("unknown_mcp_tool"));
    }
}

#[test]
fn malformed_mcp_arguments_are_invalid_arguments() {
    let runner = KaiRunner::with_mcp_handler("local", "echo", FakeMcpHandler);

    for arguments in [
        json!({ "tool": "echo", "arguments": {} }),
        json!({ "server": "local", "arguments": {} }),
        json!({ "server": "", "tool": "echo", "arguments": {} }),
        json!({ "server": "local", "tool": "", "arguments": {} }),
        json!({ "server": "local", "tool": "echo", "arguments": [] }),
    ] {
        let output = runner.execute(mcp_call(arguments));

        assert_eq!(output.call_id, "mcp-call-1");
        assert!(!output.success);
        assert_eq!(output.result["tool"], json!("remote.mcp.call"));
        assert_eq!(output.result["status"], json!("error"));
        assert_eq!(output.result["error"]["code"], json!("invalid_arguments"));
    }
}
