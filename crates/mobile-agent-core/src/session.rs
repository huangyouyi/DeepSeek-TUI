use thiserror::Error;
use uuid::Uuid;

use crate::bootstrap::{BootstrapSession, BootstrapStepError};
use crate::event::{MobileEvent, MobileEventKind};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MobileCoreError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("bootstrap step not found: {0}")]
    BootstrapStepNotFound(String),
}

#[derive(Debug, Clone)]
pub struct MobileSession {
    pub id: String,
    pub title: String,
    pub bootstrap: BootstrapSession,
    pub events: Vec<MobileEvent>,
}

#[derive(Debug, Default)]
pub struct MobileAgentCore {
    sessions: Vec<MobileSession>,
}

impl MobileAgentCore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&mut self, title: impl Into<String>) -> String {
        let id = format!("mobile-session-{}", Uuid::new_v4());
        let session = MobileSession {
            id: id.clone(),
            title: title.into(),
            bootstrap: BootstrapSession::default(),
            events: vec![MobileEvent {
                seq: 1,
                kind: MobileEventKind::SessionCreated,
                payload: serde_json::json!({ "session_id": id }),
            }],
        };
        self.sessions.push(session);
        id
    }

    pub fn session(&self, id: &str) -> Result<&MobileSession, MobileCoreError> {
        self.sessions
            .iter()
            .find(|session| session.id == id)
            .ok_or_else(|| MobileCoreError::SessionNotFound(id.to_string()))
    }

    pub fn homebrew_bootstrap_step(
        &mut self,
        session_id: &str,
    ) -> Result<MobileEvent, MobileCoreError> {
        let session = self.session_mut(session_id)?;
        let step = session.bootstrap.homebrew_bootstrap_step();
        let event = MobileEvent {
            seq: next_seq(&session.events),
            kind: MobileEventKind::BootstrapStepCreated,
            payload: serde_json::json!({
                "session_id": session_id,
                "step_id": step.step_id,
                "command": step.command,
            }),
        };
        session.events.push(event.clone());
        Ok(event)
    }

    pub fn submit_bootstrap_output(
        &mut self,
        session_id: &str,
        step_id: &str,
        output: impl Into<String>,
    ) -> Result<MobileEvent, MobileCoreError> {
        let session = self.session_mut(session_id)?;
        let output = output.into();
        session
            .bootstrap
            .submit_output(step_id, output.clone())
            .map_err(MobileCoreError::from)?;
        let event = MobileEvent {
            seq: next_seq(&session.events),
            kind: MobileEventKind::BootstrapStepCreated,
            payload: serde_json::json!({
                "action": "bootstrap_output_submitted",
                "session_id": session_id,
                "step_id": step_id,
                "output": output,
            }),
        };
        session.events.push(event.clone());
        Ok(event)
    }

    fn session_mut(&mut self, id: &str) -> Result<&mut MobileSession, MobileCoreError> {
        self.sessions
            .iter_mut()
            .find(|session| session.id == id)
            .ok_or_else(|| MobileCoreError::SessionNotFound(id.to_string()))
    }
}

impl From<BootstrapStepError> for MobileCoreError {
    fn from(value: BootstrapStepError) -> Self {
        match value {
            BootstrapStepError::StepNotFound(step_id) => Self::BootstrapStepNotFound(step_id),
        }
    }
}

fn next_seq(events: &[MobileEvent]) -> u64 {
    events.last().map_or(1, |event| event.seq + 1)
}
