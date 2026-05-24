use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use kai_runner::server::build_router_with_shell_runner_and_token;
use kai_runner::{
    KaiRunner, ShellApprovalNonceManager, ShellExecutionPolicy, ShellExecutionRequest,
    ShellExecutionResult, ShellExecutor, SystemShellExecutor,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

#[derive(Debug, Clone)]
struct FakeShellExecutor {
    requests: Arc<Mutex<Vec<ShellExecutionRequest>>>,
    result: ShellExecutionResult,
}

impl FakeShellExecutor {
    fn new(result: ShellExecutionResult) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            result,
        }
    }

    fn requests(&self) -> Vec<ShellExecutionRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl ShellExecutor for FakeShellExecutor {
    fn execute(&self, request: ShellExecutionRequest) -> ShellExecutionResult {
        self.requests.lock().unwrap().push(request);
        self.result.clone()
    }
}

async fn request_json(
    router: Router,
    method: Method,
    uri: &str,
    bearer_token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");

    if let Some(token) = bearer_token {
        request = request.header("authorization", format!("Bearer {token}"));
    }

    let request = request
        .body(match body {
            Some(body) => Body::from(body.to_string()),
            None => Body::empty(),
        })
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };

    (status, body)
}

fn shell_router_with_fake_executor(fake: FakeShellExecutor) -> Router {
    build_router_with_shell_runner_and_token(
        KaiRunner::with_shell_executor(fake)
            .with_approval_nonce_manager(ShellApprovalNonceManager::new()),
        "test-token",
    )
}

fn shell_router_with_system_executor(
    executor: SystemShellExecutor,
    configure_policy: bool,
) -> Router {
    let runner = KaiRunner::with_shell_executor(executor)
        .with_approval_nonce_manager(ShellApprovalNonceManager::new());
    let runner = if configure_policy {
        runner.with_shell_execution_policy(
            ShellExecutionPolicy::default().with_path_entries(shell_path_entries()),
        )
    } else {
        runner
    };

    build_router_with_shell_runner_and_token(runner, "test-token")
}

fn shell_path_entries() -> Vec<String> {
    #[cfg(windows)]
    {
        vec![r"C:\Windows\System32".to_string()]
    }

    #[cfg(not(windows))]
    {
        vec!["/bin".to_string(), "/usr/bin".to_string()]
    }
}

fn harmless_echo_command() -> &'static str {
    #[cfg(windows)]
    {
        "echo kai-runner-real-shell-smoke"
    }

    #[cfg(not(windows))]
    {
        "printf 'kai-runner-real-shell-smoke\\n'"
    }
}

#[tokio::test]
async fn authenticated_shell_http_flow_issues_nonce_executes_once_and_rejects_replay() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello from fake shell\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_fake_executor(fake.clone());

    let (missing_status, missing_body) = request_json(
        router.clone(),
        Method::POST,
        "/tool-call",
        None,
        Some(json!({
            "call_id": "shell-http-missing-bearer",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo should-not-run",
                "approval_nonce": "missing-bearer-nonce"
            }
        })),
    )
    .await;

    assert_eq!(missing_status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        missing_body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());

    let (nonce_status, nonce_body) = request_json(
        router.clone(),
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;

    assert_eq!(nonce_status, StatusCode::OK);
    let nonce = nonce_body["approval_nonce"]
        .as_str()
        .expect("approval nonce response should include approval_nonce");
    assert!(!nonce.is_empty(), "approval nonce should not be empty");

    let (first_status, first_body) = request_json(
        router.clone(),
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "shell-http-first",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo hello",
                "approval_nonce": nonce
            }
        })),
    )
    .await;

    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(first_body["type"], json!("tool_call"));
    assert_eq!(first_body["call_id"], json!("shell-http-first"));
    assert_eq!(first_body["success"], json!(true));
    assert_eq!(first_body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(first_body["result"]["status"], json!("ok"));
    assert_eq!(
        first_body["result"]["stdout"],
        json!("hello from fake shell\n")
    );
    assert_eq!(fake.requests().len(), 1);
    assert_eq!(fake.requests()[0].command, "echo hello");

    let (replay_status, replay_body) = request_json(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "shell-http-replay",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo should-not-run",
                "approval_nonce": nonce
            }
        })),
    )
    .await;

    assert_eq!(replay_status, StatusCode::OK);
    assert_eq!(replay_body["type"], json!("tool_call"));
    assert_eq!(replay_body["call_id"], json!("shell-http-replay"));
    assert_eq!(replay_body["success"], json!(false));
    assert_eq!(replay_body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(replay_body["result"]["status"], json!("error"));
    assert_eq!(
        replay_body["result"]["error"]["code"],
        json!("approval_replayed")
    );
    assert_eq!(
        fake.requests().len(),
        1,
        "replayed nonce must not invoke executor again"
    );
}

