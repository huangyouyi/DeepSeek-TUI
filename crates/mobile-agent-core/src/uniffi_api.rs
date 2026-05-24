//! Swift-facing facade placeholder.
//!
//! The crate is already built as `staticlib`/`cdylib` so iOS integration can
//! link the Rust core. UniFFI scaffolding should stay thin and call into the
//! pure Rust modules, which are tested on Linux in this repository.

use crate::{
    AuditLog, BootstrapSession, BootstrapStep, CapabilitySet, ConnectionProfile, ConnectionStatus,
    ExecutionMode, SessionSnapshot, session::MobileAgentCore, transport::RunnerHttpTransport,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosConnectionProfile {
    pub id: String,
    pub label: String,
    pub status: String,
    pub mode: String,
    pub tool_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosBootstrapStep {
    pub step_id: String,
    pub instruction: String,
    pub command: String,
    pub expected_output: String,
    pub approval_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosMobileEvent {
    pub seq: u64,
    pub kind: String,
    pub payload_json: String,
}

#[derive(Debug, Default)]
pub struct IosMobileAgentCore {
    inner: MobileAgentCore,
}

impl IosMobileAgentCore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&mut self, title: String) -> String {
        self.inner.create_session(title)
    }

    pub fn homebrew_bootstrap_step(
        &mut self,
        session_id: String,
    ) -> Result<IosBootstrapStep, String> {
        self.inner
            .homebrew_bootstrap_step(&session_id)
            .map_err(|error| error.to_string())?;
        let session = self
            .inner
            .session(&session_id)
            .map_err(|error| error.to_string())?;
        let step = session
            .bootstrap
            .steps
            .last()
            .ok_or_else(|| "bootstrap step not found after creation".to_string())?;
        Ok(IosBootstrapStep {
            step_id: step.step_id.clone(),
            instruction: step.instruction.clone(),
            command: step.command.clone(),
            expected_output: step.expected_output.clone(),
            approval_required: step.approval_required,
        })
    }

    pub fn submit_bootstrap_output(
        &mut self,
        session_id: String,
        step_id: String,
        output: String,
    ) -> Result<IosMobileEvent, String> {
        let event = self
            .inner
            .submit_bootstrap_output(&session_id, &step_id, output)
            .map_err(|error| error.to_string())?;
        let kind = event
            .payload
            .get("action")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("mobile_event")
            .to_string();
        Ok(IosMobileEvent {
            seq: event.seq,
            kind,
            payload_json: event.payload.to_string(),
        })
    }

    pub fn session_snapshot_json(&self, session_id: String) -> Result<String, String> {
        let session = self
            .inner
            .session(&session_id)
            .map_err(|error| error.to_string())?;
        serde_json::to_string(&SessionSnapshot::from_session(session))
            .map_err(|error| error.to_string())
    }

    pub fn bootstrap_install_steps_json(&self) -> Result<String, String> {
        let mut bootstrap = BootstrapSession::default();
        let macos = bootstrap.macos_runner_install_step();
        let windows = bootstrap.windows_runner_install_step();

        serde_json::to_string(&json!({
            "steps": [
                bootstrap_step_json("macos", &macos),
                bootstrap_step_json("windows", &windows),
            ],
        }))
        .map_err(|error| error.to_string())
    }

    pub fn runner_maintenance_audit_json(
        &self,
        session_id: String,
        response_json: String,
    ) -> Result<String, String> {
        let response = serde_json::from_str::<Value>(&response_json)
            .map_err(|error| format!("maintenance response JSON was invalid: {error}"))?;
        let parsed = RunnerHttpTransport::parse_maintenance_response(response)
            .map_err(|error| error.to_string())?;

        let mut audit = AuditLog::default();
        for record in &parsed.audit {
            audit.append_runner_maintenance_audit(&session_id, record);
        }

        let mut body = parsed.body;
        sanitize_sensitive_json(&mut body);
        let mut records = serde_json::to_value(&parsed.audit).map_err(|error| error.to_string())?;
        sanitize_sensitive_json(&mut records);

        serde_json::to_string(&json!({
            "body": body,
            "audit_records": records,
            "audit_entries": audit.entries(),
        }))
        .map_err(|error| error.to_string())
    }

    pub fn runner_recent_audit_json(
        &self,
        session_id: String,
        response_json: String,
    ) -> Result<String, String> {
        let response = serde_json::from_str::<Value>(&response_json)
            .map_err(|error| format!("runner audit/recent JSON was invalid: {error}"))?;
        let mut audit = AuditLog::default();
        audit.append_runner_recent_audit_body(session_id, &response)?;
        let mut audit_entries =
            serde_json::to_value(audit.entries()).map_err(|error| error.to_string())?;
        normalize_ios_runner_command_lease_entries(&mut audit_entries);

        serde_json::to_string(&json!({
            "audit_entries": audit_entries,
        }))
        .map_err(|error| error.to_string())
    }

    pub fn runner_maintenance_plan_json(
        &self,
        session_id: String,
        response_json: String,
    ) -> Result<String, String> {
        let mut body = serde_json::from_str::<Value>(&response_json)
            .map_err(|error| format!("maintenance plan JSON was invalid: {error}"))?;
        sanitize_sensitive_json(&mut body);

        let action = string_field(&body, "action").unwrap_or_else(|| "self_update".to_string());
        let operation = match normalize_core_enum(&action).as_str() {
            "uninstall" => "uninstall",
            _ => "self_update",
        };
        let title = string_field(&body, "title").unwrap_or_else(|| match operation {
            "uninstall" => "Runner uninstall dry-run".to_string(),
            _ => "Runner self-update dry-run".to_string(),
        });
        let connection_id_label = string_field(&body, "connection_id_label")
            .unwrap_or_else(|| "paired runner".to_string());
        let requested_by = string_field(&body, "requested_by")
            .unwrap_or_else(|| "runner maintenance plan".to_string());
        let target_summary = nested_string_field(&body, &["target", "summary"])
            .or_else(|| string_field(&body, "target_summary"))
            .unwrap_or_else(|| match operation {
                "uninstall" => "Remove paired runner service and local support files".to_string(),
                _ => "Update paired runner artifact".to_string(),
            });
        let risk = nested_string_field(&body, &["risk", "level"])
            .or_else(|| string_field(&body, "risk"))
            .unwrap_or_else(|| "high".to_string());
        let approval_status = if body.get("dry_run").and_then(Value::as_bool).unwrap_or(true) {
            "waiting_for_dry_run_approval"
        } else {
            "planned"
        };
        let dry_run_steps = maintenance_steps_json(&body);
        let artifact_verification = artifact_verification_json(&body);
        let audit_summary = format!(
            "{} dry-run planned; artifact verification is visible; no download, install, or rollback was executed.",
            title
        );

        serde_json::to_string(&json!({
            "requests": [{
                "id": Uuid::new_v4().to_string(),
                "app_session_id": session_id,
                "operation": operation,
                "title": safe_display_string(title),
                "connection_id_label": safe_display_string(connection_id_label),
                "target_summary": safe_display_string(target_summary),
                "risk": safe_display_string(risk),
                "approval_status": approval_status,
                "dry_run_steps": dry_run_steps,
                "artifact_verification": artifact_verification,
                "audit_summary": safe_display_string(audit_summary),
                "requested_by": safe_display_string(requested_by),
            }]
        }))
        .map_err(|error| error.to_string())
    }

    pub fn browser_audit_json(
        &self,
        session_id: String,
        response_json: String,
    ) -> Result<String, String> {
        let response = serde_json::from_str::<Value>(&response_json)
            .map_err(|error| format!("browser response JSON was invalid: {error}"))?;
        let parsed = RunnerHttpTransport::parse_tool_call_response_with_browser_audit(response)
            .map_err(|error| error.to_string())?;

        let mut audit = AuditLog::default();
        for record in &parsed.audit {
            audit.append_runner_browser_audit(&session_id, record);
        }

        let mut output = serde_json::to_value(&parsed.output).map_err(|error| error.to_string())?;
        let mut records = serde_json::to_value(&parsed.audit).map_err(|error| error.to_string())?;
        sanitize_sensitive_json(&mut output);
        sanitize_sensitive_json(&mut records);

        serde_json::to_string(&json!({
            "output": output,
            "audit_records": records,
            "audit_entries": audit.entries(),
        }))
        .map_err(|error| error.to_string())
    }

    pub fn capability_report_upgrade_json(
        &self,
        id: String,
        label: String,
        report: String,
    ) -> Result<String, String> {
        let profile = ConnectionProfile::bootstrap(id, label)
            .upgrade_to_runner_from_pasted_report(&report)
            .map_err(|error| error.to_string())?;
        let action = if profile.mode() == ExecutionMode::Bootstrap {
            "show_bootstrap_instructions"
        } else {
            "start_remote_agent_loop"
        };

        serde_json::to_string(&json!({
            "action": action,
            "profile": connection_profile_json(&profile),
            "next_step": profile.next_step_suggestion(),
        }))
        .map_err(|error| error.to_string())
    }

    #[must_use]
    pub fn connection_bootstrap(&self, id: String, label: String) -> IosConnectionProfile {
        IosConnectionProfile::from(ConnectionProfile::bootstrap(id, label))
    }

    #[must_use]
    pub fn connection_runner_from_tools(
        &self,
        id: String,
        label: String,
        tool_names: Vec<String>,
    ) -> IosConnectionProfile {
        let capabilities = CapabilitySet::from_runner_reported_tools(tool_names);
        IosConnectionProfile::from(
            ConnectionProfile::bootstrap(id, label).with_runner_capabilities(capabilities),
        )
    }

    #[must_use]
    pub fn connection_next_step(&self, profile: IosConnectionProfile) -> String {
        let action = if profile.mode == "bootstrap" {
            "show_bootstrap_instructions"
        } else {
            "start_remote_agent_loop"
        };

        serde_json::json!({
            "action": action,
            "connection_id": profile.id,
            "label": profile.label,
            "status": profile.status,
            "mode": profile.mode,
            "tool_names": profile.tool_names,
        })
        .to_string()
    }
}

fn bootstrap_step_json(platform: &str, step: &BootstrapStep) -> Value {
    json!({
        "platform": platform,
        "step_id": step.step_id,
        "title": step.title,
        "commands": step.commands,
        "instruction": step.instruction,
        "command": step.command,
        "explanation": step.explanation,
        "risk": step.risk,
        "expected_output": step.expected_output,
        "approval_required": step.approval_required,
    })
}

fn connection_profile_json(profile: &ConnectionProfile) -> Value {
    json!({
        "id": profile.id,
        "label": profile.label,
        "status": status_name(&profile.status),
        "mode": mode_name(profile.mode()),
        "tool_names": profile
            .capabilities
            .tools
            .iter()
            .map(|tool| tool.as_str())
            .collect::<Vec<_>>(),
    })
}

fn sanitize_sensitive_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| !is_sensitive_json_key(key));
            for value in object.values_mut() {
                sanitize_sensitive_json(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                sanitize_sensitive_json(value);
            }
        }
        _ => {}
    }
}

