use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, Uri, header},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::{
    AppState, AuditEntry, CommandPrepareRequest, DiagnosticRequest, HealthResponse, Message,
    MessagePart, SessionSummary, SshTarget,
    approvals::{ApprovalError, ApprovalService},
    diagnostics::{DiagnosticError, DiagnosticService, preset_diagnostics},
    events::{broadcast_event, event_stream},
    ssh_exec::{CommandRunner, SystemSshCommandRunner},
};

pub const SERVICE_NAME: &str = "deepseek-mobile-web-server";
const PROTOCOL: &str = "mobile-web-v1";

#[derive(Clone, Debug)]
pub struct MobileWebServerConfig {
    pub use_real_model: bool,
    pub static_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct AccessToken {
    value: Arc<str>,
}

impl std::fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccessToken")
            .field("token_present", &true)
            .field("token", &"<redacted>")
            .finish()
    }
}

impl AccessToken {
    fn new(value: String) -> Self {
        Self {
            value: Arc::from(value),
        }
    }

    fn matches(&self, candidate: &str) -> bool {
        self.value.as_ref() == candidate
    }
}

#[derive(Clone)]
struct RouterState {
    app: AppState,
    config: MobileWebServerConfig,
    runner: Arc<dyn CommandRunner>,
}

#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateSshTargetRequest {
    host: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    key_present: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct EventAccessTokenQuery {
    access_token: Option<String>,
}

pub fn app_router(state: AppState, use_real_model: bool) -> Router {
    app_router_with_config(
        state,
        MobileWebServerConfig {
            use_real_model,
            static_dir: None,
        },
    )
}

pub fn app_router_with_config(state: AppState, config: MobileWebServerConfig) -> Router {
    app_router_inner(state, config, Arc::new(SystemSshCommandRunner), None)
}

pub fn app_router_with_access_token(
    state: AppState,
    use_real_model: bool,
    access_token: String,
) -> Router {
    app_router_with_config_and_access_token(
        state,
        MobileWebServerConfig {
            use_real_model,
            static_dir: None,
        },
        access_token,
    )
}

pub fn app_router_with_config_and_access_token(
    state: AppState,
    config: MobileWebServerConfig,
    access_token: String,
) -> Router {
    app_router_inner(
        state,
        config,
        Arc::new(SystemSshCommandRunner),
        Some(AccessToken::new(access_token)),
    )
}

pub fn app_router_with_runner<R>(state: AppState, use_real_model: bool, runner: R) -> Router
where
    R: CommandRunner,
{
    app_router_inner(
        state,
        MobileWebServerConfig {
            use_real_model,
            static_dir: None,
        },
        Arc::new(runner),
        None,
    )
}

fn app_router_inner(
    state: AppState,
    config: MobileWebServerConfig,
    runner: Arc<dyn CommandRunner>,
    access_token: Option<AccessToken>,
) -> Router {
    let router_state = RouterState {
        app: state,
        config: config.clone(),
        runner,
    };

    let protected_routes = Router::new()
        .route("/event", get(events))
        .route("/api/ssh/target", get(get_ssh_target).put(put_ssh_target))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}/messages", get(list_messages))
        .route("/api/sessions/{id}/prompt", post(prompt_session))
        .route("/api/diagnostics/presets", get(list_diagnostic_presets))
        .route("/api/diagnostics/run", post(run_diagnostic))
        .route("/api/commands/prepare", post(prepare_command))
        .route("/api/approvals/{id}/respond", post(respond_approval))
        .route("/api/audit/recent", get(list_audit));

    let protected_routes = if let Some(access_token) = access_token {
        protected_routes.route_layer(middleware::from_fn(
            move |headers: HeaderMap, request, next| {
                require_access_token(headers, request, next, access_token.clone())
            },
        ))
    } else {
        protected_routes
    };

    let router = Router::new()
        .route("/health", get(health))
        .merge(protected_routes);

    let router = if let Some(static_dir) = config.static_dir {
        router.fallback_service(ServeDir::new(static_dir))
    } else {
        router
    };

    router.with_state(router_state)
}

async fn require_access_token(
    headers: HeaderMap,
    request: axum::extract::Request,
    next: middleware::Next,
    access_token: AccessToken,
) -> impl IntoResponse {
    if request_access_token_matches(&headers, request.uri(), &access_token) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "code": "unauthorized",
                "message": "missing or invalid mobile web access token"
            })),
        )
            .into_response()
    }
}

fn request_access_token_matches(
    headers: &HeaderMap,
    uri: &Uri,
    access_token: &AccessToken,
) -> bool {
    bearer_token(headers).is_some_and(|candidate| access_token.matches(candidate))
        || mobile_web_token(headers).is_some_and(|candidate| access_token.matches(candidate))
        || event_query_token(uri).is_some_and(|candidate| access_token.matches(&candidate))
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn mobile_web_token(headers: &HeaderMap) -> Option<&str> {
    headers.get("X-Mobile-Web-Token")?.to_str().ok()
}

fn event_query_token(uri: &Uri) -> Option<String> {
    if uri.path() != "/event" {
        return None;
    }

    Query::<EventAccessTokenQuery>::try_from_uri(uri)
        .ok()?
        .0
        .access_token
}

async fn health(State(state): State<RouterState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: SERVICE_NAME.to_string(),
        protocol: PROTOCOL.to_string(),
        model: if state.config.use_real_model {
            "real"
        } else {
            "mock"
        }
        .to_string(),
    })
}

async fn events(State(state): State<RouterState>) -> impl IntoResponse {
    event_stream(state.app)
}

async fn get_ssh_target(State(state): State<RouterState>) -> Json<SshTarget> {
    Json(state.app.ssh_target())
}

