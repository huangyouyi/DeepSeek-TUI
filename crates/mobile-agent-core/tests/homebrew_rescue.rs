use deepseek_mobile_agent_core::agent_loop::AgentLoop;
use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, RunnerTransportConfig, TransportError,
};
use deepseek_mobile_agent_core::{
    FakeModelClient, MobileAgentCore, MobileEventKind, ModelResponse, RemoteToolCall,
    RemoteToolName,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

fn shell_call(call_id: &str, command: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": command }),
    }
}

fn response(text: &str, tool_calls: Vec<RemoteToolCall>) -> ModelResponse {
    ModelResponse {
        text: text.to_string(),
        tool_calls,
    }
}

#[test]
fn homebrew_rescue_runs_diagnostic_then_requires_approval_for_repair() {
    let user_prompt = "帮我检查 Mac 上 Homebrew 为什么安装失败";
    let diagnostic_call = shell_call("diagnose-brew", "brew doctor");
    let repair_call = shell_call(
        "repair-brew",
        "sudo chown -R $(whoami):admin /opt/homebrew && brew install wget",
    );
    let runner_stdout = r#"Warning: /opt/homebrew is not writable.
You should change the ownership and permissions of /opt/homebrew back to your user account:
  sudo chown -R $(whoami) /opt/homebrew
"#;

    let mut core = MobileAgentCore::new();
    let session_id = core.create_session(user_prompt);
    let session = core.session(&session_id).unwrap();
    assert_eq!(session.title, user_prompt);
    assert_eq!(session.events[0].kind, MobileEventKind::SessionCreated);

    let mut model = FakeModelClient::new(vec![
        response(
            "我会先运行只读的 Homebrew 诊断。",
            vec![diagnostic_call.clone()],
        ),
        response(
            "诊断显示 /opt/homebrew 权限异常；下一步需要你批准修复权限或重新安装包。",
            vec![repair_call.clone()],
        ),
    ]);
    let mut runner = RunnerHttpTransport::new(RunnerTransportConfig {
        endpoint: "https://runner.invalid/api".to_string(),
        token_present: true,
    });
    runner.connect_fake();
    runner.push_response_body(json!({
        "type": "tool_call",
        "call_id": "diagnose-brew",
        "success": true,
        "result": {
            "stdout": runner_stdout,
            "stderr": "",
            "exit_code": 0
        }
    }));

    let first_turn = AgentLoop::new().run_turn(&mut model, &mut runner, user_prompt);

    assert_eq!(model.prompts(), &[user_prompt.to_string()]);
    assert_eq!(runner.requests().len(), 1);
    assert_eq!(
        runner.requests()[0].body,
        json!({
            "call_id": "diagnose-brew",
            "name": "remote.shell.exec",
            "arguments": { "command": "brew doctor" }
        })
    );
    assert_eq!(
        first_turn.assistant_text,
        "我会先运行只读的 Homebrew 诊断。"
    );
    assert!(first_turn.pending_approvals.is_empty());
    assert_eq!(first_turn.executed_tool_outputs.len(), 1);
    assert_eq!(
        first_turn.executed_tool_outputs[0].result["stdout"],
        json!(runner_stdout)
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
        ]
    );

    let diagnostic_stdout = first_turn.executed_tool_outputs[0].result["stdout"]
        .as_str()
        .unwrap();
    let followup_prompt =
        format!("{user_prompt}\n\nRunner diagnostic output:\n{diagnostic_stdout}");

    let second_turn = AgentLoop::new().run_turn(&mut model, &mut runner, &followup_prompt);

    assert_eq!(model.prompts()[1], followup_prompt);
    assert!(model.prompts()[1].contains("Homebrew"));
    assert!(model.prompts()[1].contains("/opt/homebrew is not writable"));
    assert_eq!(
        second_turn.assistant_text,
        "诊断显示 /opt/homebrew 权限异常；下一步需要你批准修复权限或重新安装包。"
    );
    assert!(second_turn.executed_tool_outputs.is_empty());
    assert_eq!(second_turn.pending_approvals.len(), 1);
    assert_eq!(
        second_turn.pending_approvals[0].approval_id,
        "approval-repair-brew"
    );
    assert_eq!(second_turn.pending_approvals[0].call_id, "repair-brew");
    assert_eq!(second_turn.pending_approvals[0].tool_name, "shell_exec");
    assert_eq!(second_turn.pending_approvals[0].call, repair_call);
    assert_eq!(runner.requests().len(), 1);
    assert_eq!(
        second_turn
            .events
            .iter()
            .map(|event| event.kind.clone())
            .collect::<Vec<_>>(),
        vec![MobileEventKind::ApprovalRequired]
    );
    assert_eq!(
        second_turn.events[0].payload["approval_id"],
        json!("approval-repair-brew")
    );
    assert_eq!(
        second_turn.events[0].payload["call_id"],
        json!("repair-brew")
    );
    assert_eq!(
        second_turn.events[0].payload["risk"]["level"],
        json!("High")
    );
}

