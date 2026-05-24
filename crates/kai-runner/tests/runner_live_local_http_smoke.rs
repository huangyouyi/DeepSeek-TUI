use axum::Router;
use kai_runner::server::{build_router, build_router_with_token};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

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

struct LiveRunnerServer {
    addr: SocketAddr,
    task: JoinHandle<()>,
}

impl LiveRunnerServer {
    async fn start() -> Self {
        Self::start_with_router(build_router(kai_runner::KaiRunner)).await
    }

    async fn start_with_token(token: &str) -> Self {
        Self::start_with_router(build_router_with_token(kai_runner::KaiRunner, token)).await
    }

    async fn start_with_router(router: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        tokio::task::yield_now().await;

        Self { addr, task }
    }

    async fn request_json(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        self.request_json_with_bearer(method, path, None, body)
            .await
    }

    async fn request_json_with_bearer(
        &self,
        method: &str,
        path: &str,
        bearer_token: Option<&str>,
        body: Option<Value>,
    ) -> (u16, Value) {
        let mut stream = TcpStream::connect(self.addr).await.unwrap();
        let body = body.map(|body| body.to_string()).unwrap_or_default();
        let authorization = bearer_token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\n\
Host: {host}\r\n\
Content-Type: application/json\r\n\
{authorization}\
Content-Length: {content_length}\r\n\
Connection: close\r\n\
\r\n\
{body}",
            method = method,
            path = path,
            host = self.addr,
            authorization = authorization,
            content_length = body.len(),
            body = body
        );

        stream.write_all(request.as_bytes()).await.unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        parse_json_response(&response)
    }
}

impl Drop for LiveRunnerServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn parse_json_response(response: &[u8]) -> (u16, Value) {
    let response = std::str::from_utf8(response).unwrap();
    let (head, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("HTTP response did not include header terminator: {response:?}"));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .unwrap();
    let body = serde_json::from_str(body).unwrap();

    (status, body)
}

#[tokio::test]
async fn live_local_socket_smoke_covers_runner_http_entrypoints() {
    let server = LiveRunnerServer::start().await;

    let (status, body) = server.request_json("GET", "/health", None).await;
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({
            "status": "ok",
            "mode": "runner"
        })
    );

    let (status, body) = server.request_json("GET", "/capabilities", None).await;
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({
            "type": "capabilities",
            "mode": "runner",
            "tools": DOCUMENTED_RUNNER_TOOLS
        })
    );

    let (status, body) = server.request_json("GET", "/audit/recent", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["type"], json!("audit_recent"));
    assert_eq!(body["mode"], json!("runner"));
    assert_eq!(body["persistence"], json!("memory_scaffold"));
    assert_eq!(body["audit"][0]["event"], json!("capabilities.snapshot"));

    let (status, body) = server
        .request_json(
            "POST",
            "/tool-call",
            Some(json!({
                "call_id": "live-local-diagnose",
                "name": "remote.diagnose.system",
                "arguments": {
                    "include": ["system", "capabilities"]
                }
            })),
        )
        .await;

    assert_eq!(status, 200);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("live-local-diagnose"));
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
async fn live_local_socket_powershell_smoke_plans_without_running_powershell() {
    let server = LiveRunnerServer::start().await;

    let (status, body) = server
        .request_json(
            "POST",
            "/tool-call",
            Some(json!({
                "call_id": "live-local-powershell-plan",
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

    assert_eq!(status, 200);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("live-local-powershell-plan"));
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
    assert_eq!(body["result"]["risk"], json!("low"));
    assert_eq!(body["result"]["requires_approval"], json!(false));
    assert_eq!(body["result"]["error"]["code"], json!("blocked"));
}

#[tokio::test]
async fn live_local_socket_authenticated_powershell_smoke_requires_bearer_and_approval() {
    let server = LiveRunnerServer::start_with_token("test-token").await;
    let body = json!({
        "call_id": "live-local-powershell-high-risk",
        "name": "remote.powershell.exec",
        "arguments": {
            "script": "Set-ExecutionPolicy RemoteSigned -Scope LocalMachine; Start-Process powershell -Verb RunAs",
            "idempotency_key": "fix-execution-policy-1"
        }
    });

    let (missing_status, missing_body) = server
        .request_json_with_bearer("POST", "/tool-call", None, Some(body.clone()))
        .await;
    assert_eq!(missing_status, 401);
    assert_eq!(
        missing_body,
        json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        })
    );

    let (status, body) = server
        .request_json_with_bearer("POST", "/tool-call", Some("test-token"), Some(body))
        .await;

    assert_eq!(status, 200);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("live-local-powershell-high-risk"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.powershell.exec"));
    assert_eq!(body["result"]["status"], json!("planned"));
    assert_eq!(body["result"]["blocked"], json!(true));
    assert_eq!(body["result"]["would_execute"], json!(false));
    assert_eq!(body["result"]["risk"], json!("high"));
    assert_eq!(body["result"]["requires_approval"], json!(true));
    assert_eq!(body["result"]["approval"]["required"], json!(true));
    assert_eq!(
        body["result"]["approval"]["reasons"],
        json!(["uac_elevation", "execution_policy_change"])
    );
    assert_eq!(body["result"]["error"]["code"], json!("approval_required"));
}

#[tokio::test]
async fn live_local_socket_windows_rescue_diagnose_smoke_returns_powershell_plan() {
    let server = LiveRunnerServer::start().await;

    let (status, body) = server
        .request_json(
            "POST",
            "/tool-call",
            Some(json!({
                "call_id": "live-local-windows-rescue",
                "name": "remote.diagnose.system",
                "arguments": {
                    "profile": "windows_rescue",
                    "include_package_managers": true,
                    "include_permissions": true
                }
            })),
        )
        .await;

    assert_eq!(status, 200);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("live-local-windows-rescue"));
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
    }
    assert_eq!(checks[3]["requires_approval_for_fix"], json!(true));
    assert_eq!(checks[3]["risk_for_fix"], json!("high"));
}
