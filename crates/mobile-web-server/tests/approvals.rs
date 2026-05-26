use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use deepseek_mobile_agent_core::ssh::SshCommandRequest;
use deepseek_mobile_web_server::{
    AppState, SshTarget, app_router_with_runner,
    ssh_exec::{CommandRunError, CommandRunner, SshCommandOutput},
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Clone, Debug)]
struct FakeRunner {
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeRunner {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls mutex").clone()
    }
}

#[derive(Clone, Debug)]
struct FailingRunner;

impl CommandRunner for FailingRunner {
    fn run(
        &self,
        _target: &SshTarget,
        _command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        Err(CommandRunError::Spawn(std::io::Error::other(
            "token: SHOULD_NOT_LEAK",
        )))
    }
}

impl CommandRunner for FakeRunner {
    fn run(
        &self,
        _target: &SshTarget,
        command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(command.command.clone());
        Ok(SshCommandOutput {
            stdout: format!("ran {}\n", command.command),
            stderr: String::new(),
            exit_code: Some(0),
            duration: Duration::from_millis(9),
            timed_out: false,
        })
    }
}

fn test_state() -> AppState {
    AppState::new(SshTarget {
        host: "127.0.0.1".to_string(),
        user: "tester".to_string(),
        port: 2222,
        key_present: false,
    })
}

async fn json_response(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body must be readable");
    serde_json::from_slice(&bytes).expect("response body must be json")
}

#[tokio::test]
async fn diagnostic_route_runs_preset_with_fake_runner() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/diagnostics/run")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"session_id": "session-1", "diagnostic": "system_info"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(runner.calls(), vec!["uname -a"]);
}

#[tokio::test]
async fn advanced_command_reject_does_not_execute() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());

    let prepared = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/commands/prepare")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"session_id": "session-1", "command": "rm -rf /tmp/nope"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(prepared.status(), StatusCode::CREATED);
    let approval = json_response(prepared).await;
    let approval_id = approval["id"].as_str().expect("approval id");

    let rejected = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "reject"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(rejected.status(), StatusCode::OK);
    assert_eq!(json_response(rejected).await["status"], "rejected");
    assert!(runner.calls().is_empty());
}

#[tokio::test]
async fn advanced_command_approve_once_executes_and_replay_is_rejected() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());

    let prepared = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/commands/prepare")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"session_id": "session-1", "command": "pwd"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let approval = json_response(prepared).await;
    let approval_id = approval["id"].as_str().expect("approval id");

    let approved = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "approve_once"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = json_response(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert_eq!(approved_body["result"]["stdout"], "ran pwd\n");
    assert_eq!(runner.calls(), vec!["pwd"]);

    let replay = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "approve_once"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(replay.status(), StatusCode::CONFLICT);
    assert_eq!(runner.calls(), vec!["pwd"]);
}

#[tokio::test]
async fn advanced_command_unsupported_response_does_not_consume_pending_approval() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());

    let prepared = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/commands/prepare")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"session_id": "session-1", "command": "pwd"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let approval = json_response(prepared).await;
    let approval_id = approval["id"].as_str().expect("approval id");

    let unsupported = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "typo"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(unsupported.status(), StatusCode::BAD_REQUEST);
    assert!(runner.calls().is_empty());

    let approved = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "approve_once"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(approved.status(), StatusCode::OK);
    assert_eq!(runner.calls(), vec!["pwd"]);
}

#[tokio::test]
async fn advanced_command_approve_session_executes_and_records_session_scope_audit() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());

    let prepared = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/commands/prepare")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "session_id": "session-1",
                        "command": "opkg update",
                        "cwd": "/tmp"
                    })
                    .to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let approval = json_response(prepared).await;
    assert_eq!(approval["risk_level"], "high");
    assert_eq!(approval["target"], "remote.shell.exec");
    let approval_id = approval["id"].as_str().expect("approval id");

    let approved = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"response": "approve_session"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = json_response(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert_eq!(approved_body["result"]["scope"], "session");
    assert_eq!(runner.calls(), vec!["opkg update"]);

    let audit_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/audit/recent")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let audit = json_response(audit_response).await;
    let last = audit
        .as_array()
        .expect("audit array")
        .last()
        .expect("audit");
    assert_eq!(last["kind"], "approval.approved");
    assert_eq!(last["metadata"]["action"], "approved");
    assert_eq!(last["metadata"]["response"], "approve_session");
    assert_eq!(last["metadata"]["scope"], "session");
    assert_eq!(last["metadata"]["risk"]["level"], "high");
    assert_eq!(last["metadata"]["target"], "remote.shell.exec");
    assert_eq!(last["metadata"]["command"], "opkg update");
    assert_eq!(last["metadata"]["cwd"], "/tmp");
}

#[tokio::test]
async fn advanced_command_always_alias_behaves_like_approve_session_with_stable_status() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());

    let prepared = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/commands/prepare")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"session_id": "session-1", "command": "opkg update"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let approval = json_response(prepared).await;
    let approval_id = approval["id"].as_str().expect("approval id");

    let approved = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "always"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = json_response(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert_eq!(approved_body["approval"]["status"], "approved");
    assert_eq!(approved_body["result"]["scope"], "session");
    assert_eq!(runner.calls(), vec!["opkg update"]);
}

#[tokio::test]
async fn advanced_command_approve_session_grants_and_audits_even_when_execution_fails() {
    let state = test_state();
    let app = app_router_with_runner(state.clone(), false, FailingRunner);

    let prepared = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/commands/prepare")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "session_id": "session-1",
                        "command": "opkg update",
                        "cwd": "/tmp"
                    })
                    .to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let approval = json_response(prepared).await;
    let approval_id = approval["id"].as_str().expect("approval id");

    let approved = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"response": "approve_session"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(approved.status(), StatusCode::BAD_GATEWAY);
    assert!(state.is_session_allowed("session-1", "opkg update", Some("/tmp")));
    let audit = state.audit_recent();
    let last = audit.last().expect("audit entry");
    assert_eq!(last.kind, "approval.approved");
    assert_eq!(last.metadata["response"], "approve_session");
    assert_eq!(last.metadata["scope"], "session");
    assert_eq!(last.metadata["result"]["status"], "failed");
    assert!(!last.metadata.to_string().contains("SHOULD_NOT_LEAK"));
}
