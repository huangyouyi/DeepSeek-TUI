use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use deepseek_mobile_web_server::{
    AppState, AuditEntry, SshTarget, app_router, app_router_with_access_token,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

fn test_state() -> AppState {
    AppState::new(SshTarget {
        host: "192.168.30.244".to_string(),
        user: "root".to_string(),
        port: 22,
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
async fn health_returns_ok_contract() {
    let response = app_router(test_state(), false)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await,
        json!({
            "status": "ok",
            "service": "deepseek-mobile-web-server",
            "protocol": "mobile-web-v1",
            "model": "mock"
        })
    );
}

#[tokio::test]
async fn ssh_target_can_be_read_and_updated() {
    let state = test_state();
    let app = app_router(state, false);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/ssh/target")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await,
        json!({
            "host": "192.168.30.244",
            "user": "root",
            "port": 22,
            "key_present": false
        })
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/ssh/target")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "host": "10.0.0.42",
                        "user": "tester",
                        "port": 2200,
                        "key_present": true
                    })
                    .to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await,
        json!({
            "host": "10.0.0.42",
            "user": "tester",
            "port": 2200,
            "key_present": true
        })
    );
}

#[tokio::test]
async fn sessions_can_be_created_listed_and_messages_read() {
    let app = app_router(test_state(), false);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"title": "Phone SSH"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_response(response).await;
    assert_eq!(created["title"], "Phone SSH");
    let session_id = created["id"].as_str().expect("id must be a string");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(response.status(), StatusCode::OK);
    let sessions = json_response(response).await;
    assert_eq!(
        sessions.as_array().expect("sessions must be array").len(),
        1
    );
    assert_eq!(sessions[0]["id"], session_id);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/messages"))
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await, json!([]));
}

#[tokio::test]
async fn audit_recent_lists_seeded_entries() {
    let state = test_state();
    state.push_audit(AuditEntry {
        id: "audit-1".to_string(),
        session_id: Some("session-1".to_string()),
        kind: "session.created".to_string(),
        created_at_ms: 123,
        summary: "created session".to_string(),
        metadata: json!({"source": "test"}),
    });

    let response = app_router(state, false)
        .oneshot(
            Request::builder()
                .uri("/api/audit/recent")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await,
        json!([{
            "id": "audit-1",
            "session_id": "session-1",
            "kind": "session.created",
            "created_at_ms": 123,
            "summary": "created session",
            "metadata": {"source": "test"}
        }])
    );
}

#[tokio::test]
async fn http_api_diagnostic_presets_return_read_only_metadata() {
    let response = app_router(test_state(), false)
        .oneshot(
            Request::builder()
                .uri("/api/diagnostics/presets")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await,
        json!([
            {
                "key": "system_info",
                "label": "System info",
                "command": "uname -a",
                "requires_approval": false
            },
            {
                "key": "current_user",
                "label": "Current user",
                "command": "id",
                "requires_approval": false
            },
            {
                "key": "disk_usage",
                "label": "Disk usage",
                "command": "df -h",
                "requires_approval": false
            },
            {
                "key": "memory",
                "label": "Memory",
                "command": "free -m || cat /proc/meminfo",
                "requires_approval": false
            },
            {
                "key": "network",
                "label": "Network",
                "command": "ip addr || ifconfig",
                "requires_approval": false
            },
            {
                "key": "working_directory",
                "label": "Working directory",
                "command": "pwd",
                "requires_approval": false
            }
        ])
    );
}

#[tokio::test]
async fn http_api_default_router_leaves_json_api_unprotected() {
    let response = app_router(test_state(), false)
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await, json!([]));
}

#[tokio::test]
async fn http_api_protected_json_api_rejects_missing_token() {
    let response = app_router_with_access_token(test_state(), false, "secret-token".to_string())
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_response(response).await,
        json!({
            "code": "unauthorized",
            "message": "missing or invalid mobile web access token"
        })
    );
}

#[tokio::test]
async fn http_api_protected_event_rejects_missing_token() {
    let response = app_router_with_access_token(test_state(), false, "secret-token".to_string())
        .oneshot(
            Request::builder()
                .uri("/event")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_response(response).await,
        json!({
            "code": "unauthorized",
            "message": "missing or invalid mobile web access token"
        })
    );
}

#[tokio::test]
async fn http_api_protected_json_api_rejects_wrong_token() {
    let response = app_router_with_access_token(test_state(), false, "secret-token".to_string())
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .header(header::AUTHORIZATION, "Bearer wrong-token")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_response(response).await,
        json!({
            "code": "unauthorized",
            "message": "missing or invalid mobile web access token"
        })
    );
}

#[tokio::test]
async fn http_api_protected_json_api_accepts_bearer_token() {
    let response = app_router_with_access_token(test_state(), false, "secret-token".to_string())
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .header(header::AUTHORIZATION, "Bearer secret-token")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await, json!([]));
}

#[tokio::test]
async fn http_api_protected_json_api_accepts_x_mobile_web_token() {
    let response = app_router_with_access_token(test_state(), false, "secret-token".to_string())
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .header("X-Mobile-Web-Token", "secret-token")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_response(response).await, json!([]));
}

#[tokio::test]
async fn http_api_protected_health_remains_public() {
    let response = app_router_with_access_token(test_state(), false, "secret-token".to_string())
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_response(response).await,
        json!({
            "status": "ok",
            "service": "deepseek-mobile-web-server",
            "protocol": "mobile-web-v1",
            "model": "mock"
        })
    );
}
