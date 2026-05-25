use std::time::{SystemTime, UNIX_EPOCH};

use deepseek_mobile_agent_core::risk::RiskAssessment;
use deepseek_mobile_agent_core::ssh::SshCommandRequest;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::ssh_exec::{CommandRunError, CommandRunner, SshCommandOutput};
use crate::{
    AppState, AuditEntry, DiagnosticPreset, DiagnosticRequest, DiagnosticResponse, ServerEvent,
};

#[derive(Clone, Debug)]
pub struct DiagnosticService<R> {
    runner: R,
}

impl<R> DiagnosticService<R>
where
    R: CommandRunner,
{
    #[must_use]
    pub const fn new(runner: R) -> Self {
        Self { runner }
    }

    pub fn run(
        &self,
        state: &AppState,
        request: DiagnosticRequest,
    ) -> Result<DiagnosticRun, DiagnosticError> {
        let command = preset_command(&request.diagnostic).ok_or_else(|| {
            DiagnosticError::UnknownDiagnostic {
                diagnostic: request.diagnostic.clone(),
            }
        })?;
        let target = state.ssh_target();
        let command_request = SshCommandRequest {
            command: command.to_string(),
            cwd: None,
            timeout_ms: None,
            risk: RiskAssessment::low("read-only diagnostic command"),
        };
        let call_id = format!("diagnostic-{}", Uuid::new_v4());

        let mut events = vec![tool_event(
            "tool.started",
            &request,
            command,
            json!({
                "call_id": call_id,
                "tool": "remote.shell.exec",
                "requires_approval": false,
            }),
        )];
        state.broadcast(events[0].clone());

        let output = self
            .runner
            .run(&target, &command_request)
            .map_err(|source| DiagnosticError::Command {
                diagnostic: request.diagnostic.clone(),
                source,
            })?;

        if !output.stdout.is_empty() {
            let event = tool_event(
                "tool.stdout",
                &request,
                command,
                json!({"stdout": output.stdout}),
            );
            state.broadcast(event.clone());
            events.push(event);
        }

        if !output.stderr.is_empty() {
            let event = tool_event(
                "tool.stderr",
                &request,
                command,
                json!({"stderr": output.stderr}),
            );
            state.broadcast(event.clone());
            events.push(event);
        }

        let duration_ms = duration_ms(output.duration);
        let status = if output.timed_out {
            "timed_out"
        } else {
            "completed"
        };
        let result = command_result(command, &output, duration_ms);
        let completed = tool_event(
            if output.timed_out {
                "tool.failed"
            } else {
                "tool.completed"
            },
            &request,
            command,
            json!({
                "exit_code": output.exit_code,
                "duration_ms": duration_ms,
                "timed_out": output.timed_out,
                "status": status,
            }),
        );
        state.broadcast(completed.clone());
        events.push(completed);

        let response = DiagnosticResponse {
            session_id: request.session_id.clone(),
            diagnostic: request.diagnostic.clone(),
            status: status.to_string(),
            result: result.clone(),
        };
        let audit = audit_entry(&request, status, command, &result);
        state.push_audit(audit.clone());

        Ok(DiagnosticRun {
            response,
            events,
            audit,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticRun {
    pub response: DiagnosticResponse,
    pub events: Vec<ServerEvent>,
    pub audit: AuditEntry,
}

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticError {
    #[error("unknown diagnostic preset: {diagnostic}")]
    UnknownDiagnostic { diagnostic: String },
    #[error("failed to run diagnostic preset {diagnostic}: {source}")]
    Command {
        diagnostic: String,
        source: CommandRunError,
    },
}

#[must_use]
pub fn preset_command(diagnostic: &str) -> Option<&'static str> {
    match diagnostic {
        "system_info" => Some("uname -a"),
        "current_user" => Some("id"),
        "disk_usage" => Some("df -h"),
        "memory" => Some("free -m || cat /proc/meminfo"),
        "network" => Some("ip addr || ifconfig"),
        "working_directory" => Some("pwd"),
        _ => None,
    }
}

#[must_use]
pub fn preset_diagnostics() -> Vec<DiagnosticPreset> {
    [
        ("system_info", "System info"),
        ("current_user", "Current user"),
        ("disk_usage", "Disk usage"),
        ("memory", "Memory"),
        ("network", "Network"),
        ("working_directory", "Working directory"),
    ]
    .into_iter()
    .map(|(key, label)| DiagnosticPreset {
        key: key.to_string(),
        label: label.to_string(),
        command: preset_command(key)
            .expect("diagnostic preset metadata must match preset command mapping")
            .to_string(),
        requires_approval: false,
    })
    .collect()
}

fn command_result(command: &str, output: &SshCommandOutput, duration_ms: u64) -> Value {
    json!({
        "command": command,
        "requires_approval": false,
        "stdout": output.stdout,
        "stderr": output.stderr,
        "exit_code": output.exit_code,
        "duration_ms": duration_ms,
        "timed_out": output.timed_out,
    })
}

fn tool_event(
    event_type: &str,
    request: &DiagnosticRequest,
    command: &str,
    extra: Value,
) -> ServerEvent {
    let mut payload = json!({
        "session_id": request.session_id,
        "diagnostic": request.diagnostic,
        "command": command,
    });
    merge_json(&mut payload, extra);

    ServerEvent {
        event_type: event_type.to_string(),
        payload,
    }
}

fn audit_entry(
    request: &DiagnosticRequest,
    status: &str,
    command: &str,
    result: &Value,
) -> AuditEntry {
    AuditEntry {
        id: format!("audit-{}", Uuid::new_v4()),
        session_id: Some(request.session_id.clone()),
        kind: format!("diagnostic.{status}"),
        created_at_ms: now_ms(),
        summary: format!("diagnostic {} {status}", request.diagnostic),
        metadata: redact_value(&json!({
            "diagnostic": request.diagnostic,
            "command": command,
            "requires_approval": false,
            "stdout": result["stdout"],
            "stderr": result["stderr"],
            "exit_code": result["exit_code"],
            "duration_ms": result["duration_ms"],
            "timed_out": result["timed_out"],
        })),
    }
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_text(text)),
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if is_secret_key(key) {
                        (key.clone(), Value::String("[REDACTED]".to_string()))
                    } else {
                        (key.clone(), redact_value(value))
                    }
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn redact_text(text: &str) -> String {
    let mut redacted = text.lines().map(redact_line).collect::<Vec<_>>().join("\n");
    if text.ends_with('\n') {
        redacted.push('\n');
    }
    redacted
}

fn redact_line(line: &str) -> String {
    for separator in ['=', ':'] {
        if let Some((key, _value)) = line.split_once(separator)
            && is_secret_key(key.trim())
        {
            return format!("{}{}[REDACTED]", key, separator);
        }
    }
    line.to_string()
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "nonce", "secret", "bearer", "lease", "idempotency"]
        .iter()
        .any(|needle| key.contains(needle))
}

fn merge_json(target: &mut Value, extra: Value) {
    if let (Some(target), Some(extra)) = (target.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(duration_ms)
        .unwrap_or_default()
}
