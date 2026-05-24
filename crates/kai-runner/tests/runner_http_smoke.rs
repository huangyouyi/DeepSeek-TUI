use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use kai_runner::server::{build_router, build_router_with_token};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

const DOCUMENTED_RUNNER_TOOLS: &[&str] = &[
    "remote.shell.exec",
    "remote.powershell.exec",
    "remote.file.read",
    "remote.file.write",
    "remote.diagnose.system",
    "remote.package.install",
    "remote.browser.open",
    "remote.browser.extract_text",
    "remote.browser.click",
    "remote.bootstrap.guide",
    "remote.mcp.call",
];

async fn request_json(method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
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

async fn authenticated_request_json(
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

    let response = build_router_with_token(kai_runner::KaiRunner, "test-token")
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&body).unwrap();

    (status, body)
}

#[tokio::test]
async fn capabilities_http_smoke_returns_runner_mode_with_documented_tools() {
    let (status, body) = request_json(Method::GET, "/capabilities", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "type": "capabilities",
            "mode": "runner",
            "tools": DOCUMENTED_RUNNER_TOOLS,
        })
    );
}

#[tokio::test]
async fn diagnose_system_http_smoke_succeeds() {
    let (status, body) = request_json(
        Method::POST,
        "/tool-call",
        Some(json!({
            "call_id": "smoke-diagnose",
            "name": "remote.diagnose.system",
            "arguments": {}
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("smoke-diagnose"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.diagnose.system"));
    assert_eq!(body["result"]["status"], json!("ok"));
    assert_eq!(body["result"]["system"]["os"], json!(std::env::consts::OS));
    assert_eq!(
        body["result"]["system"]["arch"],
        json!(std::env::consts::ARCH)
    );
    assert_eq!(body["result"]["capabilities"]["mode"], json!("runner"));
    assert_eq!(
        body["result"]["capabilities"]["tools"],
        json!(DOCUMENTED_RUNNER_TOOLS)
    );
}

#[tokio::test]
async fn powershell_exec_http_smoke_plans_without_running_powershell() {
    let (status, body) = request_json(
        Method::POST,
        "/tool-call",
        Some(json!({
            "call_id": "smoke-powershell-plan",
            "name": "remote.powershell.exec",
            "arguments": {
                "script": "Get-Command winget; $PSVersionTable.PSVersion",
                "working_directory": "C:\\Users\\Alice",
                "timeout_ms": 60000,
                "execution_policy": "bypass_not_allowed",
                "idempotency_key": "diag-winget-1",
                "risk_hint": "low"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("smoke-powershell-plan"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.powershell.exec"));
    assert_eq!(body["result"]["status"], json!("planned"));
    assert_eq!(body["result"]["blocked"], json!(true));
    assert_eq!(body["result"]["would_execute"], json!(false));
    assert_eq!(
        body["result"]["script"],
        json!("Get-Command winget; $PSVersionTable.PSVersion")
    );
    assert_eq!(
        body["result"]["working_directory"],
        json!("C:\\Users\\Alice")
    );
    assert_eq!(body["result"]["timeout_ms"], json!(60000));
    assert_eq!(
        body["result"]["execution_policy"],
        json!("bypass_not_allowed")
    );
    assert_eq!(body["result"]["idempotency_key"], json!("diag-winget-1"));
    assert_eq!(body["result"]["risk"], json!("low"));
    assert_eq!(body["result"]["requires_approval"], json!(false));
    assert_eq!(body["result"]["error"]["code"], json!("blocked"));
}

#[tokio::test]
async fn authenticated_powershell_http_smoke_rejects_missing_bearer_and_reports_high_risk_approval()
{
    let body = json!({
        "call_id": "smoke-powershell-high-risk",
        "name": "remote.powershell.exec",
        "arguments": {
            "script": "Set-ExecutionPolicy RemoteSigned -Scope LocalMachine; Start-Process powershell -Verb RunAs",
            "idempotency_key": "fix-execution-policy-1"
        }
    });

    let (missing_status, missing_body) =
        authenticated_request_json(Method::POST, "/tool-call", None, Some(body.clone())).await;

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

    let (status, body) =
        authenticated_request_json(Method::POST, "/tool-call", Some("test-token"), Some(body))
            .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("smoke-powershell-high-risk"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.powershell.exec"));
    assert_eq!(body["result"]["status"], json!("planned"));
    assert_eq!(body["result"]["blocked"], json!(true));
    assert_eq!(body["result"]["would_execute"], json!(false));
    assert_eq!(body["result"]["risk"], json!("high"));
    assert_eq!(body["result"]["requires_approval"], json!(true));
    assert_eq!(body["result"]["approval"]["required"], json!(true));
    assert_eq!(body["result"]["approval"]["risk"], json!("high"));
    assert_eq!(
        body["result"]["approval"]["reasons"],
        json!(["uac_elevation", "execution_policy_change"])
    );
    assert_eq!(body["result"]["error"]["code"], json!("approval_required"));
}

#[tokio::test]
async fn windows_rescue_diagnose_http_smoke_returns_powershell_plan() {
    let (status, body) = request_json(
        Method::POST,
        "/tool-call",
        Some(json!({
            "call_id": "smoke-windows-rescue",
            "name": "remote.diagnose.system",
            "arguments": {
                "profile": "windows_rescue",
                "include_package_managers": true,
                "include_permissions": true
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("smoke-windows-rescue"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.diagnose.system"));
    assert_eq!(body["result"]["status"], json!("ok"));
    assert_eq!(
        body["result"]["diagnostic_plan"]["profile"],
        json!("windows_rescue")
    );
    assert_eq!(
        body["result"]["diagnostic_plan"]["would_execute"],
        json!(false)
    );

    let checks = body["result"]["diagnostic_plan"]["checks"]
        .as_array()
        .expect("windows rescue diagnostic plan should include checks");
    let codes: Vec<&str> = checks
        .iter()
        .map(|check| check["code"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes,
        vec![
            "path_environment",
            "winget_health",
            "event_log_installer_errors",
            "uac_elevation_state",
            "python_install_failure",
            "node_install_failure",
            "vscode_install_failure",
        ]
    );

    for check in checks {
        assert_eq!(check["tool"], json!("remote.powershell.exec"));
        assert_eq!(check["status"], json!("planned"));
        assert_eq!(check["would_execute"], json!(false));
        assert_eq!(check["risk"], json!("low"));
    }

    assert_eq!(checks[3]["requires_approval_for_fix"], json!(true));
    assert_eq!(checks[3]["risk_for_fix"], json!("high"));
}

#[tokio::test]
async fn package_install_http_smoke_defaults_to_planned_dry_run() {
    let (status, body) = request_json(
        Method::POST,
        "/tool-call",
        Some(json!({
            "call_id": "smoke-package",
            "name": "remote.package.install",
            "arguments": {
                "manager": "apt",
                "package": "ripgrep"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "type": "tool_call",
            "call_id": "smoke-package",
            "success": true,
            "result": {
                "tool": "remote.package.install",
                "status": "ok",
                "data": {
                    "status": "planned",
                    "commands": ["apt install ripgrep"],
                    "requires_sudo": false,
                    "log": "planned package install: apt install ripgrep",
                    "next_approval_required": true,
                    "manager": "apt",
                    "packages": ["ripgrep"],
                    "operation": "install",
                    "dry_run": true,
                    "args": ["apt", "install", "ripgrep"],
                    "command_preview": "apt install ripgrep"
                }
            }
        })
    );
}

#[tokio::test]
async fn browser_open_http_smoke_returns_planned_response() {
    let (status, body) = request_json(
        Method::POST,
        "/tool-call",
        Some(json!({
            "call_id": "smoke-browser",
            "name": "remote.browser.open",
            "arguments": {
                "url": "https://example.com/docs",
                "profile": "default"
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "type": "tool_call",
            "call_id": "smoke-browser",
            "success": true,
            "result": {
                "tool": "remote.browser.open",
                "status": "ok",
                "data": {
                    "status": "planned",
                    "url": "https://example.com/docs",
                    "profile": "default",
                    "events": [
                        {
                            "type": "planned",
                            "message": "browser.open accepted but no browser was launched"
                        }
                    ],
                    "next_approval_required": false
                }
            }
        })
    );
}

#[tokio::test]
async fn mcp_call_http_smoke_round_trips_call_id_as_unsupported_tool_call() {
    let (status, body) = request_json(
        Method::POST,
        "/mcp/call",
        Some(json!({
            "call_id": "smoke-mcp",
            "arguments": {
                "server": "docs",
                "tool": "search",
                "arguments": {
                    "query": "capabilities"
                }
            }
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "type": "tool_call",
            "call_id": "smoke-mcp",
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