async fn put_ssh_target(
    State(state): State<RouterState>,
    Json(request): Json<UpdateSshTargetRequest>,
) -> Json<SshTarget> {
    let current = state.app.ssh_target();
    let target = SshTarget {
        host: request.host.unwrap_or(current.host),
        user: request.user.unwrap_or(current.user),
        port: request.port.unwrap_or(current.port),
        key_present: request.key_present.unwrap_or(current.key_present),
    };
    state.app.set_ssh_target(target.clone());
    broadcast_event(
        &state.app,
        "connection.updated",
        json!({
            "ssh_target": {
                "host": target.host,
                "user": target.user,
                "port": target.port,
                "key_present": target.key_present
            },
            "updated_at_ms": now_ms()
        }),
    );
    Json(state.app.ssh_target())
}

async fn list_sessions(State(state): State<RouterState>) -> Json<Vec<SessionSummary>> {
    Json(state.app.sessions())
}

async fn create_session(
    State(state): State<RouterState>,
    request: Option<Json<CreateSessionRequest>>,
) -> (StatusCode, Json<SessionSummary>) {
    let now = now_ms();
    let session = SessionSummary {
        id: Uuid::new_v4().to_string(),
        title: request
            .and_then(|Json(request)| request.title)
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| "Mobile SSH session".to_string()),
        created_at_ms: now,
        updated_at_ms: now,
    };

    state.app.upsert_session(session.clone());
    state.app.push_audit(AuditEntry {
        id: Uuid::new_v4().to_string(),
        session_id: Some(session.id.clone()),
        kind: "session.created".to_string(),
        created_at_ms: now,
        summary: "created session".to_string(),
        metadata: json!({}),
    });
    broadcast_event(
        &state.app,
        "session.updated",
        serde_json::to_value(&session).expect("session summary must serialize"),
    );

    (StatusCode::CREATED, Json(session))
}

#[derive(Debug, Deserialize)]
struct PromptRequest {
    input: Option<String>,
    content: Option<String>,
}

async fn prompt_session(
    State(state): State<RouterState>,
    Path(session_id): Path<String>,
    Json(request): Json<PromptRequest>,
) -> (StatusCode, Json<Message>) {
    let now = now_ms();
    let text = request
        .input
        .or(request.content)
        .unwrap_or_default()
        .trim()
        .to_string();
    let message = Message {
        id: Uuid::new_v4().to_string(),
        session_id,
        role: "user".to_string(),
        created_at_ms: now,
        parts: vec![MessagePart {
            id: Uuid::new_v4().to_string(),
            kind: "text".to_string(),
            text: Some(text),
            data: json!({}),
        }],
    };
    state.app.push_message(message.clone());
    broadcast_event(
        &state.app,
        "message.updated",
        serde_json::to_value(&message).expect("message must serialize"),
    );
    (StatusCode::CREATED, Json(message))
}

async fn list_messages(
    State(state): State<RouterState>,
    Path(session_id): Path<String>,
) -> Json<Vec<Message>> {
    Json(state.app.messages(&session_id))
}

async fn list_audit(State(state): State<RouterState>) -> Json<Vec<AuditEntry>> {
    Json(state.app.audit_recent())
}

async fn list_diagnostic_presets() -> Json<Vec<crate::DiagnosticPreset>> {
    Json(preset_diagnostics())
}

async fn run_diagnostic(
    State(state): State<RouterState>,
    Json(request): Json<DiagnosticRequest>,
) -> impl IntoResponse {
    let service = DiagnosticService::new(state.runner.clone());
    match service.run(&state.app, request) {
        Ok(run) => {
            broadcast_event(
                &state.app,
                "audit.updated",
                serde_json::to_value(&run.audit).expect("audit must serialize"),
            );
            (StatusCode::OK, Json(run.response)).into_response()
        }
        Err(DiagnosticError::UnknownDiagnostic { diagnostic }) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "code": "unknown_diagnostic",
                "message": format!("unknown diagnostic preset: {diagnostic}")
            })),
        )
            .into_response(),
        Err(DiagnosticError::Command { source, .. }) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "code": "ssh_failed",
                "message": source.to_string()
            })),
        )
            .into_response(),
    }
}

async fn prepare_command(
    State(state): State<RouterState>,
    Json(request): Json<CommandPrepareRequest>,
) -> (StatusCode, Json<crate::PendingApproval>) {
    let service = ApprovalService::new(state.runner.clone());
    let approval = service.prepare(&state.app, request);
    (StatusCode::CREATED, Json(approval))
}

async fn respond_approval(
    State(state): State<RouterState>,
    Path(approval_id): Path<String>,
    Json(request): Json<crate::ApprovalRespondRequest>,
) -> impl IntoResponse {
    let service = ApprovalService::new(state.runner.clone());
    match service.respond(&state.app, &approval_id, request) {
        Ok(response) => {
            if let Some(entry) = state.app.audit_recent().last().cloned() {
                broadcast_event(
                    &state.app,
                    "audit.updated",
                    serde_json::to_value(&entry).expect("audit must serialize"),
                );
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(ApprovalError::NotFound { approval_id }) => (
            StatusCode::CONFLICT,
            Json(json!({
                "code": "approval_not_found",
                "message": format!("approval not found or already consumed: {approval_id}")
            })),
        )
            .into_response(),
        Err(ApprovalError::UnsupportedResponse { response }) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "code": "unsupported_approval_response",
                "message": format!("unsupported approval response: {response}")
            })),
        )
            .into_response(),
        Err(ApprovalError::Command(source)) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "code": "ssh_failed",
                "message": source.to_string()
            })),
        )
            .into_response(),
    }
}

#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch")
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
