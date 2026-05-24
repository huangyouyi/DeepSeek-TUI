use crate::approval::{ApprovalDecision, ApprovalRequest};
use crate::remote_schema::{RemoteToolCall, RemoteToolOutput};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerMaintenanceAuditNonce {
    pub redacted: Option<bool>,
    pub label: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerMaintenanceAuditRecord {
    pub event: String,
    pub status: String,
    pub nonce: Option<RunnerMaintenanceAuditNonce>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerBrowserAuditRecord {
    pub record_type: String,
    pub tool: String,
    pub action: Option<String>,
    pub session_id: Option<String>,
    pub selector: Option<String>,
    pub reason: Option<String>,
    pub risk: Option<String>,
    pub approval_required: Option<bool>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub kind: String,
    pub action: String,
    pub created_at: String,
    pub summary: String,
    pub detail: String,
    pub call_id: Option<String>,
    pub approval_id: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    #[must_use]
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn append_bootstrap_command(
        &mut self,
        session_id: impl Into<String>,
        summary: impl Into<String>,
        command: impl Into<String>,
    ) -> &AuditEntry {
        self.append_bootstrap_command_at(now_created_at(), session_id, summary, command)
    }

    pub fn append_bootstrap_command_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        summary: impl Into<String>,
        command: impl Into<String>,
    ) -> &AuditEntry {
        self.append_entry(AuditEntryDraft {
            kind: "bootstrap".to_string(),
            action: "command".to_string(),
            created_at: created_at.into(),
            summary: summary.into(),
            detail: command.into(),
            call_id: None,
            approval_id: None,
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_bootstrap_output(
        &mut self,
        session_id: impl Into<String>,
        summary: impl Into<String>,
        output: impl Into<String>,
    ) -> &AuditEntry {
        self.append_bootstrap_output_at(now_created_at(), session_id, summary, output)
    }

    pub fn append_bootstrap_output_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        summary: impl Into<String>,
        output: impl Into<String>,
    ) -> &AuditEntry {
        self.append_entry(AuditEntryDraft {
            kind: "bootstrap".to_string(),
            action: "output".to_string(),
            created_at: created_at.into(),
            summary: summary.into(),
            detail: output.into(),
            call_id: None,
            approval_id: None,
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_remote_tool_call(
        &mut self,
        session_id: impl Into<String>,
        call: &RemoteToolCall,
    ) -> &AuditEntry {
        self.append_remote_tool_call_at(now_created_at(), session_id, call)
    }

    pub fn append_remote_tool_call_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        call: &RemoteToolCall,
    ) -> &AuditEntry {
        self.append_entry(AuditEntryDraft {
            kind: "remote_tool".to_string(),
            action: "call".to_string(),
            created_at: created_at.into(),
            summary: call.name.to_string(),
            detail: call.arguments.to_string(),
            call_id: Some(call.call_id.clone()),
            approval_id: None,
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_remote_tool_output(
        &mut self,
        session_id: impl Into<String>,
        output: &RemoteToolOutput,
    ) -> &AuditEntry {
        self.append_remote_tool_output_at(now_created_at(), session_id, output)
    }

    pub fn append_remote_tool_output_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        output: &RemoteToolOutput,
    ) -> &AuditEntry {
        self.append_entry(AuditEntryDraft {
            kind: "remote_tool".to_string(),
            action: "output".to_string(),
            created_at: created_at.into(),
            summary: if output.success { "success" } else { "failure" }.to_string(),
            detail: output.result.to_string(),
            call_id: Some(output.call_id.clone()),
            approval_id: None,
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_approval_required(
        &mut self,
        session_id: impl Into<String>,
        request: &ApprovalRequest,
        user_visible_summary: impl Into<String>,
    ) -> &AuditEntry {
        self.append_approval_required_at(
            now_created_at(),
            session_id,
            request,
            user_visible_summary,
        )
    }

    pub fn append_approval_required_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        request: &ApprovalRequest,
        user_visible_summary: impl Into<String>,
    ) -> &AuditEntry {
        let user_visible_summary = user_visible_summary.into();
        let detail = json!({
            "tool_name": request.tool_name,
            "risk": {
                "level": format!("{:?}", request.risk.level),
                "reason": request.risk.reason.clone(),
            },
            "arguments": request.call.arguments.clone(),
            "user_visible_summary": user_visible_summary,
        });

        self.append_entry(AuditEntryDraft {
            kind: "approval".to_string(),
            action: "required".to_string(),
            created_at: created_at.into(),
            summary: user_visible_summary,
            detail: detail.to_string(),
            call_id: Some(request.call_id.clone()),
            approval_id: Some(request.approval_id.clone()),
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_approval_decision(
        &mut self,
        session_id: impl Into<String>,
        approval_id: impl Into<String>,
        call_id: impl Into<String>,
        decision: ApprovalDecision,
        summary: impl Into<String>,
    ) -> &AuditEntry {
        self.append_approval_decision_at(
            now_created_at(),
            session_id,
            approval_id,
            call_id,
            decision,
            summary,
        )
    }

    pub fn append_approval_decision_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        approval_id: impl Into<String>,
        call_id: impl Into<String>,
        decision: ApprovalDecision,
        summary: impl Into<String>,
    ) -> &AuditEntry {
        let action = match decision {
            ApprovalDecision::Approved => "approved",
            ApprovalDecision::Denied => "denied",
        };

        self.append_entry(AuditEntryDraft {
            kind: "approval".to_string(),
            action: action.to_string(),
            created_at: created_at.into(),
            summary: summary.into(),
            detail: String::new(),
            call_id: Some(call_id.into()),
            approval_id: Some(approval_id.into()),
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_runner_maintenance_audit(
        &mut self,
        session_id: impl Into<String>,
        record: &RunnerMaintenanceAuditRecord,
    ) -> &AuditEntry {
        self.append_runner_maintenance_audit_at(now_created_at(), session_id, record)
    }

    pub fn append_runner_maintenance_audit_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        record: &RunnerMaintenanceAuditRecord,
    ) -> &AuditEntry {
        let action = record
            .event
            .strip_prefix("maintenance.")
            .unwrap_or(&record.event)
            .to_string();
        let summary = match record.event.as_str() {
            "maintenance.approval_nonce" => {
                format!("maintenance approval nonce {}", record.status)
            }
            "maintenance.execute" => format!("maintenance execute {}", record.status),
            _ => format!("{} {}", record.event, record.status),
        };
        let mut detail = json!({
            "event": record.event,
            "status": record.status,
        });

        if let Some(nonce) = &record.nonce {
            detail["nonce"] = json!({
                "redacted": nonce.redacted,
                "label": nonce.label,
                "status": nonce.status,
            });
        }
        if let Some(reason) = &record.reason {
            detail["reason"] = json!(reason);
        }

        self.append_entry(AuditEntryDraft {
            kind: "runner_maintenance".to_string(),
            action,
            created_at: created_at.into(),
            summary,
            detail: detail.to_string(),
            call_id: None,
            approval_id: None,
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_runner_browser_audit(
        &mut self,
        session_id: impl Into<String>,
        record: &RunnerBrowserAuditRecord,
    ) -> &AuditEntry {
        self.append_runner_browser_audit_at(now_created_at(), session_id, record)
    }

    pub fn append_runner_browser_audit_at(
        &mut self,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
        record: &RunnerBrowserAuditRecord,
    ) -> &AuditEntry {
        let action = record
            .action
            .clone()
            .unwrap_or_else(|| record.record_type.clone());
        let summary = match (&record.action, &record.reason) {
            (Some(action), Some(reason)) => format!("browser {action} {reason}"),
            (Some(action), None) => format!("browser {action}"),
            (None, Some(reason)) => format!("browser {} {reason}", record.record_type),
            (None, None) => format!("browser {}", record.record_type),
        };
        let mut detail = json!({
            "type": record.record_type,
            "tool": record.tool,
        });

        if let Some(browser_action) = &record.action {
            detail["action"] = json!(browser_action);
        }
        if let Some(browser_session_id) = &record.session_id {
            detail["session_id"] = json!(browser_session_id);
        }
        if let Some(selector) = &record.selector {
            detail["selector"] = json!(selector);
        }
        if let Some(reason) = &record.reason {
            detail["reason"] = json!(reason);
        }
        if let Some(risk) = &record.risk {
            detail["risk"] = json!(risk);
        }
        if let Some(approval_required) = record.approval_required {
            detail["approval_required"] = json!(approval_required);
        }
        if !record.metadata.is_null() {
            detail["metadata"] = record.metadata.clone();
        }

        self.append_entry(AuditEntryDraft {
            kind: "runner_browser".to_string(),
            action,
            created_at: created_at.into(),
            summary,
            detail: detail.to_string(),
            call_id: None,
            approval_id: None,
            session_id: Some(session_id.into()),
        })
    }

    pub fn append_runner_recent_entries<I>(&mut self, session_id: impl Into<String>, entries: I)
    where
        I: IntoIterator<Item = AuditEntry>,
    {
        let session_id = session_id.into();

        for entry in entries {
            self.append_entry(AuditEntryDraft {
                kind: entry.kind,
                action: entry.action,
                created_at: entry.created_at,
                summary: entry.summary,
                detail: sanitized_audit_detail(&entry.detail),
                call_id: entry.call_id,
                approval_id: entry.approval_id,
                session_id: Some(session_id.clone()),
            });
        }
    }

    fn append_entry(&mut self, draft: AuditEntryDraft) -> &AuditEntry {
        let seq = self
            .entries
            .iter()
            .map(|entry| entry.seq)
            .max()
            .unwrap_or(0)
            + 1;

        self.entries.push(AuditEntry {
            seq,
            kind: draft.kind,
            action: draft.action,
            created_at: draft.created_at,
            summary: draft.summary,
            detail: draft.detail,
            call_id: draft.call_id,
            approval_id: draft.approval_id,
            session_id: draft.session_id,
        });

        self.entries
            .last()
            .expect("append_entry just pushed an audit entry")
    }
}

struct AuditEntryDraft {
    kind: String,
    action: String,
    created_at: String,
    summary: String,
    detail: String,
    call_id: Option<String>,
    approval_id: Option<String>,
    session_id: Option<String>,
}

fn now_created_at() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn sanitized_audit_detail(detail: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(detail) else {
        return detail.to_string();
    };

    sanitize_sensitive_audit_value(&mut value);
    value.to_string()
}

fn sanitize_sensitive_audit_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, value| {
                if is_sensitive_audit_key(key) {
                    if key.eq_ignore_ascii_case("nonce") && value.is_object() {
                        sanitize_sensitive_audit_value(value);
                        return value.as_object().is_some_and(|object| !object.is_empty());
                    }

                    return false;
                }

                sanitize_sensitive_audit_value(value);
                true
            });
        }
        Value::Array(values) => {
            for value in values {
                sanitize_sensitive_audit_value(value);
            }
        }
        _ => {}
    }
}

fn is_sensitive_audit_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower == "authorization"
        || lower == "bearer_token"
        || lower == "raw"
        || lower == "raw_nonce"
        || lower == "token"
        || lower == "access_token"
        || lower == "refresh_token"
        || lower == "secret"
        || lower == "nonce"
}
