use std::cell::RefCell;
use std::rc::Rc;

use deepseek_mobile_agent_core::agent_loop::AgentLoop;
use deepseek_mobile_agent_core::persistence::PendingApprovalSnapshot;
use deepseek_mobile_agent_core::transport::{
    RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec, RunnerHttpResponseSpec,
    RunnerHttpTransport, RunnerMaintenanceApproval, RunnerMaintenancePlanRequest, TransportError,
};
use deepseek_mobile_agent_core::{
    ApprovalDecision, ApprovalRequest, FakeTransport, ModelResponse, RemoteToolCall,
    RemoteToolName, RiskLevel,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn maintenance_call(call_id: &str, action: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::McpCall,
        arguments: json!({
            "server": "mobile-maintenance",
            "tool": "runner.maintenance.plan",
            "arguments": {
                "action": action,
                "dry_run": true
            }
        }),
    }
}

fn response(text: &str, tool_calls: Vec<RemoteToolCall>) -> ModelResponse {
    ModelResponse {
        text: text.to_string(),
        tool_calls,
    }
}

fn visible_summary(approval: &ApprovalRequest) -> String {
    format!(
        "Approve {} before requesting a runner maintenance plan",
        approval.tool_name
    )
}

fn request_maintenance_plan_after_mobile_approval(
    client: &mut RunnerHttpClient<
        RunnerHttpClosureBackend<
            impl FnMut(RunnerHttpRequestSpec) -> Result<RunnerHttpResponseSpec, TransportError>,
        >,
    >,
    pending: ApprovalRequest,
    decision: ApprovalDecision,
    nonce: &str,
    approved_by: &str,
) -> Result<Option<Value>, TransportError> {
    if decision == ApprovalDecision::Denied {
        return Ok(None);
    }

    let action = pending.call.arguments["arguments"]["action"]
        .as_str()
        .unwrap_or("self_update");
    let dry_run = pending.call.arguments["arguments"]["dry_run"]
        .as_bool()
        .unwrap_or(true);
    let plan = RunnerMaintenancePlanRequest::new(action)
        .with_dry_run(dry_run)
        .with_approval(
            RunnerMaintenanceApproval::new()
                .with_nonce(nonce)
                .with_metadata(json!({
                    "approval_id": pending.approval_id,
                    "call_id": pending.call_id,
                    "tool_name": pending.tool_name,
                    "approved_by": approved_by,
                    "risk": {
                        "level": format!("{:?}", pending.risk.level),
                        "reason": pending.risk.reason
                    }
                })),
        );

    client.plan_maintenance_request(plan).map(Some)
}

#[test]
fn mobile_approval_requests_runner_maintenance_plan_with_nonce_metadata_without_network() {
    let call = maintenance_call("maintenance-plan", "self_update");
    let mut tool_transport = FakeTransport::default();

    let turn = AgentLoop::new().run_model_tool_turn(
        response(
            "Self-update needs phone approval before runner maintenance planning.",
            vec![call],
        ),
        &mut tool_transport,
    );

    assert!(tool_transport.calls().is_empty());
    assert_eq!(turn.pending_approvals.len(), 1);
    let pending = turn.pending_approvals[0].clone();
    let summary = visible_summary(&pending);
    let visible = PendingApprovalSnapshot::from_request(&pending, &summary);
    assert_eq!(visible.user_visible_summary, summary);
    assert_eq!(pending.tool_name, "mcp_call");
    assert_eq!(pending.risk.level, RiskLevel::High);

    let captured_requests: Rc<RefCell<Vec<RunnerHttpRequestSpec>>> =
        Rc::new(RefCell::new(Vec::new()));
    let captured_for_backend = Rc::clone(&captured_requests);
    let backend = RunnerHttpClosureBackend::new(move |request: RunnerHttpRequestSpec| {
        captured_for_backend.borrow_mut().push(request);
        Ok(RunnerHttpResponseSpec::ok_json(json!({
            "type": "maintenance_plan",
            "action": "self_update",
            "dry_run": true,
            "planned": true
        })))
    });
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", "pairing-secret");
    let mut client = RunnerHttpClient::new(transport, backend);

    let plan = request_maintenance_plan_after_mobile_approval(
        &mut client,
        pending,
        ApprovalDecision::Approved,
        "approval-nonce-123",
        "mobile-user",
    )
    .expect("approved maintenance plan request should be sent");

    assert_eq!(plan.expect("runner should return a plan")["planned"], true);
    let requests = captured_requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url,
        "https://runner.example/mobile/maintenance/plan"
    );
    assert_eq!(
        requests[0].body,
        json!({
            "action": "self_update",
            "dry_run": true,
            "approval": {
                "nonce": "approval-nonce-123",
                "metadata": {
                    "approval_id": "approval-maintenance-plan",
                    "call_id": "maintenance-plan",
                    "tool_name": "mcp_call",
                    "approved_by": "mobile-user",
                    "risk": {
                        "level": "High",
                        "reason": "runner maintenance can update or remove the active runner"
                    }
                }
            }
        })
    );
}

#[test]
fn denied_mobile_maintenance_approval_does_not_send_runner_request() {
    let call = maintenance_call("maintenance-plan", "uninstall");
    let mut tool_transport = FakeTransport::default();
    let turn = AgentLoop::new()
        .run_model_tool_turn(response("Needs approval.", vec![call]), &mut tool_transport);
    let pending = turn.pending_approvals[0].clone();

    let captured_requests: Rc<RefCell<Vec<RunnerHttpRequestSpec>>> =
        Rc::new(RefCell::new(Vec::new()));
    let captured_for_backend = Rc::clone(&captured_requests);
    let backend = RunnerHttpClosureBackend::new(move |request: RunnerHttpRequestSpec| {
        captured_for_backend.borrow_mut().push(request);
        Ok(RunnerHttpResponseSpec::ok_json(Value::Null))
    });
    let transport =
        RunnerHttpTransport::with_bearer_token("https://runner.example/mobile", "pairing-secret");
    let mut client = RunnerHttpClient::new(transport, backend);

    let plan = request_maintenance_plan_after_mobile_approval(
        &mut client,
        pending,
        ApprovalDecision::Denied,
        "approval-nonce-456",
        "mobile-user",
    )
    .expect("denied maintenance approval should not fail");

    assert_eq!(plan, None);
    assert!(captured_requests.borrow().is_empty());
}
