use crate::approval::ApprovalRequest;
use crate::risk::RiskLevel;
use crate::session::MobileSession;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapStepSnapshot {
    pub step_id: String,
    pub command: String,
    pub submitted_output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub title: String,
    pub event_count: usize,
    pub bootstrap_step_count: usize,
    pub bootstrap_steps: Vec<BootstrapStepSnapshot>,
}

impl SessionSnapshot {
    #[must_use]
    pub fn from_session(session: &MobileSession) -> Self {
        Self::from(session)
    }
}

impl From<&MobileSession> for SessionSnapshot {
    fn from(session: &MobileSession) -> Self {
        Self {
            session_id: session.id.clone(),
            title: session.title.clone(),
            event_count: session.events.len(),
            bootstrap_step_count: session.bootstrap.steps.len(),
            bootstrap_steps: session
                .bootstrap
                .steps
                .iter()
                .map(|step| BootstrapStepSnapshot {
                    step_id: step.step_id.clone(),
                    command: step.command.clone(),
                    submitted_output: step.submitted_output.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingTurnSnapshot {
    pub session_id: String,
    pub turn_id: String,
    pub prompt: String,
    pub created_at: String,
    pub pending_approval_ids: Vec<String>,
    pub pending_tool_call_ids: Vec<String>,
    #[serde(default)]
    pub pending_approvals: Vec<PendingApprovalSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApprovalSnapshot {
    pub approval_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub risk_level: RiskLevel,
    pub risk_reason: String,
    pub user_visible_summary: String,
}

impl PendingApprovalSnapshot {
    #[must_use]
    pub fn from_request(
        request: &ApprovalRequest,
        user_visible_summary: impl Into<String>,
    ) -> Self {
        Self {
            approval_id: request.approval_id.clone(),
            call_id: request.call_id.clone(),
            tool_name: request.tool_name.clone(),
            risk_level: request.risk.level,
            risk_reason: request.risk.reason.clone(),
            user_visible_summary: user_visible_summary.into(),
        }
    }
}

pub trait SnapshotStore {
    fn save(&mut self, snapshot: SessionSnapshot);
    fn load(&self, session_id: &str) -> Option<SessionSnapshot>;
}

pub trait PendingTurnStore {
    fn save_pending_turn(&mut self, snapshot: PendingTurnSnapshot);
    fn load_pending_turn(&self, session_id: &str, turn_id: &str) -> Option<PendingTurnSnapshot>;
}

#[derive(Debug, Default)]
pub struct InMemorySnapshotStore {
    snapshots: HashMap<String, SessionSnapshot>,
    pending_turns: HashMap<(String, String), PendingTurnSnapshot>,
}

impl SnapshotStore for InMemorySnapshotStore {
    fn save(&mut self, snapshot: SessionSnapshot) {
        self.snapshots.insert(snapshot.session_id.clone(), snapshot);
    }

    fn load(&self, session_id: &str) -> Option<SessionSnapshot> {
        self.snapshots.get(session_id).cloned()
    }
}

impl PendingTurnStore for InMemorySnapshotStore {
    fn save_pending_turn(&mut self, snapshot: PendingTurnSnapshot) {
        self.pending_turns.insert(
            (snapshot.session_id.clone(), snapshot.turn_id.clone()),
            snapshot,
        );
    }

    fn load_pending_turn(&self, session_id: &str, turn_id: &str) -> Option<PendingTurnSnapshot> {
        self.pending_turns
            .get(&(session_id.to_string(), turn_id.to_string()))
            .cloned()
    }
}

#[derive(Debug)]
pub struct SqliteSnapshotStore {
    connection: Connection,
}

impl SqliteSnapshotStore {
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    fn from_connection(connection: Connection) -> rusqlite::Result<Self> {
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS session_snapshots (
                session_id TEXT PRIMARY KEY NOT NULL,
                snapshot_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS pending_turns (
                session_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                snapshot_json TEXT NOT NULL,
                PRIMARY KEY (session_id, turn_id)
            );
            ",
        )?;
        Ok(Self { connection })
    }
}

impl SnapshotStore for SqliteSnapshotStore {
    fn save(&mut self, snapshot: SessionSnapshot) {
        let snapshot_json =
            serde_json::to_string(&snapshot).expect("session snapshot should serialize");
        self.connection
            .execute(
                "
                INSERT INTO session_snapshots (session_id, snapshot_json)
                VALUES (?1, ?2)
                ON CONFLICT(session_id) DO UPDATE SET
                    snapshot_json = excluded.snapshot_json
                ",
                params![snapshot.session_id, snapshot_json],
            )
            .expect("session snapshot should save to sqlite");
    }

    fn load(&self, session_id: &str) -> Option<SessionSnapshot> {
        let snapshot_json = self
            .connection
            .query_row(
                "SELECT snapshot_json FROM session_snapshots WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, String>(0),
            )
            .ok()?;
        serde_json::from_str(&snapshot_json).ok()
    }
}

impl PendingTurnStore for SqliteSnapshotStore {
    fn save_pending_turn(&mut self, snapshot: PendingTurnSnapshot) {
        let snapshot_json =
            serde_json::to_string(&snapshot).expect("pending turn should serialize");
        self.connection
            .execute(
                "
                INSERT INTO pending_turns (session_id, turn_id, snapshot_json)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(session_id, turn_id) DO UPDATE SET
                    snapshot_json = excluded.snapshot_json
                ",
                params![snapshot.session_id, snapshot.turn_id, snapshot_json],
            )
            .expect("pending turn should save to sqlite");
    }

    fn load_pending_turn(&self, session_id: &str, turn_id: &str) -> Option<PendingTurnSnapshot> {
        let snapshot_json = self
            .connection
            .query_row(
                "
                SELECT snapshot_json
                FROM pending_turns
                WHERE session_id = ?1 AND turn_id = ?2
                ",
                params![session_id, turn_id],
                |row| row.get::<_, String>(0),
            )
            .ok()?;
        serde_json::from_str(&snapshot_json).ok()
    }
}
