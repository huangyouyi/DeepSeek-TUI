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

    let health = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(health.status(), StatusCode::OK);

    fs::remove_dir_all(static_dir).expect("static dir must be removed");
}
