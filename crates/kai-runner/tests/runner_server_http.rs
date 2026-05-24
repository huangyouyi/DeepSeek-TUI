use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use kai_runner::pairing::PairingManager;
use kai_runner::server::{
    build_router, build_router_with_browser_runner, build_router_with_mcp_runner,
    build_router_with_pairing_manager, build_router_with_shell_runner_and_token,
    build_router_with_token,
};
use kai_runner::{
    BrowserSessionMetadata, BrowserSessionRegistry, KaiRunner, McpToolHandler, McpToolRequest,
    RunnerToolError, ShellApprovalNonceManager, ShellExecutionRequest, ShellExecutionResult,
    ShellExecutor,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

#[derive(Debug, Clone)]
struct FakeMcpHandler;

impl McpToolHandler for FakeMcpHandler {
    fn call(&self, request: McpToolRequest) -> Result<Value, RunnerToolError> {
        Ok(json!({
            "server": request.server,
            "tool": request.tool,
            "echo": request.arguments,
        }))
    }
}

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

async fn json_request(method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(match body {
            Some(body) => Body::from(body.to_string()),
            None => Body::empty(),
        })
        .unwrap();

    let response = build_router(kai_runner::KaiRunner)
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&body).unwrap();

    (status, body)
}

async fn authenticated_json_request(
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let request = request
        .body(match body {
            Some(body) => Body::from(body.to_string()),
            None => Body::empty(),
        })
        .unwrap();

    let response = build_router_with_token(kai_runner::KaiRunner, "test-token")
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&body).unwrap();

    (status, body)
}

async fn mcp_json_request(method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(match body {
            Some(body) => Body::from(body.to_string()),
            None => Body::empty(),
        })
        .unwrap();

    let response =
        build_router_with_mcp_runner(KaiRunner::with_mcp_handler("local", "echo", FakeMcpHandler))
            .oneshot(request)
            .await
            .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&body).unwrap();

    (status, body)
}

async fn router_json_request(
    router: axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
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
    let body = serde_json::from_slice(&body).unwrap();

    (status, body)
}

fn shell_router_with_fake_executor(fake: FakeShellExecutor) -> axum::Router {
    build_router_with_shell_runner_and_token(
        KaiRunner::with_shell_executor(fake).with_approval_nonce("nonce-123"),
        "test-token",
    )
}

fn shell_router_with_managed_fake_executor(fake: FakeShellExecutor) -> axum::Router {
    build_router_with_shell_runner_and_token(
        KaiRunner::with_shell_executor(fake)
            .with_approval_nonce_manager(ShellApprovalNonceManager::new()),
        "test-token",
    )
}

#[tokio::test]
async fn health_endpoint_reports_runner_status() {
    let (status, body) = json_request(Method::GET, "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "status": "ok",
            "mode": "runner"
        })
    );
}

#[tokio::test]
async fn authenticated_health_endpoint_allows_missing_token() {
    let (status, body) = authenticated_json_request(Method::GET, "/health", None, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "status": "ok",
            "mode": "runner"
        })
    );
}

