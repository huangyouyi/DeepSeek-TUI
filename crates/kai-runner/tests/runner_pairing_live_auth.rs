use kai_runner::pairing::PairingManager;
use kai_runner::server::build_router_with_pairing_manager;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

struct LivePairingRunnerServer {
    addr: SocketAddr,
    task: JoinHandle<()>,
}

impl LivePairingRunnerServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router_with_pairing_manager(
            kai_runner::KaiRunner,
            PairingManager::deterministic(Duration::from_secs(120)),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        tokio::task::yield_now().await;

        Self { addr, task }
    }

    async fn request_json(
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

impl Drop for LivePairingRunnerServer {
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
async fn live_pairing_token_authorizes_capabilities_and_tool_call() {
    let server = LivePairingRunnerServer::start().await;

    let (status, body) = server
        .request_json("POST", "/pairing/code", None, None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["code"], json!("000001"));
    assert_eq!(body["ttl_seconds"], json!(120));

    let (status, body) = server
        .request_json(
            "POST",
            "/pairing/redeem",
            None,
            Some(json!({ "code": body["code"] })),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["token"], json!("pairing-token-000001"));
    assert_eq!(body["label"], json!("pairing-token"));
    let token = body["token"].as_str().unwrap();

    let (status, body) = server
        .request_json("GET", "/capabilities", Some(token), None)
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["type"], json!("capabilities"));
    assert_eq!(body["mode"], json!("runner"));
    assert!(
        body["tools"]
            .as_array()
            .unwrap()
            .contains(&json!("remote.diagnose.system"))
    );

    let (status, body) = server
        .request_json(
            "POST",
            "/tool-call",
            Some(token),
            Some(json!({
                "call_id": "live-pairing-diagnose",
                "name": "remote.diagnose.system",
                "arguments": {}
            })),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("live-pairing-diagnose"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.diagnose.system"));
    assert_eq!(body["result"]["status"], json!("ok"));
}

#[tokio::test]
async fn live_pairing_protected_routes_reject_missing_and_wrong_tokens() {
    let server = LivePairingRunnerServer::start().await;

    let (missing_status, missing_body) = server
        .request_json("GET", "/capabilities", None, None)
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

    let (wrong_status, wrong_body) = server
        .request_json("GET", "/capabilities", Some("wrong-token"), None)
        .await;
    assert_eq!(wrong_status, 401);
    assert_eq!(wrong_body, missing_body);

    let (status, body) = server
        .request_json("POST", "/pairing/code", None, None)
        .await;
    assert_eq!(status, 200);
    let (status, body) = server
        .request_json(
            "POST",
            "/pairing/redeem",
            None,
            Some(json!({ "code": body["code"] })),
        )
        .await;
    assert_eq!(status, 200);
    let token = body["token"].as_str().unwrap();

    let (missing_status, missing_body) = server
        .request_json(
            "POST",
            "/tool-call",
            None,
            Some(json!({
                "call_id": "missing-token-diagnose",
                "name": "remote.diagnose.system",
                "arguments": {}
            })),
        )
        .await;
    assert_eq!(missing_status, 401);
    assert_eq!(missing_body, wrong_body);

    let (wrong_status, wrong_body) = server
        .request_json(
            "POST",
            "/tool-call",
            Some("wrong-token"),
            Some(json!({
                "call_id": "wrong-token-diagnose",
                "name": "remote.diagnose.system",
                "arguments": {}
            })),
        )
        .await;
    assert_eq!(wrong_status, 401);
    assert_eq!(wrong_body, missing_body);

    let (status, body) = server
        .request_json(
            "POST",
            "/tool-call",
            Some(token),
            Some(json!({
                "call_id": "correct-token-diagnose",
                "name": "remote.diagnose.system",
                "arguments": {}
            })),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["success"], json!(true));
}
