use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::types::{
    AuditEntry, Message, MessagePart, PendingApproval, ServerEvent, SessionSummary, SshTarget,
};

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
    session_allows: HashSet<SessionAllowKey>,
    audit: VecDeque<AuditEntry>,
    ssh_target: SshTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SessionAllowKey {
    session_id: String,
    command: String,
    cwd: Option<String>,
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
                session_allows: HashSet::new(),
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
        sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at_ms));
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
    pub fn delete_session(&self, session_id: &str) -> bool {
        let mut state = self
            .inner
            .lock()
            .expect("app state mutex must not be poisoned");
        if state.sessions.remove(session_id).is_none() {
            return false;
        }

        state.messages.remove(session_id);
        state
            .pending_approvals
            .retain(|_, approval| approval.session_id != session_id);
        state
            .session_allows
            .retain(|allow| allow.session_id != session_id);
        true
    }

    #[must_use]
    pub fn update_session_title(
        &self,
        session_id: &str,
        title: String,
        updated_at_ms: u64,
    ) -> Option<SessionSummary> {
        let mut state = self
            .inner
            .lock()
            .expect("app state mutex must not be poisoned");
        let session = state.sessions.get_mut(session_id)?;
        session.title = title;
        session.updated_at_ms = updated_at_ms;
        Some(session.clone())
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

    pub fn push_message_part(&self, session_id: &str, message_id: &str, part: MessagePart) -> bool {
        let mut state = self
            .inner
            .lock()
            .expect("app state mutex must not be poisoned");
        let Some(messages) = state.messages.get_mut(session_id) else {
            return false;
        };
        let Some(message) = messages.iter_mut().find(|message| message.id == message_id) else {
            return false;
        };
        message.parts.push(part);
        true
    }

    pub fn upsert_message_part(
        &self,
        session_id: &str,
        message_id: &str,
        part: MessagePart,
    ) -> bool {
        let mut state = self
            .inner
            .lock()
            .expect("app state mutex must not be poisoned");
        let Some(messages) = state.messages.get_mut(session_id) else {
            return false;
        };
        let Some(message) = messages.iter_mut().find(|message| message.id == message_id) else {
            return false;
        };
        if let Some(existing) = message
            .parts
            .iter_mut()
            .find(|existing| existing.id == part.id)
        {
            *existing = part;
        } else {
            message.parts.push(part);
        }
        true
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

    #[must_use]
    pub fn remove_pending_approvals_for_agent_turn(
        &self,
        agent_turn_id: &str,
    ) -> Vec<PendingApproval> {
        let mut state = self
            .inner
            .lock()
            .expect("app state mutex must not be poisoned");
        let ids = state
            .pending_approvals
            .values()
            .filter(|approval| approval.agent_turn_id().as_deref() == Some(agent_turn_id))
            .map(|approval| approval.id.clone())
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| state.pending_approvals.remove(&id))
            .collect()
    }

    pub fn grant_session_allow(&self, session_id: &str, command: &str, cwd: Option<&str>) {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .session_allows
            .insert(SessionAllowKey {
                session_id: session_id.to_string(),
                command: command.to_string(),
                cwd: cwd.map(ToOwned::to_owned),
            });
    }

    #[must_use]
    pub fn is_session_allowed(&self, session_id: &str, command: &str, cwd: Option<&str>) -> bool {
        self.inner
            .lock()
            .expect("app state mutex must not be poisoned")
            .session_allows
            .contains(&SessionAllowKey {
                session_id: session_id.to_string(),
                command: command.to_string(),
                cwd: cwd.map(ToOwned::to_owned),
            })
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
