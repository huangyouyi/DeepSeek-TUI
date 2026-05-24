use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use kai_runner::pairing::PairingManager;
use kai_runner::server::build_router_with_pairing_manager;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;

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

#[tokio::test]
async fn pairing_code_can_be_redeemed_for_authenticated_runner_access() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(60)),
    );
    let (status, body) =
        request_json(router.clone(), Method::POST, "/pairing/code", None, None).await;

    assert!(
        status.is_success(),
        "pairing code creation should succeed, got {status}: {body}"
    );
    let code = body["code"]
        .as_str()
        .expect("pairing code response should include a string code");
    assert!(!code.is_empty(), "pairing code should not be empty");

    let (status, body) = request_json(
        router.clone(),
        Method::POST,
        "/pairing/redeem",
        None,
        Some(json!({ "code": code })),
    )
    .await;

    assert!(
        status.is_success(),
        "pairing code redemption should succeed, got {status}: {body}"
    );
    let token = body["token"]
        .as_str()
        .expect("pairing redemption response should include a string token");
    assert!(!token.is_empty(), "pairing token should not be empty");

    let (status, body) = request_json(
        router.clone(),
        Method::GET,
        "/capabilities",
        Some(token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("capabilities"));
    assert_eq!(body["mode"], json!("runner"));
    assert_eq!(body["tools"][4], json!("remote.diagnose.system"));

    let (status, body) = request_json(
        router,
        Method::POST,
        "/tool-call",
        Some(token),
        Some(json!({
            "call_id": "auth-smoke-diagnose",
            "name": "remote.diagnose.system",
            "arguments": {}
        })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], json!("tool_call"));
    assert_eq!(body["call_id"], json!("auth-smoke-diagnose"));
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["result"]["tool"], json!("remote.diagnose.system"));
    assert_eq!(body["result"]["status"], json!("ok"));
}

#[tokio::test]
async fn protected_runner_routes_reject_missing_bearer_token() {
    let router = build_router_with_pairing_manager(
        kai_runner::KaiRunner,
        PairingManager::deterministic(Duration::from_secs(60)),
    );
    let (status, body) = request_json(router, Method::GET, "/capabilities", None, None).await;

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
