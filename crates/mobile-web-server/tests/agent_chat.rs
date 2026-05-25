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
            duration: Duration::from_millis(12),
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

async fn create_session(app: axum::Router) -> String {
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"title": "Agent chat"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_response(response).await;
    body["id"].as_str().expect("session id").to_string()
}

#[tokio::test]
async fn agent_turn_routes_chinese_system_question_to_uname_not_raw_shell_text() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请问当前运行在什么系统？"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"][0]["command"], "uname -a");
    assert_eq!(body["executed_tools"][0]["requires_approval"], false);
    assert!(
        body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("ran uname -a")
    );
    assert_eq!(runner.calls(), vec!["uname -a"]);
    assert!(
        !runner
            .calls()
            .contains(&"请问当前运行在什么系统？".to_string())
    );

    let messages = state.messages(&session_id);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
}

#[tokio::test]
async fn agent_turn_routes_disk_question_to_df() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "请查看磁盘空间"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"][0]["command"], "df -h");
    assert_eq!(runner.calls(), vec!["df -h"]);
}

#[tokio::test]
async fn agent_turn_text_only_answer_executes_no_ssh() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "你好"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"], json!([]));
    assert!(
        body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("remote Linux")
    );
    assert!(runner.calls().is_empty());
}

#[tokio::test]
async fn agent_turn_high_risk_tool_request_creates_pending_approval_without_execution() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "waiting_for_approval");
    assert_eq!(body["executed_tools"], json!([]));
    assert_eq!(body["pending_approvals"][0]["command"], "opkg update");
    assert_eq!(
        body["pending_approvals"][0]["agent_turn_id"].is_string(),
        true
    );
    assert!(runner.calls().is_empty());
}

#[tokio::test]
async fn agent_turn_high_risk_approval_returns_final_assistant_summary() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let turn_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(turn_response.status(), StatusCode::OK);
    let turn_body = json_response(turn_response).await;
    let approval_id = turn_body["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");

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
    let approved_body = json_response(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert!(
        approved_body["result"]["summary"]
            .as_str()
            .unwrap_or("")
            .contains("Approved command `opkg update`")
    );
    assert!(
        state
            .messages(&session_id)
            .last()
            .and_then(|message| message.parts[0].text.as_deref())
            .unwrap_or("")
            .contains("本轮远程命令已全部执行完成")
    );
    assert_eq!(runner.calls(), vec!["opkg update"]);
}
