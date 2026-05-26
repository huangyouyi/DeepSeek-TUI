use deepseek_mobile_web_server::types::{
    AgentExecutedTool, AgentTurnRequest, AgentTurnResponse, HealthResponse, Message, MessagePart,
    PendingApproval,
};
use pretty_assertions::assert_eq;
use serde_json::{from_value, json};

#[test]
fn agent_chat_contract_turn_request_uses_stable_json_fields() {
    let request: AgentTurnRequest =
        from_value(json!({ "message": "check disk space" })).expect("request must deserialize");

    assert_eq!(request.message, "check disk space");
    assert_eq!(
        serde_json::to_value(request).expect("request must serialize"),
        json!({ "message": "check disk space" })
    );
}

#[test]
fn agent_chat_contract_turn_response_uses_stable_json_fields() {
    let response = AgentTurnResponse {
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        status: "awaiting_approval".to_string(),
        assistant_text: "I need approval before restarting ssh.".to_string(),
        executed_tools: vec![AgentExecutedTool {
            tool: "remote.shell.exec".to_string(),
            command: "systemctl restart ssh".to_string(),
            requires_approval: true,
            exit_code: None,
            status: "pending_approval".to_string(),
        }],
        pending_approvals: vec![PendingApproval::remote_shell(
            "approval-1".to_string(),
            "session-1".to_string(),
            "systemctl restart ssh".to_string(),
            Some("/".to_string()),
            123,
            "pending".to_string(),
        )],
    };

    assert_eq!(
        serde_json::to_value(response).expect("response must serialize"),
        json!({
            "session_id": "session-1",
            "turn_id": "turn-1",
            "status": "awaiting_approval",
            "assistant_text": "I need approval before restarting ssh.",
            "executed_tools": [{
                "tool": "remote.shell.exec",
                "command": "systemctl restart ssh",
                "requires_approval": true,
                "exit_code": null,
                "status": "pending_approval"
            }],
            "pending_approvals": [{
                "id": "approval-1",
                "session_id": "session-1",
                "command": "systemctl restart ssh",
                "cwd": "/",
                "created_at_ms": 123,
                "status": "pending",
                "risk_level": "high",
                "risk_reason": "advanced command requires explicit approval",
                "target": "remote.shell.exec",
                "target_label": "Remote shell command"
            }]
        })
    );
}

#[test]
fn pending_approval_contract_includes_server_supplied_risk_and_target_fields() {
    let approval: PendingApproval = from_value(json!({
        "id": "approval-1",
        "session_id": "session-1",
        "command": "opkg update",
        "created_at_ms": 123,
        "status": "pending",
        "agent_turn_id": "turn-1"
    }))
    .expect("approval must deserialize with default enrichment");

    assert_eq!(
        serde_json::to_value(approval).expect("approval must serialize"),
        json!({
            "id": "approval-1",
            "session_id": "session-1",
            "command": "opkg update",
            "created_at_ms": 123,
            "status": "pending",
            "agent_turn_id": "turn-1",
            "risk_level": "high",
            "risk_reason": "advanced command requires explicit approval",
            "target": "remote.shell.exec",
            "target_label": "Remote shell command"
        })
    );
}

#[test]
fn message_contract_serializes_compatible_text_and_tool_parts() {
    let message = Message {
        id: "message-1".to_string(),
        session_id: "session-1".to_string(),
        role: "assistant".to_string(),
        created_at_ms: 123,
        parts: vec![
            MessagePart {
                id: "part-text-1".to_string(),
                kind: "text".to_string(),
                text: Some("answer".to_string()),
                data: json!({}),
            },
            MessagePart {
                id: "part-tool-1".to_string(),
                kind: "tool".to_string(),
                text: None,
                data: json!({
                    "turn_id": "turn-1",
                    "agent_turn_id": "turn-1",
                    "tool_call_id": "agent-call-1",
                    "tool": "remote.shell.exec",
                    "title": "Remote shell command",
                    "status": "completed",
                    "requires_approval": false,
                    "command": "uname -a",
                    "input": { "command": "uname -a" },
                    "output": "Linux test-host\n",
                    "stdout": "Linux test-host\n",
                    "stderr": "",
                    "exit_code": 0,
                    "duration_ms": 12,
                    "timed_out": false
                }),
            },
        ],
    };

    assert_eq!(
        serde_json::to_value(message).expect("message must serialize"),
        json!({
            "id": "message-1",
            "session_id": "session-1",
            "role": "assistant",
            "created_at_ms": 123,
            "parts": [
                {
                    "id": "part-text-1",
                    "kind": "text",
                    "text": "answer",
                    "data": {}
                },
                {
                    "id": "part-tool-1",
                    "kind": "tool",
                    "data": {
                        "turn_id": "turn-1",
                        "agent_turn_id": "turn-1",
                        "tool_call_id": "agent-call-1",
                        "tool": "remote.shell.exec",
                        "title": "Remote shell command",
                        "status": "completed",
                        "requires_approval": false,
                        "command": "uname -a",
                        "input": { "command": "uname -a" },
                        "output": "Linux test-host\n",
                        "stdout": "Linux test-host\n",
                        "stderr": "",
                        "exit_code": 0,
                        "duration_ms": 12,
                        "timed_out": false
                    }
                }
            ]
        })
    );
}

#[test]
fn health_contract_can_report_typed_message_parts_capability() {
    let health: HealthResponse = from_value(json!({
        "status": "ok",
        "service": "deepseek-mobile-web-server",
        "protocol": "mobile-web-v1",
        "model": "mock",
        "capabilities": ["typed_message_parts"]
    }))
    .expect("health response must deserialize");

    assert_eq!(
        serde_json::to_value(health).expect("health response must serialize"),
        json!({
            "status": "ok",
            "service": "deepseek-mobile-web-server",
            "protocol": "mobile-web-v1",
            "model": "mock",
            "capabilities": ["typed_message_parts"]
        })
    );
}
