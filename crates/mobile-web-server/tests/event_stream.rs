use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use deepseek_mobile_web_server::{AppState, SshTarget, app_router};
use pretty_assertions::assert_eq;
use tower::ServiceExt;

#[tokio::test]
async fn event_stream_uses_sse_headers() {
    let state = AppState::new(SshTarget {
        host: "192.168.30.244".to_string(),
        user: "root".to_string(),
        port: 22,
        key_present: false,
    });

    let response = app_router(state, false)
        .oneshot(
            Request::builder()
                .uri("/event")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static("text/event-stream"))
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-cache"))
    );
}