#[tokio::test]
async fn authenticated_capabilities_endpoint_rejects_missing_token() {
    let (status, body) = authenticated_json_request(Method::GET, "/capabilities", None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
}

#[tokio::test]
async fn authenticated_capabilities_endpoint_rejects_wrong_token() {
    let (status, body) =
        authenticated_json_request(Method::GET, "/capabilities", Some("wrong-token"), None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
}

#[tokio::test]
async fn authenticated_capabilities_endpoint_accepts_correct_token() {
    let (status, body) =
        authenticated_json_request(Method::GET, "/capabilities", Some("test-token"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("capabilities"));
    assert_eq!(body["mode"], json!("runner"));
}

#[tokio::test]
async fn audit_recent_endpoint_returns_empty_memory_scaffold_before_events() {
    let (status, body) = json_request(Method::GET, "/audit/recent", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("audit_recent"));
    assert_eq!(body["mode"], json!("runner"));
    assert_eq!(body["persistence"], json!("memory_scaffold"));
    assert_eq!(body["audit"], json!([]));
}

#[tokio::test]
async fn authenticated_audit_recent_endpoint_rejects_missing_token() {
    let (status, body) = authenticated_json_request(Method::GET, "/audit/recent", None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
}

#[tokio::test]
async fn authenticated_audit_recent_endpoint_accepts_correct_token_and_redacts_secrets() {
    let (status, body) =
        authenticated_json_request(Method::GET, "/audit/recent", Some("test-token"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("audit_recent"));

    let body_text = body.to_string();
    assert!(!body_text.contains("test-token"));
    assert!(!body_text.contains("Bearer"));
    assert!(!body_text.contains("approval-nonce-"));
    assert!(!body_text.contains("nonce-123"));
}

#[tokio::test]
async fn maintenance_plan_endpoint_returns_self_update_dry_run_plan() {
    let (status, body) = json_request(
        Method::POST,
        "/maintenance/plan",
        Some(json!({
            "action": "self_update",
            "dry_run": true,
            "idempotency_key": "self-update-http-1"
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["action"], json!("self_update"));
    assert_eq!(body["risk"], json!("high"));
    assert_eq!(body["requires_approval"], json!(true));
    assert_eq!(body["dry_run"], json!(true));
    assert_eq!(body["idempotency_key"], json!("self-update-http-1"));
    assert_eq!(
        body["commands"],
        json!([
            "deepseek --version",
            "deepseek-tui --version",
            "download kai-runner release artifact to a temporary staging directory",
            "verify release checksum or signature",
            "replace deepseek and deepseek-tui from verified staging after explicit approval",
            "rollback to preserved binaries if health checks fail"
        ])
    );
    assert_eq!(
        body["maintenance_steps"][0],
        json!(
            "download candidate kai-runner, deepseek, and deepseek-tui artifacts into a temporary staging directory"
        )
    );
    assert_eq!(
        body["rollback_steps"][0],
        json!("restore the preserved previous binaries if verification or post-update checks fail")
    );
}

#[tokio::test]
async fn maintenance_plan_endpoint_returns_uninstall_dry_run_plan() {
    let (status, body) = json_request(
        Method::POST,
        "/maintenance/plan",
        Some(json!({
            "action": "uninstall",
            "dry_run": true
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["action"], json!("uninstall"));
    assert_eq!(body["risk"], json!("high"));
    assert_eq!(body["requires_approval"], json!(true));
    assert_eq!(body["dry_run"], json!(true));
    assert!(body.get("idempotency_key").is_none());
    assert_eq!(
        body["commands"],
        json!([
            "deepseek --version",
            "deepseek-tui --version",
            "stop active kai-runner or deepseek-tui process after explicit approval",
            "remove resolved deepseek and deepseek-tui binaries after explicit approval",
            "cleanup runner shims and temporary installer files",
            "check resolved binary paths are absent while user data remains"
        ])
    );
    assert_eq!(
        body["maintenance_steps"][0],
        json!("stop any active kai-runner or deepseek-tui process after explicit local approval")
    );
    assert_eq!(
        body["rollback_steps"][0],
        json!(
            "reinstall the previously recorded deepseek and deepseek-tui versions if removal was unintended"
        )
    );
}

#[tokio::test]
async fn authenticated_maintenance_plan_endpoint_rejects_missing_token() {
    let (status, body) = authenticated_json_request(
        Method::POST,
        "/maintenance/plan",
        None,
        Some(json!({
            "action": "self_update",
            "dry_run": true
        })),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
}

#[tokio::test]
async fn maintenance_plan_endpoint_rejects_execute_request() {
    let (status, body) = json_request(
        Method::POST,
        "/maintenance/plan",
        Some(json!({
            "action": "self_update",
            "dry_run": true,
            "execute": true
        })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], json!("approval_required"));
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("maintenance actions are not executable")
    );
}

#[tokio::test]
async fn shell_maintenance_plan_rejects_approval_with_missing_nonce() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_managed_fake_executor(fake);

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/maintenance/plan",
        Some("test-token"),
        Some(json!({
            "action": "self_update",
            "dry_run": true,
            "approval": { "metadata": { "reason": "test" } }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], json!("approval_required"));
    assert_eq!(
        body["audit"],
        json!([
            {
                "event": "maintenance.approval_nonce",
                "status": "missing",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "missing"
                }
            }
        ])
    );
}

#[tokio::test]
async fn shell_maintenance_plan_rejects_wrong_approval_nonce() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_managed_fake_executor(fake);

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/maintenance/plan",
        Some("test-token"),
        Some(json!({
            "action": "self_update",
            "dry_run": true,
            "approval": { "nonce": "wrong-nonce-secret" }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], json!("approval_denied"));
    assert_eq!(
        body["audit"],
        json!([
            {
                "event": "maintenance.approval_nonce",
                "status": "wrong",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "rejected"
                }
            }
        ])
    );
    assert!(!body["audit"].to_string().contains("wrong-nonce-secret"));
    assert!(!body["audit"].to_string().contains("test-token"));
}

#[tokio::test]
async fn shell_maintenance_plan_accepts_valid_approval_nonce_once() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_managed_fake_executor(fake);

    let (_, nonce_body) = router_json_request(
        router.clone(),
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;
    let nonce = nonce_body["approval_nonce"].as_str().unwrap();

    let request_body = json!({
        "action": "self_update",
        "dry_run": true,
        "approval": {
            "nonce": nonce,
            "metadata": { "reason": "operator confirmed" }
        }
    });

    let (first_status, first) = router_json_request(
        router.clone(),
        Method::POST,
        "/maintenance/plan",
        Some("test-token"),
        Some(request_body.clone()),
    )
    .await;
    let (second_status, second) = router_json_request(
        router,
        Method::POST,
        "/maintenance/plan",
        Some("test-token"),
        Some(request_body),
    )
    .await;

    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(first["action"], json!("self_update"));
    assert_eq!(first["dry_run"], json!(true));
    assert_eq!(first["approval"]["status"], json!("consumed"));
    assert_eq!(
        first["audit"],
        json!([
            {
                "event": "maintenance.approval_nonce",
                "status": "consumed",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "consumed"
                }
            },
            {
                "event": "maintenance.execute",
                "status": "no_op",
                "reason": "dry_run"
            }
        ])
    );
    assert!(!first["audit"].to_string().contains(nonce));
    assert!(!first["audit"].to_string().contains("test-token"));
    assert_eq!(
        first["approval"]["metadata"],
        json!({ "reason": "operator confirmed" })
    );
    assert_eq!(second_status, StatusCode::BAD_REQUEST);
    assert_eq!(second["error"]["code"], json!("approval_replayed"));
    assert_eq!(
        second["audit"],
        json!([
            {
                "event": "maintenance.approval_nonce",
                "status": "replay",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "replayed"
                }
            }
        ])
    );
}

#[tokio::test]
async fn shell_maintenance_execute_attempt_consumes_nonce_but_stays_no_op() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_managed_fake_executor(fake);

    let (_, nonce_body) = router_json_request(
        router.clone(),
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;
    let nonce = nonce_body["approval_nonce"].as_str().unwrap();
    let request_body = json!({
        "action": "uninstall",
        "dry_run": true,
        "execute": true,
        "approval": { "nonce": nonce }
    });

    let (first_status, first) = router_json_request(
        router.clone(),
        Method::POST,
        "/maintenance/plan",
        Some("test-token"),
        Some(request_body.clone()),
    )
    .await;
    let (second_status, second) = router_json_request(
        router,
        Method::POST,
        "/maintenance/plan",
        Some("test-token"),
        Some(request_body),
    )
    .await;

    assert_eq!(first_status, StatusCode::BAD_REQUEST);
    assert_eq!(first["error"]["code"], json!("not_executable"));
    assert_eq!(first["approval"]["status"], json!("consumed"));
    assert_eq!(
        first["audit"],
        json!([
            {
                "event": "maintenance.approval_nonce",
                "status": "consumed",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "consumed"
                }
            },
            {
                "event": "maintenance.execute",
                "status": "no_op",
                "reason": "not_executable"
            }
        ])
    );
    assert!(!first["audit"].to_string().contains(nonce));
    assert!(!first["audit"].to_string().contains("test-token"));
    assert_eq!(second_status, StatusCode::BAD_REQUEST);
    assert_eq!(second["error"]["code"], json!("approval_replayed"));
    assert_eq!(
        second["audit"],
        json!([
            {
                "event": "maintenance.approval_nonce",
                "status": "replay",
                "nonce": {
                    "label": "approval_nonce",
                    "status": "replayed"
                }
            }
        ])
    );
}

#[tokio::test]
async fn pairing_code_endpoint_creates_code_without_token() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(120)),
    );

    let (status, body) =
        router_json_request(router, Method::POST, "/pairing/code", None, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["code"], json!("000001"));
    assert_eq!(body["ttl_seconds"], json!(120));
    assert!(body.get("token").is_none());
}

#[tokio::test]
async fn pairing_redeem_endpoint_returns_bearer_token() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(120)),
    );
    let (_, code_body) =
        router_json_request(router.clone(), Method::POST, "/pairing/code", None, None).await;

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/pairing/redeem",
        None,
        Some(json!({ "code": code_body["code"] })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "token": "pairing-token-000001",
            "label": "pairing-token"
        })
    );
}

