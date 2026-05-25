use std::str::FromStr;

use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName, RemoteToolOutput};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[test]
fn remote_schema_tool_names_match_documented_strings() {
    let cases = [
        (RemoteToolName::ShellExec, "remote.shell.exec"),
        (RemoteToolName::PowerShellExec, "remote.powershell.exec"),
        (RemoteToolName::FileRead, "remote.file.read"),
        (RemoteToolName::FileWrite, "remote.file.write"),
        (RemoteToolName::DiagnoseSystem, "remote.diagnose.system"),
        (RemoteToolName::PackageInstall, "remote.package.install"),
        (RemoteToolName::BrowserOpen, "remote.browser.open"),
        (
            RemoteToolName::BrowserExtractText,
            "remote.browser.extract_text",
        ),
        (RemoteToolName::BrowserClick, "remote.browser.click"),
        (RemoteToolName::BootstrapGuide, "remote.bootstrap.guide"),
        (RemoteToolName::McpCall, "remote.mcp.call"),
    ];

    for (name, documented) in cases {
        assert_eq!(name.as_str(), documented);
        assert_eq!(RemoteToolName::from_str(documented).unwrap(), name);
        assert_eq!(serde_json::to_value(name).unwrap(), json!(documented));
        assert_eq!(
            serde_json::from_value::<RemoteToolName>(json!(documented)).unwrap(),
            name
        );
    }
}

#[test]
fn remote_schema_tool_call_uses_documented_name_in_json() {
    let call = RemoteToolCall {
        call_id: "call-1".to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": "pwd" }),
    };

    let encoded = serde_json::to_value(&call).unwrap();

    assert_eq!(
        encoded,
        json!({
            "call_id": "call-1",
            "name": "remote.shell.exec",
            "arguments": { "command": "pwd" }
        })
    );

    let decoded: RemoteToolCall = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, call);
}

#[test]
fn remote_schema_tool_output_round_trips_required_fields() {
    let output = RemoteToolOutput {
        call_id: "call-1".to_string(),
        success: true,
        result: json!({ "stdout": "/tmp\n", "exit_code": 0 }),
    };

    let encoded = serde_json::to_value(&output).unwrap();

    assert_eq!(
        encoded,
        json!({
            "call_id": "call-1",
            "success": true,
            "result": { "stdout": "/tmp\n", "exit_code": 0 }
        })
    );

    let decoded: RemoteToolOutput = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, output);
}

#[test]
fn remote_schema_unknown_tool_name_reports_name() {
    let err = RemoteToolName::from_str("remote.file.delete").unwrap_err();

    assert_eq!(
        err.to_string(),
        "unknown remote tool name: remote.file.delete"
    );

    let serde_err = serde_json::from_value::<RemoteToolName>(json!("remote.file.delete"))
        .expect_err("unknown names must fail deserialization");
    assert!(
        serde_err
            .to_string()
            .contains("unknown remote tool name: remote.file.delete")
    );
}

#[test]
fn remote_schema_tool_call_arguments_accept_any_json_value() {
    let raw = json!({
        "call_id": "call-2",
        "name": "remote.mcp.call",
        "arguments": {
            "server": "filesystem",
            "method": "read",
            "params": ["README.md", { "limit": 100 }]
        }
    });

    let call: RemoteToolCall = serde_json::from_value(raw).unwrap();

    assert_eq!(call.call_id, "call-2");
    assert_eq!(call.name, RemoteToolName::McpCall);
    assert_eq!(
        call.arguments,
        json!({
            "server": "filesystem",
            "method": "read",
            "params": ["README.md", { "limit": 100 }]
        })
    );

    let _: Value = call.arguments;
}
