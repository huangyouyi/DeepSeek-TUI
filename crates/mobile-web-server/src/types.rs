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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolPartData {
    pub turn_id: String,
    pub agent_turn_id: String,
    pub tool_call_id: String,
    pub tool: String,
    pub title: String,
    pub status: String,
    pub requires_approval: bool,
    pub command: String,
    #[serde(default)]
    pub input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
}

#[must_use]
pub fn text_part(text: impl Into<String>, data: Value) -> MessagePart {
    MessagePart {
        id: format!("part-{}", uuid::Uuid::new_v4()),
        kind: "text".to_string(),
        text: Some(text.into()),
        data,
    }
}

#[must_use]
pub fn tool_part(data: ToolPartData) -> MessagePart {
    MessagePart {
        id: format!("part-{}", uuid::Uuid::new_v4()),
        kind: "tool".to_string(),
        text: None,
        data: serde_json::to_value(data).expect("tool part data must serialize"),
    }
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
    pub fn remote_shell(
        id: String,
        session_id: String,
        command: String,
        cwd: Option<String>,
        created_at_ms: u64,
        status: String,
    ) -> Self {
        Self {
            id,
            session_id,
            command,
            cwd,
            created_at_ms,
            status,
        }
    }

    #[must_use]
    pub(crate) const fn risk_level(&self) -> &'static str {
        "high"
    }

    #[must_use]
    pub(crate) const fn risk_reason(&self) -> &'static str {
        "advanced command requires explicit approval"
    }

    #[must_use]
    pub(crate) const fn target(&self) -> &'static str {
        "remote.shell.exec"
    }

    #[must_use]
    pub(crate) const fn target_label(&self) -> &'static str {
        "Remote shell command"
    }

    #[must_use]
    pub(crate) fn agent_turn_id(&self) -> Option<String> {
        agent_approval_origins()
            .lock()
            .expect("agent approval origin mutex must not be poisoned")
            .get(&self.id)
            .and_then(|origin| origin.agent_turn_id.clone())
    }

    pub(crate) fn set_agent_turn_id(&self, agent_turn_id: impl Into<String>) {
        let mut origins = agent_approval_origins()
            .lock()
            .expect("agent approval origin mutex must not be poisoned");
        origins.entry(self.id.clone()).or_default().agent_turn_id = Some(agent_turn_id.into());
    }

    #[must_use]
    pub(crate) fn agent_message_id(&self) -> Option<String> {
        agent_approval_origins()
            .lock()
            .expect("agent approval origin mutex must not be poisoned")
            .get(&self.id)
            .and_then(|origin| origin.message_id.clone())
    }

    #[must_use]
    pub(crate) fn agent_tool_part_id(&self) -> Option<String> {
        agent_approval_origins()
            .lock()
            .expect("agent approval origin mutex must not be poisoned")
            .get(&self.id)
            .and_then(|origin| origin.tool_part_id.clone())
    }

    pub(crate) fn set_agent_tool_part(
        &self,
        message_id: impl Into<String>,
        tool_part_id: impl Into<String>,
    ) {
        let mut origins = agent_approval_origins()
            .lock()
            .expect("agent approval origin mutex must not be poisoned");
        let origin = origins.entry(self.id.clone()).or_default();
        origin.message_id = Some(message_id.into());
        origin.tool_part_id = Some(tool_part_id.into());
    }
}

impl Serialize for PendingApproval {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let agent_turn_id = self.agent_turn_id();
        let mut state = serializer.serialize_struct(
            "PendingApproval",
            10 + usize::from(self.cwd.is_some()) + usize::from(agent_turn_id.is_some()),
        )?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("session_id", &self.session_id)?;
        state.serialize_field("command", &self.command)?;
        if let Some(cwd) = &self.cwd {
            state.serialize_field("cwd", cwd)?;
        }
        state.serialize_field("created_at_ms", &self.created_at_ms)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("risk_level", self.risk_level())?;
        state.serialize_field("risk_reason", self.risk_reason())?;
        state.serialize_field("target", self.target())?;
        state.serialize_field("target_label", self.target_label())?;
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
                .entry(fields.id.clone())
                .or_default()
                .agent_turn_id = Some(agent_turn_id);
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

fn agent_approval_origins() -> &'static Mutex<HashMap<String, ApprovalOrigin>> {
    static ORIGINS: OnceLock<Mutex<HashMap<String, ApprovalOrigin>>> = OnceLock::new();
    ORIGINS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ApprovalOrigin {
    agent_turn_id: Option<String>,
    message_id: Option<String>,
    tool_part_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTurnRequest {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_turn_id: Option<String>,
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
