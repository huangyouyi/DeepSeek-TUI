use std::fs;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use deepseek_mobile_web_server::{
    AppState, MobileWebServerConfig, SshTarget, app_router_with_config,
};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn static_index_is_served_without_shadowing_api_routes() {
    let static_dir = std::env::temp_dir().join(format!("mobile-web-static-{}", Uuid::new_v4()));
    fs::create_dir_all(&static_dir).expect("static dir must be created");
    fs::write(static_dir.join("index.html"), "<html>mobile web</html>")
        .expect("index must be written");
    fs::write(static_dir.join("asset.txt"), "asset content").expect("asset must be written");

    let state = AppState::new(SshTarget {
        host: "127.0.0.1".to_string(),
        user: "tester".to_string(),
        port: 2222,
        key_present: false,
    });
    let app = app_router_with_config(
        state,
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: Some(static_dir.clone()),
        },
    );

    let index = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(index.status(), StatusCode::OK);
    let body = to_bytes(index.into_body(), 1024 * 1024)
        .await
        .expect("body must be readable");
    assert_eq!(&body[..], b"<html>mobile web</html>");

    for path in [
        "/web",
        "/web/sessions/session-123",
        "/debug",
        "/debug/tools/diagnostics",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request must build"),
            )
            .await
            .expect("request must complete");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body must be readable");
        assert_eq!(&body[..], b"<html>mobile web</html>");
    }

    let asset = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/asset.txt")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(asset.status(), StatusCode::OK);
    let body = to_bytes(asset.into_body(), 1024 * 1024)
        .await
        .expect("body must be readable");
    assert_eq!(&body[..], b"asset content");

    for path in ["/health", "/api/sessions"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request must build"),
            )
            .await
            .expect("request must complete");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body must be readable");
        assert_ne!(&body[..], b"<html>mobile web</html>");
    }

    let missing_api = app
        .oneshot(
            Request::builder()
                .uri("/api/not-a-static-route")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(missing_api.status(), StatusCode::NOT_FOUND);

    fs::remove_dir_all(static_dir).expect("static dir must be removed");
}
