use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::types::{AuditEntry, Message, PendingApproval, ServerEvent, SessionSummary, SshTarget};

const AUDIT_RING_CAPACITY: usize = 128;
const EVENT_RING_CAPACITY: usize = 256;

#[derive(Clone, Debug)]
pub struct AppState {
    inner: Arc<Mutex<AppStateInner>>,
    events: broadcast::Sender<ServerEvent>,
}

#[derive(Debug)]
struct AppStateInner {
    sessions: HashMap<String, SessionSummary>,
    messages: HashMap<String, Vec<Message>>,
    pending_approvals: HashMap<String, PendingApproval>,
    audit: VecDeque<AuditEntry>,
    ssh_target: SshTarget,
}

impl AppState {
    #[must_use]
    pub fn new(ssh_target: SshTarget) -> Self {
        let (events, _) = broadcast::channel(EVENT_RING_CAPACITY);
        Self {
            inner: Arc::new(Mutex::new(AppStateInner {
                sessions: HashMap::new(),
                messages: HashMap::new(),
                pending_approvals: HashMap::new(),
                audit: VecDeque::with_capacity(AUDIT_RING_CAPACITY),
                ssh_target,
            })),
            events,
        }
    }

    #[must_use]
    pub fn default_target() -> SshTarget {
        SshTarget {
            host: "192.168.30.244".to_string(),
            user: "root".to_string(),
            port: 22,
            key_present: false,
        }
    }

    #[must_use]
    pub fn ssh_target(&self) -> SshTarget {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .ssh_target
            .clone()
    }

    pub fn set_ssh_target(&self, target: SshTarget) {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .ssh_target = target;
    }

    #[must_use]
    pub fn sessions(&self) -> Vec<SessionSummary> {
        let mut sessions: Vec<_> = self
            .inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .sessions
            .values()
            .cloned()
            .collect();
        sessions.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
        sessions
    }

    pub fn upsert_session(&self, session: SessionSummary) {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .sessions
            .insert(session.id.clone(), session);
    }

    #[must_use]
    pub fn messages(&self, session_id: &str) -> Vec<Message> {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .messages
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn push_message(&self, message: Message) {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .messages
            .entry(message.session_id.clone())
            .or_default()
            .push(message);
    }

    #[must_use]
    pub fn pending_approvals(&self) -> Vec<PendingApproval> {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .pending_approvals
            .values()
            .cloned()
            .collect()
    }

    pub fn insert_pending_approval(&self, approval: PendingApproval) {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .pending_approvals
            .insert(approval.id.clone(), approval);
    }

    #[must_use]
    pub fn remove_pending_approval(&self, approval_id: &str) -> Option<PendingApproval> {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .pending_approvals
            .remove(approval_id)
    }

    pub fn push_audit(&self, entry: AuditEntry) {
        let mut state = self
            .inner
            .lock()
            .expect("app state mutex must not be poisoned");
        if state.audit.len() == AUDIT_RING_CAPACITY {
            state.audit.pop_front();
        }
        state.audit.push_back(entry);
    }

    #[must_use]
    pub fn audit_recent(&self) -> Vec<AuditEntry> {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .audit
            .iter()
            .cloned()
            .collect()
    }

    pub fn broadcast(&self, event: ServerEvent) {
        let _ = self.events.send(event);
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.events.subscribe()
    }
}
