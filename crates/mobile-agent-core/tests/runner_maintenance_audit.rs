use deepseek_mobile_agent_core::audit::{
    AuditLog, RunnerMaintenanceAuditNonce, RunnerMaintenanceAuditRecord,
};
use deepseek_mobile_agent_core::transport::RunnerHttpTransport;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn parses_consumed_nonce_and_dry_run_no_op_maintenance_audit_without_network() {
    let raw_nonce = "approval-nonce-secret";
    let bearer_token = "runner-bearer-secret";
    let parsed = RunnerHttpTransport::parse_maintenance_response(json!({
        "action": "self_update",
        "dry_run": true,
        "audit": [
            {
                "event": "maintenance.approval_nonce",
                "status": "consumed",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "consumed",
                    "raw": raw_nonce
                },
                "authorization": format!("Bearer {bearer_token}")
            },
            {
                "event": "maintenance.execute",
                "status": "no_op",
                "reason": "dry_run"
            }
        ]
    }))
    .expect("maintenance response should parse");

    assert_eq!(parsed.body["action"], "self_update");
    assert!(parsed.body.get("audit").is_none());
    assert_eq!(
        parsed.audit,
        vec![
            RunnerMaintenanceAuditRecord {
                event: "maintenance.approval_nonce".to_string(),
                status: "consumed".to_string(),
                nonce: Some(RunnerMaintenanceAuditNonce {
                    redacted: None,
                    label: "approval_nonce".to_string(),
                    status: "consumed".to_string(),
                }),
                reason: None,
            },
            RunnerMaintenanceAuditRecord {
                event: "maintenance.execute".to_string(),
                status: "no_op".to_string(),
                nonce: None,
                reason: Some("dry_run".to_string()),
            },
        ]
    );

    let mut audit = AuditLog::default();
    for record in &parsed.audit {
        audit.append_runner_maintenance_audit_at("2026-05-24T10:00:00Z", "session-1", record);
    }
    let rendered = serde_json::to_string(&audit).expect("audit log should serialize");

    assert!(rendered.contains("approval_nonce"));
    assert!(rendered.contains("consumed"));
    assert!(rendered.contains("dry_run"));
    assert!(!rendered.contains(raw_nonce));
    assert!(!rendered.contains(bearer_token));
}

#[test]
fn parses_replayed_nonce_maintenance_audit_without_leaking_raw_fields() {
    let raw_nonce = "approval-nonce-replayed-secret";
    let parsed = RunnerHttpTransport::parse_maintenance_response(json!({
        "error": {
            "code": "approval_replayed",
            "message": "approval was already used"
        },
        "audit": [
            {
                "event": "maintenance.approval_nonce",
                "status": "replay",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "replayed",
                    "value": raw_nonce
                }
            }
        ]
    }))
    .expect("replay response should parse");

    assert_eq!(
        parsed.audit,
        vec![RunnerMaintenanceAuditRecord {
            event: "maintenance.approval_nonce".to_string(),
            status: "replay".to_string(),
            nonce: Some(RunnerMaintenanceAuditNonce {
                redacted: None,
                label: "approval_nonce".to_string(),
                status: "replayed".to_string(),
            }),
            reason: None,
        }]
    );

    let mut audit = AuditLog::default();
    audit.append_runner_maintenance_audit_at("2026-05-24T10:00:00Z", "session-1", &parsed.audit[0]);
    let rendered = serde_json::to_string(&audit).expect("audit log should serialize");

    assert!(rendered.contains("replay"));
    assert!(rendered.contains("replayed"));
    assert!(!rendered.contains(raw_nonce));
}

#[test]
fn parses_execute_attempt_no_op_maintenance_audit_without_network() {
    let parsed = RunnerHttpTransport::parse_maintenance_response(json!({
        "error": {
            "code": "not_executable",
            "message": "maintenance execution is disabled"
        },
        "approval": {
            "status": "consumed"
        },
        "audit": [
            {
                "event": "maintenance.approval_nonce",
                "status": "consumed",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "consumed",
                    "redacted": true
                }
            },
            {
                "event": "maintenance.execute",
                "status": "no_op",
                "reason": "not_executable"
            }
        ]
    }))
    .expect("execute no-op response should parse");

    assert_eq!(parsed.audit[0].status, "consumed");
    assert_eq!(parsed.audit[0].nonce.as_ref().unwrap().redacted, Some(true));
    assert_eq!(parsed.audit[1].event, "maintenance.execute");
    assert_eq!(parsed.audit[1].status, "no_op");
    assert_eq!(parsed.audit[1].reason.as_deref(), Some("not_executable"));

    let mut audit = AuditLog::default();
    for record in &parsed.audit {
        audit.append_runner_maintenance_audit_at("2026-05-24T10:00:00Z", "session-1", record);
    }
    let entries = audit.entries();

    assert_eq!(entries[0].kind, "runner_maintenance");
    assert_eq!(entries[0].action, "approval_nonce");
    assert_eq!(entries[1].kind, "runner_maintenance");
    assert_eq!(entries[1].action, "execute");
    assert!(entries[1].detail.contains("not_executable"));
}

#[test]
fn rejects_non_array_maintenance_audit() {
    let err = RunnerHttpTransport::parse_maintenance_response(json!({
        "action": "self_update",
        "audit": {
            "event": "maintenance.execute",
            "status": "no_op"
        }
    }))
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "remote tool failed: runner maintenance response was invalid: audit must be an array"
    );
}
