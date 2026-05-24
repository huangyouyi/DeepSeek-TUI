use std::collections::VecDeque;

use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, RunnerMaintenanceApproval, RunnerMaintenancePlanRequest, TransportError,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[test]
fn runner_maintenance_plans_self_update_and_uninstall_dry_runs_without_network() {
    let pairing_token = "runner-maintenance-secret";
    let mut captured_requests: Vec<RunnerHttpRequestSpec> = Vec::new();
    let mut responses = VecDeque::from([
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "maintenance_plan",
            "action": "self_update",
            "dry_run": true,
            "planned": true,
            "risk": {
                "level": "high",
                "approval_required": true,
                "reason": "self-update can replace the active runner binary"
            },
            "steps": [
                "download signed runner artifact",
                "verify artifact signature",
                "replace runner after approval"
            ]
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "maintenance_plan",
            "action": "uninstall",
            "dry_run": true,
            "planned": true,
            "risk": {
                "level": "high",
                "approval_required": true,
                "reason": "uninstall removes the rescue control plane from the target computer"
            },
            "steps": [
                "stop runner service",
                "remove runner binary",
                "remove pairing credentials"
            ]
        })),
    ]);
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", pairing_token);
    let backend = RunnerHttpClosureBackend::new(|request: RunnerHttpRequestSpec| {
        captured_requests.push(request);
        responses
            .pop_front()
            .ok_or_else(|| TransportError::Failed("unexpected maintenance request".to_string()))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    assert!(!format!("{client:?}").contains(pairing_token));

    let self_update = client.plan_maintenance("self_update", true).unwrap();
    let uninstall = client.plan_maintenance("uninstall", true).unwrap();
    drop(client);

    assert_eq!(
        self_update,
        json!({
            "type": "maintenance_plan",
            "action": "self_update",
            "dry_run": true,
            "planned": true,
            "risk": {
                "level": "high",
                "approval_required": true,
                "reason": "self-update can replace the active runner binary"
            },
            "steps": [
                "download signed runner artifact",
                "verify artifact signature",
                "replace runner after approval"
            ]
        })
    );
    assert_eq!(self_update["risk"]["level"], "high");
    assert_eq!(self_update["risk"]["approval_required"], true);

    assert_eq!(
        uninstall,
        json!({
            "type": "maintenance_plan",
            "action": "uninstall",
            "dry_run": true,
            "planned": true,
            "risk": {
                "level": "high",
                "approval_required": true,
                "reason": "uninstall removes the rescue control plane from the target computer"
            },
            "steps": [
                "stop runner service",
                "remove runner binary",
                "remove pairing credentials"
            ]
        })
    );
    assert_eq!(uninstall["risk"]["level"], "high");
    assert_eq!(uninstall["risk"]["approval_required"], true);

    assert_eq!(captured_requests.len(), 2);
    for request in &captured_requests {
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.url,
            "https://runner.example/mobile/maintenance/plan"
        );
        assert!(request.token_present);
        assert_eq!(request.auth_scheme(), Some("Bearer"));
        assert_eq!(
            request.authorization_header_value(),
            Some(format!("Bearer {pairing_token}"))
        );
        assert!(!format!("{request:?}").contains(pairing_token));
    }

    assert_eq!(
        captured_requests[0].body,
        json!({
            "action": "self_update",
            "dry_run": true
        })
    );
    assert_eq!(
        captured_requests[1].body,
        json!({
            "action": "uninstall",
            "dry_run": true
        })
    );
}

#[test]
fn runner_maintenance_plan_request_rejects_unconfigured_endpoint() {
    let transport = RunnerHttpTransport::with_bearer_token("  ///  ", "runner-secret");
    let backend = RunnerHttpClosureBackend::new(|_request: RunnerHttpRequestSpec| {
        Ok(RunnerHttpResponseSpec::ok_json(Value::Null))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    let err = client.plan_maintenance("self_update", true).unwrap_err();

    assert_eq!(
        err,
        TransportError::Failed("runner maintenance transport is not configured".to_string())
    );
}

#[test]
fn runner_maintenance_approval_options_shape_dry_run_and_execute_attempts_without_leaking_token() {
    let pairing_token = "runner-maintenance-secret";
    let mut captured_requests: Vec<RunnerHttpRequestSpec> = Vec::new();
    let mut responses = VecDeque::from([
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "maintenance_plan",
            "action": "self_update",
            "dry_run": true,
            "approved": true
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "maintenance_plan",
            "action": "uninstall",
            "dry_run": true,
            "approved": true
        })),
        RunnerHttpResponseSpec::ok_json(json!({
            "type": "maintenance_plan",
            "action": "self_update",
            "dry_run": false,
            "approved": true
        })),
    ]);
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", pairing_token);
    let backend = RunnerHttpClosureBackend::new(|request: RunnerHttpRequestSpec| {
        captured_requests.push(request);
        responses
            .pop_front()
            .ok_or_else(|| TransportError::Failed("unexpected maintenance request".to_string()))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    let approved_dry_run = RunnerMaintenancePlanRequest::new("self_update").with_approval(
        RunnerMaintenanceApproval::new()
            .with_nonce("approval-nonce-123")
            .with_metadata(json!({
                "approval_id": "approval-maintenance-1",
                "approved_by": "mobile-user",
                "reason": "operator confirmed signed update"
            })),
    );
    let execute_attempt = RunnerMaintenancePlanRequest::new("self_update")
        .with_dry_run(false)
        .with_approval(RunnerMaintenanceApproval::new().with_nonce("approval-nonce-123"));
    let metadata_only_dry_run = RunnerMaintenancePlanRequest::new("uninstall").with_approval(
        RunnerMaintenanceApproval::new().with_metadata(json!({
            "approval_id": "approval-maintenance-2",
            "approved_by": "mobile-user"
        })),
    );

    client
        .plan_maintenance_request(approved_dry_run)
        .expect("approved dry-run request should be accepted");
    client
        .plan_maintenance_request(metadata_only_dry_run)
        .expect("metadata-only dry-run request should be accepted");
    client
        .plan_maintenance_request(execute_attempt)
        .expect("approved execute attempt request should be accepted");
    drop(client);

    assert_eq!(captured_requests.len(), 3);
    for request in &captured_requests {
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.url,
            "https://runner.example/mobile/maintenance/plan"
        );
        assert!(request.token_present);
        assert_eq!(request.auth_scheme(), Some("Bearer"));
        assert_eq!(
            request.authorization_header_value(),
            Some(format!("Bearer {pairing_token}"))
        );
        assert!(!request.body.to_string().contains(pairing_token));
        assert!(!format!("{request:?}").contains(pairing_token));
    }

    assert_eq!(
        captured_requests[0].body,
        json!({
            "action": "self_update",
            "dry_run": true,
            "approval": {
                "nonce": "approval-nonce-123",
                "metadata": {
                    "approval_id": "approval-maintenance-1",
                    "approved_by": "mobile-user",
                    "reason": "operator confirmed signed update"
                }
            }
        })
    );
    assert_eq!(
        captured_requests[1].body,
        json!({
            "action": "uninstall",
            "dry_run": true,
            "approval": {
                "metadata": {
                    "approval_id": "approval-maintenance-2",
                    "approved_by": "mobile-user"
                }
            }
        })
    );
    assert_eq!(
        captured_requests[2].body,
        json!({
            "action": "self_update",
            "dry_run": false,
            "approval": {
                "nonce": "approval-nonce-123"
            }
        })
    );
}
