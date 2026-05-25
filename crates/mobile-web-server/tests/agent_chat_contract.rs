use deepseek_mobile_web_server::types::{
    AgentExecutedTool, AgentTurnRequest, AgentTurnResponse, PendingApproval,
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
        pending_approvals: vec![PendingApproval {
            id: "approval-1".to_string(),
            session_id: "session-1".to_string(),
            command: "systemctl restart ssh".to_string(),
            cwd: Some("/".to_string()),
            created_at_ms: 123,
            status: "pending".to_string(),
        }],
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
                "status": "pending"
            }]
        })
    );
}
