use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use deepseek_mobile_web_server::{
    AppState, SshTarget, app_router_with_runner,
    ssh_exec::{CommandRunner, SshCommandOutput},
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Clone, Debug)]
struct FakeRunner {
    calls: Arc<Mutex<Vec<FakeCall>>>,
    output: SshCommandOutput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FakeCall {
    target: SshTarget,
    command: String,
}

impl FakeRunner {
    fn new(output: SshCommandOutput) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            output,
        }
    }

    fn calls(&self) -> Vec<FakeCall> {
        self.calls.lock().expect("calls mutex").clone()
    }
}

impl CommandRunner for FakeRunner {
    fn run(
        &self,
        target: &SshTarget,
        command: &deepseek_mobile_agent_core::ssh::SshCommandRequest,
    ) -> Result<SshCommandOutput, deepseek_mobile_web_server::ssh_exec::CommandRunError> {
        self.calls.lock().expect("calls mutex").push(FakeCall {
            target: target.clone(),
            command: command.command.clone(),
        });
        Ok(self.output.clone())
    }
}

fn test_state() -> AppState {
    AppState::new(SshTarget {
        host: "192.168.30.244".to_string(),
        user: "root".to_string(),
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
async fn ssh_check_reports_reachable_target_without_approval() {
    let runner = FakeRunner::new(SshCommandOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        duration: Duration::from_millis(17),
        timed_out: false,
    });

    let response = app_router_with_runner(test_state(), false, runner.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ssh/check")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "reachable");
    assert_eq!(
        body["target"],
        json!({
            "host": "192.168.30.244",
            "user": "root",
            "port": 2222,
            "key_present": false
        })
    );
    assert_eq!(body["command"], "true");
    assert_eq!(body["exit_code"], 0);
    assert_eq!(body["duration_ms"], 17);
    assert_eq!(body["requires_approval"], false);
    assert!(
        body["check_id"]
            .as_str()
            .expect("check id must be a string")
            .starts_with("ssh-check-")
    );
    assert_eq!(
        runner.calls(),
        vec![FakeCall {
            target: test_state().ssh_target(),
            command: "true".to_string()
        }]
    );
}

#[tokio::test]
async fn ssh_check_reports_unreachable_target_from_failed_command() {
    let runner = FakeRunner::new(SshCommandOutput {
        stdout: String::new(),
        stderr: "ssh: connect to host 192.168.30.244 port 2222: Connection refused\n".to_string(),
        exit_code: Some(255),
        duration: Duration::from_millis(92),
        timed_out: false,
    });

    let response = app_router_with_runner(test_state(), false, runner)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/ssh/check")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "unreachable");
    assert_eq!(
        body["target"],
        json!({
            "host": "192.168.30.244",
            "user": "root",
            "port": 2222,
            "key_present": false
        })
    );
    assert_eq!(body["command"], "true");
    assert_eq!(body["requires_approval"], false);
    assert_eq!(body["exit_code"], 255);
    assert_eq!(body["duration_ms"], 92);
    assert_eq!(
        body["error_summary"],
        "ssh: connect to host 192.168.30.244 port 2222: Connection refused"
    );
    assert_eq!(body["timed_out"], false);
    assert!(
        body["check_id"]
            .as_str()
            .expect("check id must be a string")
            .starts_with("ssh-check-")
    );
}
