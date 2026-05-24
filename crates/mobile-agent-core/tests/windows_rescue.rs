use deepseek_mobile_agent_core::agent_loop::AgentLoop;
use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, TransportError,
};
use deepseek_mobile_agent_core::{
    ApprovalGate, FakeModelClient, MobileEventKind, ModelResponse, RemoteToolCall, RemoteToolName,
    RiskLevel,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

fn powershell_call(call_id: &str, script: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::PowerShellExec,
        arguments: json!({ "script": script }),
    }
}

fn response(text: &str, tool_calls: Vec<RemoteToolCall>) -> ModelResponse {
    ModelResponse {
        text: text.to_string(),
        tool_calls,
    }
}

#[test]
fn windows_rescue_diagnostic_powershell_python_node_winget_path_plan_is_low_risk() {
    let gate = ApprovalGate;

    for call in [
        powershell_call(
            "windows-rescue-powershell-version",
            "$PSVersionTable.PSVersion.ToString()",
        ),
        powershell_call("windows-rescue-python-version", "python --version"),
        powershell_call("windows-rescue-py-launcher", "py --version"),
        powershell_call("windows-rescue-node-version", "node --version"),
        powershell_call("windows-rescue-npm-version", "npm --version"),
        powershell_call("windows-rescue-winget-version", "winget --version"),
        powershell_call(
            "windows-rescue-command-discovery",
            "Get-Command python, py, node, npm, winget, pwsh -ErrorAction SilentlyContinue",
        ),
        powershell_call(
            "windows-rescue-path",
            "[Environment]::GetEnvironmentVariable('Path', 'User')",
        ),
    ] {
        assert_eq!(gate.evaluate(&call), None, "{call:?} should be low risk");
    }
}

#[test]
fn windows_rescue_winget_path_and_uac_repairs_pending_high_risk_approval() {
    let gate = ApprovalGate;

    for call in [
        powershell_call(
            "windows-rescue-install-python",
            "winget install --id Python.Python.3.12 --source winget",
        ),
        powershell_call(
            "windows-rescue-install-node",
            "winget install OpenJS.NodeJS.LTS",
        ),
        powershell_call(
            "windows-rescue-change-user-path",
            "setx PATH \"$env:PATH;C:\\Tools\\DeepSeek\"",
        ),
        powershell_call(
            "windows-rescue-admin-uac",
            "Start-Process PowerShell -Verb RunAs",
        ),
    ] {
        let request = gate
            .evaluate(&call)
            .expect("Windows rescue repair must require approval");

        assert_eq!(request.risk.level, RiskLevel::High, "{call:?}");
        assert_eq!(request.call_id, call.call_id);
        assert_eq!(request.tool_name, "power_shell_exec");
        assert_eq!(request.call, call);
        assert!(request.approval_id.starts_with("approval-windows-rescue-"));
    }
}

