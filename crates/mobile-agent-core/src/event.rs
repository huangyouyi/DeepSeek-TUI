use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobileEventKind {
    SessionCreated,
    BootstrapStepCreated,
    ApprovalRequired,
    ToolCallStarted,
    ToolStdout,
    ToolStderr,
    ToolCallCompleted,
    ToolTimedOut,
    ToolCancelled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MobileEvent {
    pub seq: u64,
    pub kind: MobileEventKind,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolEventStreamBuilder {
    call_id: String,
    tool: String,
    next_seq: u64,
}

impl ToolEventStreamBuilder {
    #[must_use]
    pub fn new(call_id: impl Into<String>, tool: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            tool: tool.into(),
            next_seq: 1,
        }
    }

    #[must_use]
    pub fn starting_at(call_id: impl Into<String>, tool: impl Into<String>, next_seq: u64) -> Self {
        Self {
            call_id: call_id.into(),
            tool: tool.into(),
            next_seq,
        }
    }

    pub fn started(&mut self) -> MobileEvent {
        self.event(MobileEventKind::ToolCallStarted, serde_json::json!({}))
    }

    pub fn stdout(&mut self, chunk: impl Into<String>) -> MobileEvent {
        self.event(
            MobileEventKind::ToolStdout,
            serde_json::json!({
                "chunk": chunk.into(),
            }),
        )
    }

    pub fn stderr(&mut self, chunk: impl Into<String>) -> MobileEvent {
        self.event(
            MobileEventKind::ToolStderr,
            serde_json::json!({
                "chunk": chunk.into(),
            }),
        )
    }

    pub fn completed(&mut self, exit_code: i32, duration_ms: u64) -> MobileEvent {
        self.event(
            MobileEventKind::ToolCallCompleted,
            serde_json::json!({
                "exit_code": exit_code,
                "duration_ms": duration_ms,
            }),
        )
    }

    pub fn timed_out(
        &mut self,
        timeout_ms: u64,
        duration_ms: u64,
        reason: impl Into<String>,
    ) -> MobileEvent {
        self.event(
            MobileEventKind::ToolTimedOut,
            serde_json::json!({
                "timeout_ms": timeout_ms,
                "duration_ms": duration_ms,
                "reason": reason.into(),
            }),
        )
    }

    pub fn cancelled(&mut self, duration_ms: u64, reason: impl Into<String>) -> MobileEvent {
        self.event(
            MobileEventKind::ToolCancelled,
            serde_json::json!({
                "duration_ms": duration_ms,
                "reason": reason.into(),
            }),
        )
    }

    fn event(&mut self, kind: MobileEventKind, fields: Value) -> MobileEvent {
        let seq = self.next_seq;
        self.next_seq += 1;

        let mut payload = serde_json::Map::new();
        payload.insert("call_id".to_string(), Value::String(self.call_id.clone()));
        payload.insert("tool".to_string(), Value::String(self.tool.clone()));
        if let Value::Object(fields) = fields {
            payload.extend(fields);
        }

        MobileEvent {
            seq,
            kind,
            payload: Value::Object(payload),
        }
    }
}