fn is_sensitive_json_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization" | "nonce" | "raw_nonce" | "raw" | "bearer_token" | "token"
    )
}

fn normalize_ios_runner_command_lease_entries(entries: &mut Value) {
    let Some(entries) = entries.as_array_mut() else {
        return;
    };

    for entry in entries {
        let Some(object) = entry.as_object_mut() else {
            continue;
        };
        if object.get("kind").and_then(Value::as_str) != Some("runner_command_lease") {
            continue;
        }

        let action = object
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let call_id = object.get("call_id").and_then(Value::as_str);
        let detail_json = object
            .get("detail")
            .and_then(Value::as_str)
            .and_then(|detail| serde_json::from_str::<Value>(detail).ok())
            .unwrap_or(Value::Null);
        let tool = detail_json.get("tool").and_then(Value::as_str);
        let error_code = detail_json.get("error_code").and_then(Value::as_str);

        object.insert(
            "detail".to_string(),
            Value::String(ios_command_lease_detail(action, call_id, tool, error_code)),
        );
    }
}

fn ios_command_lease_detail(
    action: &str,
    call_id: Option<&str>,
    tool: Option<&str>,
    error_code: Option<&str>,
) -> String {
    let mut detail = match action {
        "accepted" => "lease accepted".to_string(),
        "consumed" => "lease consumed".to_string(),
        "replay_rejected" => "lease replay rejected".to_string(),
        "expired_rejected" => "lease expired rejected".to_string(),
        "invalid_action_rejected" => "lease invalid rejected".to_string(),
        other => format!("lease {other}"),
    };

    if let Some(call_id) = call_id {
        detail.push_str(" call_id=");
        detail.push_str(call_id);
    }
    if let Some(tool) = tool {
        detail.push_str(" tool=");
        detail.push_str(tool);
    }
    if let Some(error_code) = error_code {
        detail.push_str(" error_code=");
        detail.push_str(error_code);
    }

    detail
}

