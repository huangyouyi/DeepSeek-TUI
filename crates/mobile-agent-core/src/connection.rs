use crate::audit::{AuditEntry, AuditLog};
use crate::capabilities::{CapabilitySet, ExecutionMode};
use crate::event::{MobileEvent, MobileEventKind};
use crate::remote_schema::RemoteToolName;
use serde_json::{Value, json};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Unreachable,
    ManualBootstrap,
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionProfile {
    pub id: String,
    pub label: String,
    pub status: ConnectionStatus,
    pub capabilities: CapabilitySet,
    pub runner_metadata: Option<RunnerProfileMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerUpgradeError;

impl fmt::Display for RunnerUpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("runner capability report did not include any recognized tools")
    }
}

impl std::error::Error for RunnerUpgradeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerProfileMetadata {
    pub endpoint: String,
    pub pairing_token_present: bool,
    pub pairing_token_label: String,
}

impl RunnerProfileMetadata {
    #[must_use]
    pub fn new(endpoint: impl Into<String>, pairing_token_present: bool) -> Self {
        Self {
            endpoint: normalized_runner_endpoint(endpoint),
            pairing_token_present,
            pairing_token_label: "<redacted>".to_string(),
        }
    }

    #[must_use]
    pub fn as_json(&self) -> Value {
        json!({
            "endpoint": self.endpoint,
            "pairing_token_present": self.pairing_token_present,
            "pairing_token_label": self.pairing_token_label,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerPairingBootstrapInput {
    pub endpoint: String,
    pub pairing_token_present: bool,
    pub capabilities_report: String,
}

impl RunnerPairingBootstrapInput {
    #[must_use]
    pub fn new(
        endpoint: impl Into<String>,
        pairing_token: impl AsRef<str>,
        capabilities_report: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: normalized_runner_endpoint(endpoint),
            pairing_token_present: !pairing_token.as_ref().trim().is_empty(),
            capabilities_report: capabilities_report.into(),
        }
    }

    #[must_use]
    pub fn metadata(&self) -> RunnerProfileMetadata {
        RunnerProfileMetadata::new(self.endpoint.clone(), self.pairing_token_present)
    }
}

impl ConnectionProfile {
    #[must_use]
    pub fn bootstrap(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: ConnectionStatus::ManualBootstrap,
            capabilities: CapabilitySet::bootstrap(),
            runner_metadata: None,
        }
    }

    #[must_use]
    pub fn unreachable(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: ConnectionStatus::Unreachable,
            capabilities: CapabilitySet::bootstrap(),
            runner_metadata: None,
        }
    }

    #[must_use]
    pub fn ssh(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::connected(id, label, CapabilitySet::ssh())
    }

    #[must_use]
    pub fn powershell(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::connected(id, label, CapabilitySet::powershell())
    }

    #[must_use]
    pub fn runner(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::connected(id, label, CapabilitySet::runner())
    }

    #[must_use]
    pub fn remote_mcp(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::connected(id, label, CapabilitySet::remote_mcp())
    }

    fn connected(
        id: impl Into<String>,
        label: impl Into<String>,
        capabilities: CapabilitySet,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            status: ConnectionStatus::Connected,
            capabilities,
            runner_metadata: None,
        }
    }

    #[must_use]
    pub fn mode(&self) -> ExecutionMode {
        self.capabilities.mode
    }

    #[must_use]
    pub fn allows_tool(&self, tool: RemoteToolName) -> bool {
        self.status != ConnectionStatus::Unreachable && self.capabilities.allows(tool)
    }

    #[must_use]
    pub fn with_runner_capabilities(mut self, capabilities: CapabilitySet) -> Self {
        self.status = ConnectionStatus::Connected;
        self.capabilities = capabilities;
        self.runner_metadata = None;
        self
    }

    #[must_use]
    pub fn with_runner_metadata(mut self, metadata: RunnerProfileMetadata) -> Self {
        self.runner_metadata = Some(metadata);
        self
    }

    pub fn upgrade_to_runner_from_report<I, S>(
        &self,
        tool_names: I,
    ) -> Result<Self, RunnerUpgradeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let capabilities = CapabilitySet::from_runner_reported_tools(tool_names);
        if capabilities.tools.is_empty() {
            return Err(RunnerUpgradeError);
        }