#[tokio::test]
async fn real_shell_http_opt_in_executes_harmless_command_with_bearer_nonce_and_policy() {
    let router = shell_router_with_system_executor(SystemShellExecutor::enabled_for_tests(), true);

    let (_, nonce_body) = request_json(
        router.clone(),
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;
    let nonce = nonce_body["approval_nonce"].as_str().unwrap();

    let (status, body) = request_json(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "real-shell-http-ok",
            "name": "remote.shell.exec",
            "arguments": {
                "command": harmless_echo_command(),
                "approval_nonce": nonce,
                "timeout_ms": 1_000
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("real-shell-http-ok"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(body["result"]["status"], json!("ok"));
    assert_eq!(
        body["result"]["stdout"],
        json!("kai-runner-real-shell-smoke\n")
    );
    assert_eq!(body["result"]["stderr"], json!(""));
    assert_eq!(body["result"]["exit_code"], json!(0));
    assert_eq!(body["result"]["timed_out"], json!(false));
}

#[tokio::test]
async fn real_shell_http_rejects_missing_approval_nonce_before_execution() {
    let router = shell_router_with_system_executor(SystemShellExecutor::enabled_for_tests(), true);

    let (status, body) = request_json(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "real-shell-http-missing-nonce",
            "name": "remote.shell.exec",
            "arguments": {
                "command": harmless_echo_command(),
                "timeout_ms": 1_000
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("real-shell-http-missing-nonce"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(body["result"]["error"]["code"], json!("approval_required"));
    assert_eq!(
        body["result"]["error"]["message"],
        json!("shell execution requires approval_nonce")
    );
}

#[tokio::test]
async fn real_shell_http_rejects_missing_shell_execution_policy() {
    let router = shell_router_with_system_executor(SystemShellExecutor::enabled_for_tests(), false);

    let (_, nonce_body) = request_json(
        router.clone(),
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;
    let nonce = nonce_body["approval_nonce"].as_str().unwrap();

    let (status, body) = request_json(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "real-shell-http-missing-policy",
            "name": "remote.shell.exec",
            "arguments": {
                "command": harmless_echo_command(),
                "approval_nonce": nonce,
                "timeout_ms": 1_000
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("real-shell-http-missing-policy"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(body["result"]["error"]["code"], json!("policy_required"));
    assert_eq!(
        body["result"]["error"]["message"],
        json!("real shell execution requires an explicit ShellExecutionPolicy")
    );
}

#[tokio::test]
async fn real_shell_http_rejects_system_executor_until_enabled_for_tests() {
    let router = shell_router_with_system_executor(SystemShellExecutor::default(), true);

    let (_, nonce_body) = request_json(
        router.clone(),
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;
    let nonce = nonce_body["approval_nonce"].as_str().unwrap();

    let (status, body) = request_json(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "real-shell-http-not-enabled",
            "name": "remote.shell.exec",
            "arguments": {
                "command": harmless_echo_command(),
                "approval_nonce": nonce,
                "timeout_ms": 1_000
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("real-shell-http-not-enabled"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(
        body["result"]["stderr"],
        json!("real shell execution is not enabled for kai-runner")
    );
}