#[test]
fn homebrew_rescue_on_mac_runner_http_smoke_auto_diagnoses_then_pends_repair_approval() {
    let pairing_token = "mobile-homebrew-runner-token";
    let token = pairing_token.to_string();
    let captured_requests: Rc<RefCell<Vec<RunnerHttpRequestSpec>>> =
        Rc::new(RefCell::new(Vec::new()));
    let captured_for_backend = Rc::clone(&captured_requests);
    let mut responses = VecDeque::from([
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "homebrew-rescue-brew-doctor",
            "success": true,
            "result": {
                "stdout": "Warning: /opt/homebrew is not writable.\nRun brew doctor before making changes.\n",
                "stderr": "",
                "exit_code": 0
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "homebrew-rescue-xcode-select",
            "success": true,
            "result": {
                "stdout": "/Library/Developer/CommandLineTools\n",
                "stderr": "",
                "exit_code": 0
            }
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "tool_call",
            "call_id": "homebrew-rescue-network",
            "success": true,
            "result": {
                "stdout": "curl found at /usr/bin/curl; simulated DNS/TLS check passed without network access.\n",
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
    let transport = RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", token);
    let mut runner = RunnerHttpClient::new(transport, backend);

    let diagnostic_calls = vec![
        shell_call("homebrew-rescue-brew-doctor", "brew doctor"),
        shell_call("homebrew-rescue-xcode-select", "xcode-select -p"),
        shell_call("homebrew-rescue-network", "command -v curl"),
    ];
    let repair_call = shell_call(
        "homebrew-rescue-repair-permissions",
        "sudo chown -R $(whoami):admin /opt/homebrew",
    );
    let mut model = FakeModelClient::new(vec![
        response(
            "Homebrew rescue on Mac: I will run safe diagnostics through the runner.",
            diagnostic_calls.clone(),
        ),
        response(
            "Homebrew rescue on Mac: diagnostics point to Homebrew ownership; repair needs approval.",
            vec![repair_call.clone()],
        ),
    ]);

    let first_turn = AgentLoop::new().run_turn(
        &mut model,
        &mut runner,
        "Homebrew rescue on Mac: diagnose brew install failures",
    );

    assert_eq!(first_turn.pending_approvals, Vec::new());
    assert_eq!(first_turn.executed_tool_outputs.len(), 3);
    assert_eq!(
        first_turn
            .executed_tool_outputs
            .iter()
            .map(|output| output.call_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "homebrew-rescue-brew-doctor",
            "homebrew-rescue-xcode-select",
            "homebrew-rescue-network",
        ]
    );
    assert!(
        first_turn.executed_tool_outputs[0].result["stdout"]
            .as_str()
            .unwrap()
            .contains("/opt/homebrew is not writable")
    );
    assert_eq!(
        first_turn.executed_tool_outputs[1].result["stdout"],
        json!("/Library/Developer/CommandLineTools\n")
    );
    assert!(
        first_turn.executed_tool_outputs[2].result["stdout"]
            .as_str()
            .unwrap()
            .contains("simulated DNS/TLS check passed")
    );

    let second_turn = AgentLoop::new().run_turn(
        &mut model,
        &mut runner,
        "Homebrew rescue on Mac: propose the next repair step",
    );

    assert!(second_turn.executed_tool_outputs.is_empty());
    assert_eq!(second_turn.pending_approvals.len(), 1);
    assert_eq!(
        second_turn.pending_approvals[0].approval_id,
        "approval-homebrew-rescue-repair-permissions"
    );
    assert_eq!(second_turn.pending_approvals[0].call, repair_call);
    assert_eq!(
        second_turn.events[0].payload["risk"]["level"],
        json!("High")
    );

    let captured = captured_requests.borrow();
    assert_eq!(captured.len(), 3);
    for request in captured.iter() {
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://runner.example/mobile/tool-call");
        assert!(request.token_present);
        assert_eq!(request.auth_scheme(), Some("Bearer"));
        assert_eq!(
            request.authorization_header_value(),
            Some(format!("Bearer {pairing_token}"))
        );
    }
    assert_eq!(
        captured
            .iter()
            .map(|request| request.body.clone())
            .collect::<Vec<_>>(),
        vec![
            json!({
                "call_id": "homebrew-rescue-brew-doctor",
                "name": "remote.shell.exec",
                "arguments": { "command": "brew doctor" }
            }),
            json!({
                "call_id": "homebrew-rescue-xcode-select",
                "name": "remote.shell.exec",
                "arguments": { "command": "xcode-select -p" }
            }),
            json!({
                "call_id": "homebrew-rescue-network",
                "name": "remote.shell.exec",
                "arguments": { "command": "command -v curl" }
            }),
        ]
    );
    assert_eq!(
        captured
            .iter()
            .flat_map(|request| request.body["arguments"]["command"].as_str())
            .collect::<Vec<_>>(),
        vec!["brew doctor", "xcode-select -p", "command -v curl"]
    );

    let debug_request = format!("{:?}", captured[0]);
    let debug_runner = format!("{runner:?}");
    assert!(!debug_request.contains(pairing_token));
    assert!(!debug_runner.contains(pairing_token));
    assert!(debug_request.contains("<redacted>"));
    assert!(debug_runner.contains("<redacted>"));
}