#[tokio::test]
async fn pairing_redeem_endpoint_rejects_used_code() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(120)),
    );
    let (_, code_body) =
        router_json_request(router.clone(), Method::POST, "/pairing/code", None, None).await;
    let redeem_body = Some(json!({ "code": code_body["code"] }));

    let (first_status, _) = router_json_request(
        router.clone(),
        Method::POST,
        "/pairing/redeem",
        None,
        redeem_body.clone(),
    )
    .await;
    let (second_status, second_body) =
        router_json_request(router, Method::POST, "/pairing/redeem", None, redeem_body).await;

    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(second_status, StatusCode::CONFLICT);
    assert_eq!(second_body["error"]["code"], json!("already_used"));
}

#[tokio::test]
async fn pairing_redeem_endpoint_rejects_unknown_code() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(120)),
    );

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/pairing/redeem",
        None,
        Some(json!({ "code": "missing-code" })),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], json!("unknown"));
}

#[tokio::test]
async fn pairing_redeem_endpoint_rejects_expired_code() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(0)),
    );
    let (_, code_body) =
        router_json_request(router.clone(), Method::POST, "/pairing/code", None, None).await;
    tokio::time::sleep(Duration::from_millis(1)).await;

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/pairing/redeem",
        None,
        Some(json!({ "code": code_body["code"] })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], json!("expired"));
}