#[test]
fn windows_rescue_runner_http_smoke_auto_diagnoses_then_pends_repairs() {
    let pairing_token = "windows-rescue-runner-token";
    let captured_requests: Rc<RefCell<Vec<RunnerHttpRequestSpec>>> =
        Rc::new(RefCell::new(Vec::new()));
    let captured_for_backend = Rc::clone(&captured_requests);
    let mut responses = VecDeque::from([
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "windows-rescue-powershell-version",
            "success": true,
            "result": {
                "stdout": "7.4.6\r\n",
                "stderr": "",
                "exit_code": 0
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "windows-rescue-python-version",
            "success": true,
            "result": {
                "stdout": "Python 3.12.4\r\n",
                "stderr": "",
                "exit_code": 0
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "windows-rescue-node-version",
            "success": true,
            "result": {
                "stdout": "v22.11.0\r\n",
                "stderr": "",
                "exit_code": 0
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "windows-rescue-winget-version",
            "success": true,
            "result": {
                "stdout": "v1.9.25200\r\n",
                "stderr": "",
                "exit_code": 0
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "windows-rescue-path",
            "success": true,
            "result": {
                "stdout": "C:\\Users\\dev\\AppData\\Local\\Programs\\Python\\Python312;C:\\Program Files\\nodejs\r\n",
                "stderr": "",
                "exit_code": 0
            }
        })),
    ]);
    let backend = RunnerHttpClosureBackend::new(move |request: RunnerHttpRequestSpec| {
        captured_for_backend.borrow_mut().push(request);
        responses
            .pop_front()
            .ok_or_else(|| TransportError::Failed("unexpected runner request".to_string()))
    });
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", pairing_token);
    let mut runner = RunnerHttpClient::new(transport, backend);

    let diagnostic_calls = vec![
        powershell_call(
            "windows-rescue-powershell-version",
            "$PSVersionTable.PSVersion.ToString()",
        ),
        powershell_call("windows-rescue-python-version", "python --version"),
        powershell_call("windows-rescue-node-version", "node --version"),
        powershell_call("windows-rescue-winget-version", "winget --version"),
        powershell_call(
            "windows-rescue-path",
            "[Environment]::GetEnvironmentVariable('Path', 'User')",
        ),
    ];
    let repair_calls = vec![
        powershell_call(
            "windows-rescue-install-python",
            "winget install --id Python.Python.3.12 --source winget",
        ),
        powershell_call(
            "windows-rescue-add-python-path",
            "setx PATH \"$env:PATH;C:\\Users\\dev\\AppData\\Local\\Programs\\Python\\Python312\"",
        ),
    ];
    let mut model = FakeModelClient::new(vec![
        response(
            "Windows rescue: running PowerShell, Python, Node, winget, and PATH diagnostics.",
            diagnostic_calls.clone(),
        ),
        response(
            "Windows rescue: installing Python or editing PATH needs approval.",
            repair_calls.clone(),
        ),
    ]);

    let first_turn = AgentLoop::new().run_turn(
        &mut model,
        &mut runner,
        "Windows rescue: diagnose Python, Node, winget, PowerShell, and PATH",
    );

    assert!(first_turn.pending_approvals.is_empty());
    assert_eq!(first_turn.executed_tool_outputs.len(), 5);
    assert_eq!(
        first_turn
            .executed_tool_outputs
            .iter()
            .map(|output| output.call_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "windows-rescue-powershell-version",
            "windows-rescue-python-version",
            "windows-rescue-node-version",
            "windows-rescue-winget-version",
            "windows-rescue-path",
        ]
    );
    assert_eq!(
        first_turn
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            MobileEventKind::ToolCallStarted,
            MobileEventKind::ToolCallCompleted,
            MobileEventKind::ToolCallStarted,
            MobileEventKind::ToolCallCompleted,
            MobileEventKind::ToolCallStarted,
            MobileEventKind::ToolCallCompleted,
            MobileEventKind::ToolCallStarted,
            MobileEventKind::ToolCallCompleted,
            MobileEventKind::ToolCallStarted,
            MobileEventKind::ToolCallCompleted,
        ]
    );

    let second_turn = AgentLoop::new().run_turn(
        &mut model,
        &mut runner,
        "Windows rescue: propose repair for missing Python or PATH",
    );

    assert!(second_turn.executed_tool_outputs.is_empty());
    assert_eq!(second_turn.pending_approvals.len(), 2);
    assert!(
        second_turn
            .pending_approvals
            .iter()
            .all(|request| request.risk.level == RiskLevel::High)
    );
    assert_eq!(
        second_turn
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            MobileEventKind::ApprovalRequired,
            MobileEventKind::ApprovalRequired,
        ]
    );

    let captured = captured_requests.borrow();
    assert_eq!(captured.len(), 5);
    for request in captured.iter() {
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://runner.example/mobile/tool-call");
        assert!(request.token_present);
        assert_eq!(request.auth_scheme(), Some("Bearer"));
        assert_eq!(
            request.authorization_header_value(),
            Some(format!("Bearer {pairing_token}"))
        );
        assert!(!format!("{request:?}").contains(pairing_token));
        assert_eq!(request.body["name"], json!("remote.powershell.exec"));
    }
    assert_eq!(
        captured
            .iter()
            .map(|request| request.body.clone())
            .collect::<Vec<_>>(),
        vec![
            json!({
                "call_id": "windows-rescue-powershell-version",
                "name": "remote.powershell.exec",
                "arguments": { "script": "$PSVersionTable.PSVersion.ToString()" }
            }),
            json!({
                "call_id": "windows-rescue-python-version",
                "name": "remote.powershell.exec",
                "arguments": { "script": "python --version" }
            }),
            json!({
                "call_id": "windows-rescue-node-version",
                "name": "remote.powershell.exec",
                "arguments": { "script": "node --version" }
            }),
            json!({
                "call_id": "windows-rescue-winget-version",
                "name": "remote.powershell.exec",
                "arguments": { "script": "winget --version" }
            }),
            json!({
                "call_id": "windows-rescue-path",
                "name": "remote.powershell.exec",
                "arguments": { "script": "[Environment]::GetEnvironmentVariable('Path', 'User')" }
            }),
        ]
    );
}
