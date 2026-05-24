use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use kai_runner::server::build_router;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn post_tool_call(body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/tool-call")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
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

#[tokio::test]
async fn tool_call_http_handler_runs_diagnose_system_end_to_end() {
    let (status, body) = post_tool_call(json!({
        "call_id": "diagnose-http-e2e",
        "name": "remote.diagnose.system",
        "arguments": {
            "include": ["system", "capabilities"]
        }
    }))
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("diagnose-http-e2e"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.diagnose.system"));
    assert_eq!(body["result"]["status"], json!("ok"));
    assert_eq!(body["result"]["system"]["os"], json!(std::env::consts::OS));
    assert_eq!(
        body["result"]["system"]["arch"],
        json!(std::env::consts::ARCH)
    );
    assert_eq!(
        body["result"]["system"]["family"],
        json!(std::env::consts::FAMILY)
    );
    assert_eq!(body["result"]["capabilities"]["mode"], json!("runner"));
    assert_eq!(
        body["result"]["capabilities"]["tools"],
        json!([
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
            "remote.mcp.call"
        ])
    );
}

#[tokio::test]
async fn tool_call_http_handler_blocks_shell_without_running_local_command() {
    let marker = std::env::temp_dir().join(format!(
        "kai-runner-shell-must-not-run-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&marker);

    let (status, body) = post_tool_call(json!({
        "call_id": "shell-http-blocked",
        "name": "remote.shell.exec",
        "arguments": {
            "command": format!("touch {}", marker.display())
        }
    }))
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("shell-http-blocked"));
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["result"]["tool"], json!("remote.shell.exec"));
    assert_eq!(body["result"]["status"], json!("error"));
    assert_eq!(body["result"]["error"]["code"], json!("blocked"));
    assert_eq!(
        body["result"]["error"]["message"],
        json!("kai-runner scaffold does not execute local shell commands")
    );
    assert!(
        !marker.exists(),
        "blocked remote.shell.exec unexpectedly created {}",
        marker.display()
    );
}