#[tokio::test]
async fn pairing_redeemed_token_authorizes_capabilities() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(120)),
    );
    let (unauthorized_status, _) =
        router_json_request(router.clone(), Method::GET, "/capabilities", None, None).await;
    let (_, code_body) =
        router_json_request(router.clone(), Method::POST, "/pairing/code", None, None).await;
    let (_, token_body) = router_json_request(
        router.clone(),
        Method::POST,
        "/pairing/redeem",
        None,
        Some(json!({ "code": code_body["code"] })),
    )
    .await;
    let token = token_body["token"].as_str().unwrap();

    let (status, body) =
        router_json_request(router, Method::GET, "/capabilities", Some(token), None).await;

    assert_eq!(unauthorized_status, StatusCode::UNAUTHORIZED);
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("capabilities"));
    assert_eq!(body["mode"], json!("runner"));
}

#[tokio::test]
async fn capabilities_endpoint_returns_runner_tools() {
    let (status, body) = json_request(Method::GET, "/capabilities", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("capabilities"));
    assert_eq!(body["mode"], json!("runner"));
    assert_eq!(body["tools"][0], json!("remote.shell.exec"));
    assert_eq!(body["tools"][10], json!("remote.mcp.call"));
}

#[tokio::test]
async fn tool_call_endpoint_runs_diagnose_through_runner() {
    let (status, body) = json_request(
        Method::POST,
        "/tool-call",
        Some(json!({
            "call_id": "diag-http-1",
            "name": "remote.diagnose.system",
            "arguments": {}
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("diag-http-1"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.diagnose.system"));
    assert_eq!(body["result"]["system"]["os"], json!(std::env::consts::OS));
}

#[tokio::test]
async fn tool_call_endpoint_blocks_shell_without_executing() {
    let (status, body) = json_request(
        Method::POST,
        "/tool-call",
        Some(json!({
            "call_id": "shell-http-1",
            "name": "remote.shell.exec",
            "arguments": { "command": "echo should-not-run" }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "type": "tool_call",
            "call_id": "shell-http-1",
            "success": false,
            "result": {
                "tool": "remote.shell.exec",
                "status": "error",
                "error": {
                    "code": "blocked",
                    "message": "kai-runner scaffold does not execute local shell commands"
                }
            }
        })
    );
}

#[tokio::test]
async fn browser_enabled_tool_call_endpoint_opens_browser_session() {
    let registry = BrowserSessionRegistry::new();
    let router =
        build_router_with_browser_runner(KaiRunner::with_browser_registry(registry.clone()));

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        None,
        Some(json!({
            "call_id": "browser-http-open",
            "name": "remote.browser.open",
            "arguments": {
                "url": "https://example.com/docs",
                "profile": "work"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("browser-http-open"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.browser.open"));
    assert_eq!(body["result"]["status"], json!("ok"));
    assert_eq!(body["result"]["data"]["status"], json!("planned"));
    assert_eq!(
        body["result"]["data"]["session_id"],
        json!("browser-session-1")
    );
    assert_eq!(registry.list_sessions().len(), 1);
    assert_eq!(registry.list_sessions()[0].id, "browser-session-1");
}

#[tokio::test]
async fn browser_enabled_extract_text_endpoint_reports_browser_unavailable() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/docs".to_string()),
        profile: Some("work".to_string()),
    });
    let router = build_router_with_browser_runner(KaiRunner::with_browser_registry(registry));

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        None,
        Some(json!({
            "call_id": "browser-http-extract",
            "name": "remote.browser.extract_text",
            "arguments": {
                "session_id": session.id,
                "selector": "main"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("browser-http-extract"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.browser.extract_text"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(
        body["result"]["error"]["code"],
        json!("browser_unavailable")
    );
    assert_eq!(body["result"]["data"]["session_id"], json!(session.id));
}

#[tokio::test]
async fn browser_enabled_click_endpoint_still_requires_approval() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/docs".to_string()),
        profile: Some("work".to_string()),
    });
    let router = build_router_with_browser_runner(KaiRunner::with_browser_registry(registry));

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        None,
        Some(json!({
            "call_id": "browser-http-click",
            "name": "remote.browser.click",
            "arguments": {
                "session_id": session.id,
                "selector": "button[type=submit]"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("browser-http-click"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.browser.click"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(body["result"]["error"]["code"], json!("approval_required"));
    assert_eq!(body["result"]["data"]["session_id"], json!(session.id));
}

#[tokio::test]
async fn shell_enabled_tool_call_endpoint_rejects_missing_bearer() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_fake_executor(fake.clone());

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        None,
        Some(json!({
            "call_id": "shell-http-auth-missing",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo should-not-run",
                "approval_nonce": "nonce-123"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
}

#[tokio::test]
async fn shell_enabled_approval_nonce_endpoint_rejects_missing_bearer() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_managed_fake_executor(fake);

    let (status, body) =
        router_json_request(router, Method::POST, "/approval/nonce", None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );
}

#[tokio::test]
async fn shell_enabled_approval_nonce_endpoint_returns_bearer_nonce() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_managed_fake_executor(fake);

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let nonce = body["approval_nonce"]
        .as_str()
        .expect("nonce must be a string");
    assert!(!nonce.is_empty());
}

#[tokio::test]
async fn shell_enabled_issued_approval_nonce_allows_one_shell_call_then_rejects_replay() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_managed_fake_executor(fake.clone());

    let (_, nonce_body) = router_json_request(
        router.clone(),
        Method::POST,
        "/approval/nonce",
        Some("test-token"),
        None,
    )
    .await;
    let nonce = nonce_body["approval_nonce"].as_str().unwrap();
    let first_body = json!({
        "call_id": "shell-http-issued-once",
        "name": "remote.shell.exec",
        "arguments": {
            "command": "echo hello",
            "approval_nonce": nonce
        }
    });
    let second_body = json!({
        "call_id": "shell-http-issued-replay",
        "name": "remote.shell.exec",
        "arguments": {
            "command": "echo hello again",
            "approval_nonce": nonce
        }
    });

    let (first_status, first) = router_json_request(
        router.clone(),
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(first_body),
    )
    .await;
    let (second_status, second) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(second_body),
    )
    .await;

    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(first["success"], json!(true));
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(second["success"], json!(false));
    assert_eq!(
        second["result"]["error"]["code"],
        json!("approval_replayed")
    );
    assert_eq!(fake.requests().len(), 1);
    assert_eq!(fake.requests()[0].command, "echo hello");
}

#[tokio::test]
async fn shell_enabled_tool_call_endpoint_rejects_missing_approval_nonce() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_fake_executor(fake.clone());

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "shell-http-nonce-missing",
            "name": "remote.shell.exec",
            "arguments": { "command": "echo should-not-run" }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("shell-http-nonce-missing"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(body["result"]["error"]["code"], json!("approval_required"));
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
}

#[tokio::test]
async fn shell_enabled_tool_call_endpoint_executes_fake_shell_with_bearer_and_approval_nonce() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "hello\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_fake_executor(fake.clone());

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "shell-http-ok",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo hello",
                "approval_nonce": "nonce-123"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("shell-http-ok"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(
        body["result"],
        json!({
            "tool": "remote.shell.exec",
            "status": "ok",
            "stdout": "hello\n",
            "stderr": "",
            "exit_code": 0,
            "timed_out": false,
            "command": "echo hello"
        })
    );
    assert_eq!(fake.requests().len(), 1);
    assert_eq!(fake.requests()[0].command, "echo hello");
}

#[tokio::test]
async fn shell_enabled_tool_call_endpoint_rejects_wrong_approval_nonce() {
    let fake = FakeShellExecutor::new(ShellExecutionResult {
        stdout: "should-not-run\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        timed_out: false,
    });
    let router = shell_router_with_fake_executor(fake.clone());

    let (status, body) = router_json_request(
        router,
        Method::POST,
        "/tool-call",
        Some("test-token"),
        Some(json!({
            "call_id": "shell-http-nonce-wrong",
            "name": "remote.shell.exec",
            "arguments": {
                "command": "echo should-not-run",
                "approval_nonce": "wrong"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("shell-http-nonce-wrong"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(body["result"]["error"]["code"], json!("approval_denied"));
    assert_eq!(fake.requests(), Vec::<ShellExecutionRequest>::new());
}

#[tokio::test]
async fn mcp_call_endpoint_maps_mobile_core_body_to_runner_tool_call() {
    let (status, body) = json_request(
        Method::POST,
        "/mcp/call",
        Some(json!({
            "call_id": "mcp-http-1",
            "arguments": {
                "server": "demo",
                "tool": "lookup",
                "arguments": { "query": "rust" }
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "type": "tool_call",
            "call_id": "mcp-http-1",
            "success": false,
            "result": {
                "tool": "remote.mcp.call",
                "status": "error",
                "error": {
                    "code": "unsupported",
                    "message": "remote.mcp.call is not implemented in the kai-runner scaffold"
                }
            }
        })
    );
}

#[tokio::test]
async fn mcp_call_endpoint_routes_registered_mcp_call() {
    let (status, body) = mcp_json_request(
        Method::POST,
        "/mcp/call",
        Some(json!({
            "call_id": "mcp-http-registered-1",
            "arguments": {
                "server": "local",
                "tool": "echo",
                "arguments": { "text": "hello" }
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "type": "tool_call",
            "call_id": "mcp-http-registered-1",
            "success": true,
            "result": {
                "tool": "remote.mcp.call",
                "status": "ok",
                "data": {
                    "server": "local",
                    "tool": "echo",
                    "echo": { "text": "hello" }
                }
            }
        })
    );
}

#[tokio::test]
async fn mcp_call_endpoint_returns_unknown_mcp_tool() {
    let (status, body) = mcp_json_request(
        Method::POST,
        "/mcp/call",
        Some(json!({
            "call_id": "mcp-http-missing-1",
            "arguments": {
                "server": "local",
                "tool": "missing",
                "arguments": {}
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("mcp-http-missing-1"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.mcp.call"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(body["result"]["error"]["code"], json!("unknown_mcp_tool"));
}

#[tokio::test]
async fn mcp_call_endpoint_rejects_missing_call_id() {
    let (status, body) = json_request(
        Method::POST,
        "/mcp/call",
        Some(json!({
            "arguments": {}
        })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "bad_request",
                "message": "invalid mcp-call JSON body"
            }
        })
    );
}
