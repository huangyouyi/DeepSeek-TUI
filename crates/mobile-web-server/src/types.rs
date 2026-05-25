use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub protocol: String,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub created_at_ms: u64,
    pub parts: Vec<MessagePart>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessagePart {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshTarget {
    pub host: String,
    pub user: String,
    pub port: u16,
    pub key_present: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRequest {
    pub session_id: String,
    pub diagnostic: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticResponse {
    pub session_id: String,
    pub diagnostic: String,
    pub status: String,
    pub result: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPrepareRequest {
    pub session_id: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApproval {
    pub id: String,
    pub session_id: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub created_at_ms: u64,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRespondRequest {
    pub response: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApprovalResponse {
    pub approval: PendingApproval,
    pub status: String,
    #[serde(default)]
    pub result: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub kind: String,
    pub created_at_ms: u64,
    pub summary: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}