fn maintenance_steps_json(body: &Value) -> Vec<Value> {
    body.get("steps")
        .and_then(Value::as_array)
        .map(|steps| {
            steps
                .iter()
                .enumerate()
                .filter_map(|(index, step)| {
                    let text = step.as_str()?;
                    Some(json!({
                        "id": Uuid::new_v4().to_string(),
                        "title": safe_display_string(text),
                        "detail": safe_display_string(maintenance_step_detail(text)),
                        "destructive": is_destructive_step(index, text),
                    }))
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![json!({
                "id": Uuid::new_v4().to_string(),
                "title": "Review runner maintenance plan",
                "detail": "No execution is performed by the mobile bridge.",
                "destructive": false,
            })]
        })
}

fn artifact_verification_json(body: &Value) -> Value {
    json!({
        "artifact_name": nested_string_field(body, &["artifact", "name"])
            .map(safe_display_string)
            .unwrap_or_else(|| "Runner artifact named by release manifest".to_string()),
        "checksum_source": nested_string_field(body, &["artifact", "checksum", "source"])
            .or_else(|| nested_string_field(body, &["checksum", "source"]))
            .map(safe_display_string)
            .unwrap_or_else(|| "Release manifest checksum".to_string()),
        "signature_source": nested_string_field(body, &["artifact", "signature", "source"])
            .or_else(|| nested_string_field(body, &["signature", "source"]))
            .map(safe_display_string)
            .unwrap_or_else(|| "Release signature metadata".to_string()),
        "verify_step": nested_string_field(body, &["verification", "step"])
            .or_else(|| string_field(body, "verify_step"))
            .map(safe_display_string)
            .unwrap_or_else(|| "Verify checksum and signature before any staged replacement.".to_string()),
        "rollback_guidance": nested_string_field(body, &["rollback", "guidance"])
            .or_else(|| string_field(body, "rollback_guidance"))
            .map(safe_display_string)
            .unwrap_or_else(|| "Keep the current runner in place unless a later execution approval succeeds.".to_string()),
    })
}

fn maintenance_step_detail(step: &str) -> String {
    let normalized = step.to_ascii_lowercase();
    if normalized.contains("download") {
        "Runner reported this as a dry-run stage; mobile does not download artifacts.".to_string()
    } else if normalized.contains("verify") || normalized.contains("signature") {
        "Verify checksum and signature metadata before any file changes.".to_string()
    } else if normalized.contains("replace")
        || normalized.contains("remove")
        || normalized.contains("stop")
    {
        "Execution requires a later high-risk approval; this plan does not run it.".to_string()
    } else {
        "Runner reported this dry-run step without executing it.".to_string()
    }
}

fn is_destructive_step(index: usize, step: &str) -> bool {
    let normalized = step.to_ascii_lowercase();
    index > 0
        && (normalized.contains("replace")
            || normalized.contains("remove")
            || normalized.contains("stop")
            || normalized.contains("delete")
            || normalized.contains("uninstall"))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn nested_string_field(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(str::to_string)
}

fn safe_display_string(value: impl Into<String>) -> String {
    let value = value.into();
    let lower = value.to_ascii_lowercase();
    if lower.contains("authorization")
        || lower.contains("bearer ")
        || lower.contains("token=")
        || lower.contains("token:")
        || lower.contains("secret")
        || lower.contains("nonce")
    {
        return "redacted by mobile core".to_string();
    }
    value
}

fn normalize_core_enum(value: &str) -> String {
    value
        .chars()
        .filter(|ch| *ch != '-' && *ch != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

impl From<ConnectionProfile> for IosConnectionProfile {
    fn from(profile: ConnectionProfile) -> Self {
        let status = status_name(&profile.status).to_string();
        let mode = mode_name(profile.mode()).to_string();
        let tool_names = profile
            .capabilities
            .tools
            .into_iter()
            .map(|tool| tool.as_str().to_string())
            .collect();

        Self {
            id: profile.id,
            label: profile.label,
            status,
            mode,
            tool_names,
        }
    }
}

fn status_name(status: &ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Unreachable => "unreachable",
        ConnectionStatus::ManualBootstrap => "manual_bootstrap",
        ConnectionStatus::Connected => "connected",
    }
}

fn mode_name(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Bootstrap => "bootstrap",
        ExecutionMode::Ssh => "ssh",
        ExecutionMode::PowerShell => "powershell",
        ExecutionMode::Runner => "runner",
        ExecutionMode::RemoteMcp => "remote_mcp",
    }
}
