pub mod agent_model;
pub mod agent_tool_policy;
pub mod approvals;
pub mod diagnostics;
pub mod events;
pub mod model_config;
pub mod routes;
pub mod ssh_exec;
pub mod state;
pub mod types;

pub use routes::{
    MobileWebServerConfig, app_router, app_router_with_access_token, app_router_with_config,
    app_router_with_config_access_token_and_model, app_router_with_config_and_access_token,
    app_router_with_config_and_model, app_router_with_runner,
};
pub use ssh_exec::{CommandRunner, SshCommandOutput, SystemSshCommandRunner};
pub use state::AppState;
pub use types::{
    ApiErrorBody, ApprovalRespondRequest, ApprovalResponse, AuditEntry, CommandPrepareRequest,
    DiagnosticPreset, DiagnosticRequest, DiagnosticResponse, HealthResponse, Message, MessagePart,
    PendingApproval, ServerEvent, SessionSummary, SshTarget, ToolPartData, text_part, tool_part,
};

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::{
        ApiErrorBody, ApprovalRespondRequest, AuditEntry, CommandPrepareRequest, DiagnosticRequest,
        HealthResponse, Message, MessagePart, PendingApproval, ServerEvent, SessionSummary,
        SshTarget,
    };

    #[test]
    fn health_response_serializes_stable_fields() {
        let value = serde_json::to_value(HealthResponse {
            status: "ok".to_string(),
            service: "deepseek-mobile-web-server".to_string(),
            protocol: "mobile-web-v1".to_string(),
            model: "mock".to_string(),
            capabilities: Vec::new(),
        })
        .expect("health response must serialize");

        assert_eq!(
            value,
            json!({
                "status": "ok",
                "service": "deepseek-mobile-web-server",
                "protocol": "mobile-web-v1",
                "model": "mock"
            })
        );
    }

    #[test]
    fn session_message_and_part_fields_are_stable() {
        let session = SessionSummary {
            id: "session-1".to_string(),
            title: "Linux rescue".to_string(),
            created_at_ms: 10,
            updated_at_ms: 20,
        };
        let message = Message {
            id: "message-1".to_string(),
            session_id: session.id.clone(),
            role: "assistant".to_string(),
            created_at_ms: 30,
            parts: vec![MessagePart {
                id: "part-1".to_string(),
                kind: "text".to_string(),
                text: Some("ready".to_string()),
                data: json!({"status": "ok"}),
            }],
        };

        assert_eq!(
            serde_json::to_value(session).expect("session must serialize"),
            json!({
                "id": "session-1",
                "title": "Linux rescue",
                "created_at_ms": 10,
                "updated_at_ms": 20
            })
        );
        assert_eq!(
            serde_json::to_value(message).expect("message must serialize"),
            json!({
                "id": "message-1",
                "session_id": "session-1",
                "role": "assistant",
                "created_at_ms": 30,
                "parts": [{
                    "id": "part-1",
                    "kind": "text",
                    "text": "ready",
                    "data": {"status": "ok"}
                }]
            })
        );
    }

    #[test]
    fn command_and_approval_request_fields_are_stable() {
        let command = CommandPrepareRequest {
            session_id: "session-1".to_string(),
            command: "uptime".to_string(),
            cwd: Some("/tmp".to_string()),
        };
        let approval = ApprovalRespondRequest {
            response: "approve_once".to_string(),
        };

        assert_eq!(
            serde_json::to_value(command).expect("command must serialize"),
            json!({
                "session_id": "session-1",
                "command": "uptime",
                "cwd": "/tmp"
            })
        );
        assert_eq!(
            serde_json::to_value(approval).expect("approval must serialize"),
            json!({"response": "approve_once"})
        );
    }

    #[test]
    fn target_diagnostic_audit_and_error_fields_are_stable() {
        let target = SshTarget {
            host: "192.168.30.244".to_string(),
            user: "root".to_string(),
            port: 22,
            key_present: false,
        };
        let diagnostic = DiagnosticRequest {
            session_id: "session-1".to_string(),
            diagnostic: "system_info".to_string(),
        };
        let audit = AuditEntry {
            id: "audit-1".to_string(),
            session_id: Some("session-1".to_string()),
            kind: "diagnostic.completed".to_string(),
            created_at_ms: 40,
            summary: "system info completed".to_string(),
            metadata: json!({"exit_code": 0}),
        };
        let error = ApiErrorBody {
            code: "bad_request".to_string(),
            message: "missing session_id".to_string(),
        };

        assert_eq!(
            serde_json::to_value(target).expect("target must serialize"),
            json!({
                "host": "192.168.30.244",
                "user": "root",
                "port": 22,
                "key_present": false
            })
        );
        assert_eq!(
            serde_json::to_value(diagnostic).expect("diagnostic must serialize"),
            json!({
                "session_id": "session-1",
                "diagnostic": "system_info"
            })
        );
        assert_eq!(
            serde_json::to_value(audit).expect("audit must serialize"),
            json!({
                "id": "audit-1",
                "session_id": "session-1",
                "kind": "diagnostic.completed",
                "created_at_ms": 40,
                "summary": "system info completed",
                "metadata": {"exit_code": 0}
            })
        );
        assert_eq!(
            serde_json::to_value(error).expect("error must serialize"),
            json!({
                "code": "bad_request",
                "message": "missing session_id"
            })
        );
    }

    #[test]
    fn pending_approval_and_events_are_stable() {
        let approval = PendingApproval {
            id: "approval-1".to_string(),
            session_id: "session-1".to_string(),
            command: "systemctl restart ssh".to_string(),
            cwd: None,
            created_at_ms: 50,
            status: "pending".to_string(),
        };
        let event = ServerEvent {
            event_type: "approval.asked".to_string(),
            payload: serde_json::to_value(&approval).expect("approval must serialize"),
        };

        let value = serde_json::to_value(event).expect("event must serialize");
        assert_eq!(
            value.get("type"),
            Some(&Value::String("approval.asked".to_string()))
        );
        assert_eq!(value["payload"]["id"], "approval-1");
        assert_eq!(value["payload"]["status"], "pending");
    }
}