        Ok(Self {
            id: self.id.clone(),
            label: self.label.clone(),
            status: ConnectionStatus::Connected,
            capabilities,
            runner_metadata: None,
        })
    }

    pub fn upgrade_to_runner_from_pasted_report(
        &self,
        report: &str,
    ) -> Result<Self, RunnerUpgradeError> {
        self.upgrade_to_runner_from_report(extract_reported_tool_names(report))
    }

    pub fn upgrade_to_runner_from_pairing_bootstrap(
        &self,
        input: RunnerPairingBootstrapInput,
    ) -> Result<Self, RunnerUpgradeError> {
        let mut upgraded = self.upgrade_to_runner_from_pasted_report(&input.capabilities_report)?;
        upgraded.runner_metadata = Some(input.metadata());
        Ok(upgraded)
    }

    #[must_use]
    pub fn runner_upgrade_event(&self, seq: u64, session_id: impl Into<String>) -> MobileEvent {
        MobileEvent {
            seq,
            kind: MobileEventKind::BootstrapStepCreated,
            payload: json!({
                "action": "runner_profile_upgraded",
                "session_id": session_id.into(),
                "connection_id": self.id,
                "label": self.label,
                "status": "connected",
                "mode": "runner",
                "runner": self.runner_metadata_json(),
                "capabilities": self.capabilities_json(),
            }),
        }
    }

    pub fn append_runner_upgrade_audit_at(
        &self,
        audit: &mut AuditLog,
        created_at: impl Into<String>,
        session_id: impl Into<String>,
    ) {
        let session_id = session_id.into();
        audit.append_runner_recent_entries(
            session_id.clone(),
            [AuditEntry {
                seq: 0,
                kind: "connection".to_string(),
                action: "runner_profile_upgraded".to_string(),
                created_at: created_at.into(),
                summary: format!("runner profile upgraded for {}", self.label),
                detail: json!({
                    "connection_id": self.id,
                    "label": self.label,
                    "status": "connected",
                    "mode": "runner",
                    "runner": self.runner_metadata_json(),
                    "capabilities": self.capabilities_json(),
                })
                .to_string(),
                call_id: None,
                approval_id: None,
                session_id: Some(session_id),
            }],
        );
    }

    #[must_use]
    fn runner_metadata_json(&self) -> Value {
        self.runner_metadata
            .as_ref()
            .map(RunnerProfileMetadata::as_json)
            .unwrap_or(Value::Null)
    }

    #[must_use]
    fn capabilities_json(&self) -> Value {
        json!({
            "mode": "runner",
            "tools": self
                .capabilities
                .tools
                .iter()
                .map(|tool| tool.as_str())
                .collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn next_step_suggestion(&self) -> &'static str {
        match self.status {
            ConnectionStatus::Unreachable => "Bootstrap the device to establish a connection.",
            ConnectionStatus::ManualBootstrap => {
                "Ask the user to manually execute the bootstrap step and paste the output."
            }
            ConnectionStatus::Connected => "Remote execution is available for this connection.",
        }
    }
}

fn normalized_runner_endpoint(endpoint: impl Into<String>) -> String {
    endpoint.into().trim().trim_end_matches('/').to_string()
}

fn extract_reported_tool_names(report: &str) -> Vec<String> {
    if let Some(tools) = extract_json_reported_tool_names(report) {
        return tools;
    }

    report
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')))
        .filter(|candidate| RemoteToolName::parse(candidate).is_ok())
        .map(str::to_string)
        .collect()
}

fn extract_json_reported_tool_names(report: &str) -> Option<Vec<String>> {
    let start = report.find('{')?;
    let end = report.rfind('}')?;
    if end < start {
        return None;
    }

    let json = serde_json::from_str::<Value>(&report[start..=end]).ok()?;
    let mut tools = Vec::new();
    collect_tool_names(&json, &mut tools);
    Some(tools)
}

fn collect_tool_names(value: &Value, tools: &mut Vec<String>) {
    match value {
        Value::String(candidate) if RemoteToolName::parse(candidate).is_ok() => {
            tools.push(candidate.clone());
        }
        Value::Array(values) => {
            for value in values {
                collect_tool_names(value, tools);
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                collect_tool_names(value, tools);
            }
        }
        _ => {}
    }
}
