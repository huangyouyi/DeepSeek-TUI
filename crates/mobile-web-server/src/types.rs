use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::ser::SerializeStruct;
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
pub struct SshCheckResponse {
    pub status: String,
    pub target: SshTarget,
    pub check_id: String,
    pub command: String,
    pub requires_approval: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub timed_out: bool,
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
pub struct DiagnosticPreset {
    pub key: String,
    pub label: String,
    pub command: String,
    pub requires_approval: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPrepareRequest {
    pub session_id: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingApproval {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub cwd: Option<String>,
    pub created_at_ms: u64,
    pub status: String,
}

impl PendingApproval {
    #[must_use]
    pub(crate) fn agent_turn_id(&self) -> Option<String> {
        agent_approval_origins()
            .lock()
            .expect("agent approval origin mutex must not be poisoned")
            .get(&self.id)
            .cloned()
    }
}

impl Serialize for PendingApproval {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let agent_turn_id = self.agent_turn_id();
        let mut state = serializer
            .serialize_struct("PendingApproval", 6 + usize::from(agent_turn_id.is_some()))?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("session_id", &self.session_id)?;
        state.serialize_field("command", &self.command)?;
        if let Some(cwd) = &self.cwd {
            state.serialize_field("cwd", cwd)?;
        }
        state.serialize_field("created_at_ms", &self.created_at_ms)?;
        state.serialize_field("status", &self.status)?;
        if let Some(agent_turn_id) = agent_turn_id {
            state.serialize_field("agent_turn_id", &agent_turn_id)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for PendingApproval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PendingApprovalFields {
            id: String,
            session_id: String,
            command: String,
            #[serde(default)]
            cwd: Option<String>,
            created_at_ms: u64,
            status: String,
            #[serde(default)]
            agent_turn_id: Option<String>,
        }

        let fields = PendingApprovalFields::deserialize(deserializer)?;
        if let Some(agent_turn_id) = fields.agent_turn_id {
            agent_approval_origins()
                .lock()
                .expect("agent approval origin mutex must not be poisoned")
                .insert(fields.id.clone(), agent_turn_id);
        }
        Ok(Self {
            id: fields.id,
            session_id: fields.session_id,
            command: fields.command,
            cwd: fields.cwd,
            created_at_ms: fields.created_at_ms,
            status: fields.status,
        })
    }
}

fn agent_approval_origins() -> &'static Mutex<HashMap<String, String>> {
    static ORIGINS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    ORIGINS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTurnRequest {
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentExecutedTool {
    pub tool: String,
    pub command: String,
    pub requires_approval: bool,
    pub exit_code: Option<i32>,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTurnResponse {
    pub session_id: String,
    pub turn_id: String,
    pub status: String,
    pub assistant_text: String,
    pub executed_tools: Vec<AgentExecutedTool>,
    pub pending_approvals: Vec<PendingApproval>,
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
