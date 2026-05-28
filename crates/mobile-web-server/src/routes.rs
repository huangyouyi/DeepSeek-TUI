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
    routing::{delete, get, get_service, post},
};
use deepseek_mobile_agent_core::{
    remote_schema::{RemoteToolCall, RemoteToolName},
    risk::RiskAssessment,
    ssh::SshCommandRequest,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

use crate::{
    AppState, AuditEntry, CommandPrepareRequest, DiagnosticRequest, HealthResponse, Message,
    MessagePart, PendingApproval, SessionSummary, SshTarget,
    agent_model::{
        AgentContextMessage, AgentModel, AgentModelRequest, AgentToolCall, MockAgentModel,
    },
    agent_tool_policy::{AgentToolDecision, AgentToolPolicy},
    approvals::{ApprovalError, ApprovalService},
    diagnostics::{DiagnosticError, DiagnosticService, preset_command, preset_diagnostics},
    events::{broadcast_event, event_stream},
    ssh_exec::{CommandRunner, SshCommandOutput, SystemSshCommandRunner},
    types::{
        AgentExecutedTool, AgentTurnRequest, AgentTurnResponse, SshCheckResponse, ToolPartData,
        text_part, tool_part,
    },
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
    runner: Arc<dyn CommandRunner>,
    model: Arc<dyn AgentModel>,
}

#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateSessionTitleRequest {
    title: String,
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
    app_router_inner(
        state,
        config,
        Arc::new(SystemSshCommandRunner),
        Arc::new(MockAgentModel::new()),
        None,
    )
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
        Arc::new(MockAgentModel::new()),
        Some(AccessToken::new(access_token)),
    )
}

pub fn app_router_with_config_and_model(
    state: AppState,
    config: MobileWebServerConfig,
    model: Arc<dyn AgentModel>,
) -> Router {
    app_router_inner(state, config, Arc::new(SystemSshCommandRunner), model, None)
}

pub fn app_router_with_config_runner_and_model<R>(
    state: AppState,
    config: MobileWebServerConfig,
    runner: R,
    model: Arc<dyn AgentModel>,
) -> Router
where
    R: CommandRunner,
{
    app_router_inner(state, config, Arc::new(runner), model, None)
}

pub fn app_router_with_config_access_token_and_model(
    state: AppState,
    config: MobileWebServerConfig,
    access_token: String,
    model: Arc<dyn AgentModel>,
) -> Router {
    app_router_inner(
        state,
        config,
        Arc::new(SystemSshCommandRunner),
        model,
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
        Arc::new(MockAgentModel::new()),
        None,
    )
}

fn app_router_inner(
    state: AppState,
    config: MobileWebServerConfig,
    runner: Arc<dyn CommandRunner>,
    model: Arc<dyn AgentModel>,
    access_token: Option<AccessToken>,
) -> Router {
    let router_state = RouterState {
        app: state,
        runner,
        model,
    };

    let protected_routes = Router::new()
        .route("/event", get(events))
        .route("/api/ssh/target", get(get_ssh_target).put(put_ssh_target))
        .route("/api/ssh/check", post(check_ssh_target))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/sessions/{id}",
            delete(delete_session).patch(update_session_title),
        )
        .route("/api/sessions/{id}/messages", get(list_messages))
        .route("/api/sessions/{id}/prompt", post(prompt_session))
        .route("/api/sessions/{id}/agent-turn", post(agent_turn))
        .route(
            "/api/sessions/{id}/agent-turns/{turn_id}/stop",
            post(stop_agent_turn),
        )
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
        let index = ServeFile::new(static_dir.join("index.html"));
        router
            .route_service("/web", get_service(index.clone()))
            .route_service("/web/{*path}", get_service(index.clone()))
            .route_service("/debug", get_service(index.clone()))
            .route_service("/debug/{*path}", get_service(index))
            .fallback_service(ServeDir::new(static_dir))
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
    let model = state.model.redacted_status();
    Json(HealthResponse {
        status: "ok".to_string(),
        service: SERVICE_NAME.to_string(),
        protocol: PROTOCOL.to_string(),
        model: model.model_mode,
        capabilities: vec!["typed_message_parts".to_string()],
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

async fn check_ssh_target(State(state): State<RouterState>) -> Json<SshCheckResponse> {
    let target = state.app.ssh_target();
    let command = "true".to_string();
    let check_id = format!("ssh-check-{}", Uuid::new_v4());
    let request = SshCommandRequest {
        command: command.clone(),
        cwd: None,
        timeout_ms: Some(5_000),
        risk: RiskAssessment::low("ssh target reachability check"),
    };

    let response = match state.runner.run(&target, &request) {
        Ok(output) => {
            let reachable = output.exit_code == Some(0) && !output.timed_out;
            SshCheckResponse {
                status: if reachable {
                    "reachable".to_string()
                } else if output.timed_out {
                    "timed_out".to_string()
                } else {
                    "unreachable".to_string()
                },
                target,
                check_id,
                command,
                requires_approval: false,
                exit_code: output.exit_code,
                error_summary: check_error_summary(&output.stderr),
                duration_ms: Some(duration_ms(output.duration)),
                timed_out: output.timed_out,
            }
        }
        Err(source) => SshCheckResponse {
            status: "error".to_string(),
            target,
            check_id,
            command,
            requires_approval: false,
            exit_code: None,
            error_summary: Some(redact_text(&source.to_string())),
            duration_ms: None,
            timed_out: false,
        },
    };

    Json(response)
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

async fn delete_session(
    State(state): State<RouterState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let removed_pending_approvals = state
        .app
        .pending_approvals()
        .iter()
        .filter(|approval| approval.session_id == session_id)
        .count();
    if state.app.delete_session(&session_id) {
        let now = now_ms();
        state.app.push_audit(AuditEntry {
            id: Uuid::new_v4().to_string(),
            session_id: Some(session_id.clone()),
            kind: "session.deleted".to_string(),
            created_at_ms: now,
            summary: "deleted session".to_string(),
            metadata: json!({
                "removed_pending_approvals": removed_pending_approvals,
            }),
        });
        broadcast_event(
            &state.app,
            "session.deleted",
            json!({
                "id": session_id.clone(),
                "session_id": session_id,
                "removed_pending_approvals": removed_pending_approvals,
                "deleted_at_ms": now,
            }),
        );
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "code": "session_not_found",
                "message": format!("session not found: {session_id}")
            })),
        )
            .into_response()
    }
}

async fn update_session_title(
    State(state): State<RouterState>,
    Path(session_id): Path<String>,
    Json(request): Json<UpdateSessionTitleRequest>,
) -> impl IntoResponse {
    let title = request.title.trim().to_string();
    if title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "code": "empty_session_title",
                "message": "session title must not be empty"
            })),
        )
            .into_response();
    }

    let now = now_ms();
    match state.app.update_session_title(&session_id, title, now) {
        Some(session) => {
            state.app.push_audit(AuditEntry {
                id: Uuid::new_v4().to_string(),
                session_id: Some(session.id.clone()),
                kind: "session.updated".to_string(),
                created_at_ms: now,
                summary: "updated session title".to_string(),
                metadata: json!({
                    "title": session.title.clone(),
                    "updated_at_ms": session.updated_at_ms,
                }),
            });
            broadcast_event(
                &state.app,
                "session.updated",
                serde_json::to_value(&session).expect("session summary must serialize"),
            );
            (StatusCode::OK, Json(session)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "code": "session_not_found",
                "message": format!("session not found: {session_id}")
            })),
        )
            .into_response(),
    }
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

async fn agent_turn(
    State(state): State<RouterState>,
    Path(session_id): Path<String>,
    Json(request): Json<AgentTurnRequest>,
) -> impl IntoResponse {
    let turn_id = format!("turn-{}", Uuid::new_v4());
    let context = build_agent_context(&state.app, &session_id);
    let user_text = resolve_agent_turn_message(&state.app, &session_id, &request);
    let user_message = session_message(&session_id, "user", &user_text, json!({}));
    state.app.push_message(user_message.clone());
    broadcast_event(
        &state.app,
        "message.updated",
        serde_json::to_value(&user_message).expect("message must serialize"),
    );
    broadcast_event(
        &state.app,
        "assistant.started",
        json!({
            "session_id": session_id,
            "turn_id": turn_id,
        }),
    );

    let model_response = match state.model.complete(&AgentModelRequest {
        session_id: session_id.clone(),
        message: user_text.clone(),
        context: context.clone(),
    }) {
        Ok(response) => response,
        Err(error) => {
            let assistant_text = deterministic_model_fallback(&context, &error.to_string());
            let assistant = session_message(
                &session_id,
                "assistant",
                &assistant_text,
                json!({ "turn_id": turn_id }),
            );
            state.app.push_message(assistant.clone());
            state.app.push_audit(AuditEntry {
                id: format!("audit-{}", Uuid::new_v4()),
                session_id: Some(session_id.clone()),
                kind: "agent.model_fallback".to_string(),
                created_at_ms: now_ms(),
                summary: "model failed; deterministic fallback returned".to_string(),
                metadata: json!({
                    "turn_id": turn_id,
                    "error": redact_text(&error.to_string()),
                    "context_items": context.len(),
                }),
            });
            broadcast_event(
                &state.app,
                "message.updated",
                serde_json::to_value(&assistant).expect("message must serialize"),
            );
            broadcast_event(
                &state.app,
                "assistant.completed",
                json!({
                    "session_id": session_id,
                    "turn_id": turn_id,
                    "status": "model_fallback",
                }),
            );
            return Json(AgentTurnResponse {
                session_id,
                turn_id,
                status: "model_fallback".to_string(),
                assistant_text,
                executed_tools: Vec::new(),
                pending_approvals: Vec::new(),
            })
            .into_response();
        }
    };

    let policy = AgentToolPolicy;
    let approval_service = ApprovalService::new(state.runner.clone());
    let mut assistant_text = model_response.assistant_text;
    let model_tool_calls =
        if is_agent_meta_question(&user_text) && !model_response.tool_calls.is_empty() {
            assistant_text = agent_meta_answer(&user_text);
            Vec::new()
        } else {
            model_response.tool_calls
        };
    let mut assistant = Message {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
        role: "assistant".to_string(),
        created_at_ms: now_ms(),
        parts: Vec::new(),
    };
    state.app.push_message(assistant.clone());
    let mut executed_tools = Vec::new();
    let mut pending_approvals = Vec::new();
    let mut rejected_tools = Vec::new();

    for tool_call in model_tool_calls {
        let remote_call = match remote_tool_call(tool_call) {
            Ok(call) => call,
            Err(reason) => {
                rejected_tools.push(reason);
                continue;
            }
        };
        let tool_call_id = remote_call.call_id.clone();

        match policy.classify(&remote_call) {
            AgentToolDecision::RunLowRisk(command) => {
                let part_id = format!("part-tool-{}", Uuid::new_v4());
                let running_part = remote_shell_tool_part(RemoteShellToolPartInput {
                    id: part_id.clone(),
                    turn_id: turn_id.clone(),
                    tool_call_id,
                    command: command.command.clone(),
                    status: "running",
                    requires_approval: false,
                    approval_id: None,
                    output: None,
                });
                let _ =
                    state
                        .app
                        .upsert_message_part(&session_id, &assistant.id, running_part.clone());
                upsert_local_part(&mut assistant, running_part.clone());
                broadcast_message_part_updated(
                    &state.app,
                    &session_id,
                    &assistant.id,
                    &running_part,
                );
                broadcast_event(
                    &state.app,
                    "agent.tool.proposed",
                    json!({
                        "session_id": session_id,
                        "turn_id": turn_id,
                        "tool": "remote.shell.exec",
                        "command": command.command,
                        "requires_approval": false,
                    }),
                );
                match run_agent_shell_command(&state, &session_id, &turn_id, &command.command) {
                    Ok(output) => {
                        let status = if output.timed_out {
                            "timed_out"
                        } else {
                            "completed"
                        };
                        let completed_part = remote_shell_tool_part(RemoteShellToolPartInput {
                            id: part_id,
                            turn_id: turn_id.clone(),
                            tool_call_id: running_part.data["tool_call_id"]
                                .as_str()
                                .unwrap_or("agent-call-unknown")
                                .to_string(),
                            command: command.command.clone(),
                            status,
                            requires_approval: false,
                            approval_id: None,
                            output: Some(&output),
                        });
                        let _ = state.app.upsert_message_part(
                            &session_id,
                            &assistant.id,
                            completed_part.clone(),
                        );
                        upsert_local_part(&mut assistant, completed_part.clone());
                        broadcast_message_part_updated(
                            &state.app,
                            &session_id,
                            &assistant.id,
                            &completed_part,
                        );
                        assistant_text = summarize_shell_output(&command.command, &output);
                        executed_tools.push(AgentExecutedTool {
                            tool: "remote.shell.exec".to_string(),
                            command: command.command,
                            requires_approval: false,
                            exit_code: output.exit_code,
                            status: status.to_string(),
                        });
                    }
                    Err(error) => {
                        let failed_part = remote_shell_tool_part(RemoteShellToolPartInput {
                            id: part_id,
                            turn_id: turn_id.clone(),
                            tool_call_id: running_part.data["tool_call_id"]
                                .as_str()
                                .unwrap_or("agent-call-unknown")
                                .to_string(),
                            command: command.command.clone(),
                            status: "failed",
                            requires_approval: false,
                            approval_id: None,
                            output: None,
                        });
                        let _ = state.app.upsert_message_part(
                            &session_id,
                            &assistant.id,
                            failed_part.clone(),
                        );
                        upsert_local_part(&mut assistant, failed_part.clone());
                        broadcast_message_part_updated(
                            &state.app,
                            &session_id,
                            &assistant.id,
                            &failed_part,
                        );
                        assistant_text = format!("SSH command failed: {error}");
                        executed_tools.push(AgentExecutedTool {
                            tool: "remote.shell.exec".to_string(),
                            command: command.command,
                            requires_approval: false,
                            exit_code: None,
                            status: "failed".to_string(),
                        });
                    }
                }
            }
            AgentToolDecision::RequireApproval(command) => {
                if request.auto_approve
                    || state.app.is_session_allowed(
                        &session_id,
                        &command.command,
                        command.cwd.as_deref(),
                    )
                {
                    let part_id = format!("part-tool-{}", Uuid::new_v4());
                    let running_part = remote_shell_tool_part(RemoteShellToolPartInput {
                        id: part_id.clone(),
                        turn_id: turn_id.clone(),
                        tool_call_id,
                        command: command.command.clone(),
                        status: "running",
                        requires_approval: false,
                        approval_id: None,
                        output: None,
                    });
                    let _ = state.app.upsert_message_part(
                        &session_id,
                        &assistant.id,
                        running_part.clone(),
                    );
                    upsert_local_part(&mut assistant, running_part.clone());
                    broadcast_message_part_updated(
                        &state.app,
                        &session_id,
                        &assistant.id,
                        &running_part,
                    );
                    match run_agent_shell_command_with_policy(
                        &state,
                        &session_id,
                        &turn_id,
                        &command.command,
                        command.cwd.as_deref(),
                        RiskAssessment::high(if request.auto_approve {
                            "server-side auto approval requested"
                        } else {
                            "server-side session approval grant"
                        }),
                        false,
                    ) {
                        Ok(output) => {
                            let status = if output.timed_out {
                                "timed_out"
                            } else {
                                "completed"
                            };
                            let completed_part = remote_shell_tool_part(RemoteShellToolPartInput {
                                id: part_id,
                                turn_id: turn_id.clone(),
                                tool_call_id: running_part.data["tool_call_id"]
                                    .as_str()
                                    .unwrap_or("agent-call-unknown")
                                    .to_string(),
                                command: command.command.clone(),
                                status,
                                requires_approval: false,
                                approval_id: None,
                                output: Some(&output),
                            });
                            let _ = state.app.upsert_message_part(
                                &session_id,
                                &assistant.id,
                                completed_part.clone(),
                            );
                            upsert_local_part(&mut assistant, completed_part.clone());
                            broadcast_message_part_updated(
                                &state.app,
                                &session_id,
                                &assistant.id,
                                &completed_part,
                            );
                            assistant_text = summarize_shell_output(&command.command, &output);
                            executed_tools.push(AgentExecutedTool {
                                tool: "remote.shell.exec".to_string(),
                                command: command.command.clone(),
                                requires_approval: false,
                                exit_code: output.exit_code,
                                status: status.to_string(),
                            });
                            state.app.push_audit(agent_session_allow_audit(
                                &session_id,
                                &turn_id,
                                &command.command,
                                command.cwd.as_deref(),
                                &output,
                            ));
                        }
                        Err(error) => {
                            let failed_part = remote_shell_tool_part(RemoteShellToolPartInput {
                                id: part_id,
                                turn_id: turn_id.clone(),
                                tool_call_id: running_part.data["tool_call_id"]
                                    .as_str()
                                    .unwrap_or("agent-call-unknown")
                                    .to_string(),
                                command: command.command.clone(),
                                status: "failed",
                                requires_approval: false,
                                approval_id: None,
                                output: None,
                            });
                            let _ = state.app.upsert_message_part(
                                &session_id,
                                &assistant.id,
                                failed_part.clone(),
                            );
                            upsert_local_part(&mut assistant, failed_part.clone());
                            broadcast_message_part_updated(
                                &state.app,
                                &session_id,
                                &assistant.id,
                                &failed_part,
                            );
                            assistant_text = format!("SSH command failed: {error}");
                            executed_tools.push(AgentExecutedTool {
                                tool: "remote.shell.exec".to_string(),
                                command: command.command.clone(),
                                requires_approval: false,
                                exit_code: None,
                                status: "failed".to_string(),
                            });
                        }
                    }
                    continue;
                }
                let approval = approval_service.prepare(
                    &state.app,
                    CommandPrepareRequest {
                        session_id: session_id.clone(),
                        command: command.command,
                        cwd: command.cwd,
                    },
                );
                approval.set_agent_turn_id(turn_id.clone());
                let pending_part = remote_shell_tool_part(RemoteShellToolPartInput {
                    id: format!("part-tool-{}", Uuid::new_v4()),
                    turn_id: turn_id.clone(),
                    tool_call_id,
                    command: approval.command.clone(),
                    status: "pending_approval",
                    requires_approval: true,
                    approval_id: Some(approval.id.clone()),
                    output: None,
                });
                approval.set_agent_tool_part(assistant.id.clone(), pending_part.id.clone());
                let _ =
                    state
                        .app
                        .upsert_message_part(&session_id, &assistant.id, pending_part.clone());
                upsert_local_part(&mut assistant, pending_part.clone());
                broadcast_message_part_updated(
                    &state.app,
                    &session_id,
                    &assistant.id,
                    &pending_part,
                );
                pending_approvals.push(approval);
                assistant_text =
                    "I need approval before running the requested command.".to_string();
            }
            AgentToolDecision::Reject { reason } => {
                rejected_tools.push(reason);
            }
        }
    }

    if !rejected_tools.is_empty() {
        assistant_text = format!(
            "I could not run one or more model-proposed tools: {}",
            rejected_tools.join("; ")
        );
    } else if pending_approvals.is_empty() && !executed_tools.is_empty() {
        assistant_text = final_agent_turn_text(&state, &session_id, &turn_id);
        state.app.push_audit(agent_turn_finalized_audit(
            &session_id,
            &turn_id,
            json!({ "source": "inline_tool_result" }),
        ));
    }

    let status = if !pending_approvals.is_empty() {
        "waiting_for_approval"
    } else {
        "completed"
    }
    .to_string();
    let text = text_part(&assistant_text, json!({ "turn_id": turn_id }));
    let _ = state
        .app
        .push_message_part(&session_id, &assistant.id, text.clone());
    assistant.parts.push(text);
    broadcast_event(
        &state.app,
        "message.updated",
        serde_json::to_value(&assistant).expect("message must serialize"),
    );
    broadcast_event(
        &state.app,
        "assistant.completed",
        json!({
            "session_id": session_id,
            "turn_id": turn_id,
            "status": status,
        }),
    );

    Json(AgentTurnResponse {
        session_id,
        turn_id,
        status,
        assistant_text,
        executed_tools,
        pending_approvals,
    })
    .into_response()
}

async fn list_messages(
    State(state): State<RouterState>,
    Path(session_id): Path<String>,
) -> Json<Vec<Message>> {
    Json(state.app.messages(&session_id))
}

async fn stop_agent_turn(
    State(state): State<RouterState>,
    Path((session_id, turn_id)): Path<(String, String)>,
) -> Json<AgentTurnResponse> {
    let stopped = state.app.remove_pending_approvals_for_agent_turn(&turn_id);
    let assistant_text = if stopped.is_empty() {
        "已请求停止本轮任务。当前没有等待授权的命令；如果命令已经开始执行，服务器会等待该命令自行结束或超时。".to_string()
    } else {
        format!(
            "已停止本轮任务，{} 个等待授权的命令不会继续执行。",
            stopped.len()
        )
    };
    let assistant = session_message(
        &session_id,
        "assistant",
        &assistant_text,
        json!({
            "turn_id": turn_id,
            "status": "stopped",
            "stopped_pending_approvals": stopped.len(),
        }),
    );
    state.app.push_message(assistant.clone());
    state.app.push_audit(AuditEntry {
        id: format!("audit-{}", Uuid::new_v4()),
        session_id: Some(session_id.clone()),
        kind: "agent.turn_stopped".to_string(),
        created_at_ms: now_ms(),
        summary: "agent turn stopped by user".to_string(),
        metadata: json!({
            "turn_id": turn_id,
            "stopped_pending_approvals": stopped.len(),
            "note": "stop removes pending approvals; it does not claim to kill an already running OS process"
        }),
    });
    broadcast_event(
        &state.app,
        "message.updated",
        serde_json::to_value(&assistant).expect("message must serialize"),
    );
    broadcast_event(
        &state.app,
        "assistant.completed",
        json!({
            "session_id": session_id,
            "turn_id": turn_id,
            "status": "stopped",
        }),
    );

    Json(AgentTurnResponse {
        session_id,
        turn_id,
        status: "stopped".to_string(),
        assistant_text,
        executed_tools: Vec::new(),
        pending_approvals: Vec::new(),
    })
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
        Ok(mut response) => {
            if response.status == "approved"
                && let Some(final_text) = finalize_approved_agent_turn(&state, &response.approval)
            {
                response.result["summary"] = json!(final_text);
                response.result["assistant_text"] = json!(final_text);
            }
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
                "message": redact_text(&source.to_string())
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

fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn check_error_summary(stderr: &str) -> Option<String> {
    let summary = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(redact_text)?;

    if summary.is_empty() {
        None
    } else {
        Some(summary)
    }
}

fn session_message(session_id: &str, role: &str, text: &str, data: serde_json::Value) -> Message {
    Message {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: role.to_string(),
        created_at_ms: now_ms(),
        parts: vec![text_part(text, data)],
    }
}

fn resolve_agent_turn_message(
    state: &AppState,
    session_id: &str,
    request: &AgentTurnRequest,
) -> String {
    let message = request.message.trim();
    if !message.is_empty() {
        return message.to_string();
    }

    if request.mode.as_deref() == Some("retry")
        && let Some(retry_turn_id) = request.retry_turn_id.as_deref()
        && let Some(text) = user_message_for_turn(state, session_id, retry_turn_id)
    {
        return text;
    }

    match request.mode.as_deref() {
        Some("retry") => {
            last_user_message(state, session_id).unwrap_or_else(|| "请重试上一轮请求。".to_string())
        }
        Some("continue") => "继续分析上一轮结果。".to_string(),
        _ => String::new(),
    }
}

fn user_message_for_turn(state: &AppState, session_id: &str, turn_id: &str) -> Option<String> {
    let messages = state.messages(session_id);
    let turn_index = messages.iter().position(|message| {
        message.role == "assistant"
            && message.parts.iter().any(|part| {
                part.data.get("turn_id").and_then(serde_json::Value::as_str) == Some(turn_id)
                    || (part.kind == "tool"
                        && part
                            .data
                            .get("agent_turn_id")
                            .and_then(serde_json::Value::as_str)
                            == Some(turn_id))
            })
    })?;
    messages[..turn_index]
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .and_then(message_text)
}

fn last_user_message(state: &AppState, session_id: &str) -> Option<String> {
    state
        .messages(session_id)
        .into_iter()
        .rev()
        .find(|message| message.role == "user")
        .and_then(|message| message_text(&message))
}

fn build_agent_context(state: &AppState, session_id: &str) -> Vec<AgentContextMessage> {
    const MAX_CONTEXT_ITEMS: usize = 10;
    state
        .messages(session_id)
        .into_iter()
        .flat_map(context_items_from_message)
        .rev()
        .take(MAX_CONTEXT_ITEMS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn context_items_from_message(message: Message) -> Vec<AgentContextMessage> {
    let mut items = Vec::new();
    for part in message.parts {
        if part.kind == "text" {
            if let Some(text) = part.text.map(|text| compact_context_text("message", &text))
                && !text.is_empty()
            {
                items.push(AgentContextMessage {
                    role: context_role(&message.role),
                    content: text,
                });
            }
            continue;
        }

        if part.kind == "tool"
            && let Ok(data) = serde_json::from_value::<ToolPartData>(part.data)
        {
            items.push(AgentContextMessage {
                role: "assistant".to_string(),
                content: compact_tool_context(&data),
            });
        }
    }
    items
}

fn message_text(message: &Message) -> Option<String> {
    message
        .parts
        .iter()
        .filter(|part| part.kind == "text")
        .filter_map(|part| part.text.as_deref())
        .map(str::trim)
        .find(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn context_role(role: &str) -> String {
    match role {
        "user" => "user".to_string(),
        _ => "assistant".to_string(),
    }
}

fn compact_tool_context(data: &ToolPartData) -> String {
    let mut lines = vec![format!(
        "Tool remote.shell.exec `{}` status {}.",
        data.command, data.status
    )];
    if let Some(exit_code) = data.exit_code {
        lines.push(format!("exit_code: {exit_code}"));
    }
    if let Some(stdout) = data.stdout.as_deref().filter(|text| !text.is_empty()) {
        lines.push(compact_context_text("stdout", stdout));
    }
    if let Some(stderr) = data.stderr.as_deref().filter(|text| !text.is_empty()) {
        lines.push(compact_context_text("stderr", stderr));
    }
    lines.join("\n")
}

fn compact_context_text(label: &str, text: &str) -> String {
    const MAX_INLINE_CHARS: usize = 700;
    const EDGE_CHARS: usize = 240;
    let clean = text.trim();
    if clean.is_empty() {
        return String::new();
    }
    let line_count = clean.lines().count().max(1);
    let char_count = clean.chars().count();
    if char_count <= MAX_INLINE_CHARS {
        return format!("{label} ({line_count} lines, {char_count} chars):\n{clean}");
    }

    let head = clean.chars().take(EDGE_CHARS).collect::<String>();
    let tail = clean
        .chars()
        .rev()
        .take(EDGE_CHARS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!(
        "{label} summarized ({line_count} lines, {char_count} chars):\n[head]\n{head}\n[tail]\n{tail}"
    )
}

fn deterministic_model_fallback(context: &[AgentContextMessage], error: &str) -> String {
    let mut text = format!(
        "模型暂时不可用，已切换为确定性总结。错误摘要：{}",
        redact_text(error)
    );
    if context.is_empty() {
        text.push_str(
            "\n当前会话还没有可用于继续分析的历史结果。你可以重试，或先让 Agent 执行一次诊断。",
        );
        return text;
    }

    text.push_str("\n我会基于当前会话已有记录继续：");
    for (index, item) in context.iter().rev().take(3).enumerate() {
        text.push_str(&format!(
            "\n{}. {}",
            index + 1,
            compact_context_text("context", &item.content)
                .lines()
                .take(4)
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    text
}

fn finalize_approved_agent_turn(state: &RouterState, approval: &PendingApproval) -> Option<String> {
    let agent_turn_id = approval.agent_turn_id()?;
    let has_remaining_turn_approvals = state
        .app
        .pending_approvals()
        .into_iter()
        .any(|pending| pending.agent_turn_id().as_deref() == Some(agent_turn_id.as_str()));
    if has_remaining_turn_approvals {
        return None;
    }

    let final_text = final_agent_turn_text(state, &approval.session_id, &agent_turn_id);

    let message = session_message(
        &approval.session_id,
        "assistant",
        &final_text,
        json!({
            "agent_turn_id": agent_turn_id,
            "status": "completed",
            "finalized_from": "tool_result",
        }),
    );
    state.app.push_message(message.clone());
    state.app.push_audit(agent_turn_finalized_audit(
        &approval.session_id,
        &agent_turn_id,
        json!({
            "source": "approved_tool_result",
            "approval_id": approval.id,
            "command": redact_text(&approval.command),
        }),
    ));
    broadcast_event(
        &state.app,
        "message.updated",
        serde_json::to_value(&message).expect("message must serialize"),
    );
    broadcast_event(
        &state.app,
        "assistant.completed",
        json!({
            "session_id": approval.session_id,
            "turn_id": agent_turn_id,
            "status": "completed",
        }),
    );
    Some(final_text)
}

fn final_agent_turn_text(state: &RouterState, session_id: &str, turn_id: &str) -> String {
    let context = build_agent_context(&state.app, session_id);
    let final_message = "根据刚才远程命令结果，给出最终结论。不要复述完整 stdout/stderr；如果命令退出码非 0 或输出不完整，请说明哪些结果可信、哪些检查没有完成，以及建议下一步。";
    match state.model.complete(&AgentModelRequest {
        session_id: session_id.to_string(),
        message: final_message.to_string(),
        context: context.clone(),
    }) {
        Ok(response) if !response.assistant_text.trim().is_empty() => response.assistant_text,
        Ok(_) => deterministic_tool_result_fallback(&context),
        Err(error) => {
            let mut fallback = deterministic_model_fallback(&context, &error.to_string());
            fallback.push_str(&format!("\n关联轮次：{turn_id}。"));
            fallback
        }
    }
}

fn agent_turn_finalized_audit(session_id: &str, turn_id: &str, mut metadata: Value) -> AuditEntry {
    metadata["turn_id"] = json!(turn_id);
    AuditEntry {
        id: format!("audit-{}", Uuid::new_v4()),
        session_id: Some(session_id.to_string()),
        kind: "agent.turn_finalized".to_string(),
        created_at_ms: now_ms(),
        summary: "agent turn finalized from remote tool result".to_string(),
        metadata,
    }
}

fn deterministic_tool_result_fallback(context: &[AgentContextMessage]) -> String {
    let tool_context = context
        .iter()
        .rev()
        .find(|item| item.content.contains("Tool remote.shell.exec"));
    if let Some(item) = tool_context {
        let has_nonzero = item
            .content
            .lines()
            .any(|line| line.trim_start().starts_with("exit_code:") && !line.ends_with(" 0"));
        if has_nonzero {
            return "结论：远程命令已有部分输出，但退出码非 0，结果可能不完整。请根据工具输出中已返回的段落判断现状，并补跑缺失的检查。".to_string();
        }
    }
    "结论：远程命令已执行完成，已基于工具结果生成本轮结论。".to_string()
}

fn is_agent_meta_question(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let asks_missing_summary = text.contains("没有得到汇总")
        || text.contains("没有给")
        || text.contains("为何没有")
        || text.contains("为什么没有")
        || lower.contains("why no summary");
    asks_missing_summary
        && (text.contains("汇总")
            || text.contains("总结")
            || text.contains("结论")
            || lower.contains("summary"))
}

fn agent_meta_answer(text: &str) -> String {
    if text.contains("没有得到汇总") || text.contains("汇总") || text.contains("结论") {
        return "没有得到汇总的原因是上一轮远程命令结果没有重新回到 Agent 做最终分析，只展示了工具执行记录。后续会把工具结果作为上下文交给 Agent，再由 Agent 给出结论；这类产品流程问题不会再触发新的远程命令。".to_string();
    }
    "这是关于当前 Agent 流程的问题，不需要执行新的远程命令。我会基于已有会话记录解释原因。"
        .to_string()
}

fn remote_shell_tool_part(input: RemoteShellToolPartInput<'_>) -> MessagePart {
    let mut part = tool_part(ToolPartData {
        turn_id: input.turn_id.clone(),
        agent_turn_id: input.turn_id,
        tool_call_id: input.tool_call_id,
        tool: "remote.shell.exec".to_string(),
        title: "Remote shell command".to_string(),
        status: input.status.to_string(),
        requires_approval: input.requires_approval,
        command: input.command.clone(),
        input: json!({ "command": input.command }),
        approval_id: input.approval_id,
        output: input.output.map(|output| output.stdout.clone()),
        stdout: input.output.map(|output| output.stdout.clone()),
        stderr: input.output.map(|output| output.stderr.clone()),
        exit_code: input.output.and_then(|output| output.exit_code),
        duration_ms: input.output.map(|output| duration_ms(output.duration)),
        timed_out: input.output.map(|output| output.timed_out),
    });
    part.id = input.id;
    part
}

struct RemoteShellToolPartInput<'a> {
    id: String,
    turn_id: String,
    tool_call_id: String,
    command: String,
    status: &'a str,
    requires_approval: bool,
    approval_id: Option<String>,
    output: Option<&'a SshCommandOutput>,
}

fn upsert_local_part(message: &mut Message, part: MessagePart) {
    if let Some(existing) = message
        .parts
        .iter_mut()
        .find(|existing| existing.id == part.id)
    {
        *existing = part;
    } else {
        message.parts.push(part);
    }
}

fn broadcast_message_part_updated(
    state: &AppState,
    session_id: &str,
    message_id: &str,
    part: &MessagePart,
) {
    broadcast_event(
        state,
        "message.part.updated",
        json!({
            "session_id": session_id,
            "message_id": message_id,
            "part": part,
        }),
    );
}

fn remote_tool_call(tool_call: AgentToolCall) -> Result<RemoteToolCall, String> {
    let name = RemoteToolName::parse(&tool_call.tool)
        .map_err(|_| format!("unsupported agent tool: {}", tool_call.tool))?;
    Ok(RemoteToolCall {
        call_id: format!("agent-call-{}", Uuid::new_v4()),
        name,
        arguments: json!({ "command": tool_call.command }),
    })
}

fn run_agent_shell_command(
    state: &RouterState,
    session_id: &str,
    turn_id: &str,
    command: &str,
) -> Result<SshCommandOutput, crate::ssh_exec::CommandRunError> {
    run_agent_shell_command_with_policy(
        state,
        session_id,
        turn_id,
        command,
        None,
        RiskAssessment::low("agent read-only diagnostic command"),
        false,
    )
}

fn run_agent_shell_command_with_policy(
    state: &RouterState,
    session_id: &str,
    turn_id: &str,
    command: &str,
    cwd: Option<&str>,
    risk: RiskAssessment,
    requires_approval: bool,
) -> Result<SshCommandOutput, crate::ssh_exec::CommandRunError> {
    broadcast_event(
        &state.app,
        "tool.started",
        json!({
            "session_id": session_id,
            "turn_id": turn_id,
            "tool": "remote.shell.exec",
            "command": command,
            "requires_approval": requires_approval,
        }),
    );
    let output = state.runner.run(
        &state.app.ssh_target(),
        &SshCommandRequest {
            command: command.to_string(),
            cwd: cwd.map(ToOwned::to_owned),
            timeout_ms: None,
            risk,
        },
    )?;
    if !output.stdout.is_empty() {
        broadcast_event(
            &state.app,
            "tool.stdout",
            json!({
                "session_id": session_id,
                "turn_id": turn_id,
                "stdout": output.stdout,
                "text": output.stdout,
            }),
        );
    }
    if !output.stderr.is_empty() {
        broadcast_event(
            &state.app,
            "tool.stderr",
            json!({
                "session_id": session_id,
                "turn_id": turn_id,
                "stderr": output.stderr,
                "text": output.stderr,
            }),
        );
    }
    broadcast_event(
        &state.app,
        if output.timed_out {
            "agent.tool.failed"
        } else {
            "agent.tool.completed"
        },
        json!({
            "session_id": session_id,
            "turn_id": turn_id,
            "tool": "remote.shell.exec",
            "command": command,
            "exit_code": output.exit_code,
            "duration_ms": duration_ms(output.duration),
            "timed_out": output.timed_out,
        }),
    );
    Ok(output)
}

fn agent_session_allow_audit(
    session_id: &str,
    turn_id: &str,
    command: &str,
    cwd: Option<&str>,
    output: &SshCommandOutput,
) -> AuditEntry {
    AuditEntry {
        id: format!("audit-{}", Uuid::new_v4()),
        session_id: Some(session_id.to_string()),
        kind: "approval.auto_approved".to_string(),
        created_at_ms: now_ms(),
        summary: "server-side session approval allowed command".to_string(),
        metadata: json!({
            "action": "approved",
            "response": "approve_session",
            "scope": "session",
            "turn_id": turn_id,
            "command": redact_text(command),
            "cwd": cwd.map(redact_text),
            "risk": {
                "level": "high",
                "reason": "server-side session approval grant",
            },
            "target": "remote.shell.exec",
            "target_label": "Remote shell command",
            "result": {
                "exit_code": output.exit_code,
                "duration_ms": duration_ms(output.duration),
                "timed_out": output.timed_out,
            }
        }),
    }
}

fn summarize_shell_output(command: &str, output: &SshCommandOutput) -> String {
    let exit_code = output
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    if let Some(summary) = diagnostic_conclusion(command, output, &exit_code) {
        return summary;
    }

    let mut summary =
        format!("结论：命令已执行完成，退出码 {exit_code}。\n\n详细输出已保留在本条工具结果中。");
    if output.timed_out {
        summary.push_str("\n注意：命令执行超时，结果可能不完整。");
    }
    summary
}

fn diagnostic_conclusion(
    command: &str,
    output: &SshCommandOutput,
    exit_code: &str,
) -> Option<String> {
    let stdout = output.stdout.trim();
    let stderr = output.stderr.trim();
    let timed_out = output.timed_out;
    let detail_suffix = "\n\n详细输出已保留在本条工具结果中。";
    let conclusion = if command == preset_command("system_info")? {
        let system_line = first_non_empty_line(stdout).unwrap_or("未获取到系统信息");
        format!("结论：已获取当前系统信息。\n系统摘要：{system_line}{detail_suffix}")
    } else if command == preset_command("network")? {
        if stdout.contains("inet ") || stdout.contains("UP") {
            format!("结论：网络接口信息已获取，至少存在可见网络接口。{detail_suffix}")
        } else {
            format!(
                "结论：未从输出中看到明确的网络接口地址，需要继续检查链路或权限。{detail_suffix}"
            )
        }
    } else if command == preset_command("dns")? {
        if stdout.contains("has address")
            || stdout.contains("Address")
            || stdout.contains("nameserver")
            || stdout.contains("deepseek.com")
        {
            format!("结论：DNS 查询或解析配置有返回，DNS 基础状态看起来正常。{detail_suffix}")
        } else {
            format!(
                "结论：DNS 没有返回明确解析结果，需要检查 nameserver 和上游连通性。{detail_suffix}"
            )
        }
    } else if command == preset_command("disk_usage")? {
        let findings = disk_usage_findings(stdout);
        if findings.actionable_full {
            format!("结论：磁盘空间需要关注，存在使用率很高的可写文件系统。{detail_suffix}")
        } else if findings.ignored_rom_full {
            format!(
                "结论：未看到明显满盘迹象。/rom 是只读系统镜像，显示 100% 通常是 OpenWrt 正常现象；请重点关注 /overlay、/ 和外接挂载点。{detail_suffix}"
            )
        } else {
            format!("结论：未看到明显满盘迹象。{detail_suffix}")
        }
    } else if command == preset_command("memory")? || command == preset_command("cpu_memory")? {
        if stdout.contains("Mem:") || stdout.contains("load average") || stdout.contains("MemTotal")
        {
            format!(
                "结论：已获取内存/CPU 负载信息，可继续根据本条工具结果判断异常进程。{detail_suffix}"
            )
        } else {
            format!("结论：未获取到完整内存/CPU 信息，可能缺少相关系统工具。{detail_suffix}")
        }
    } else if command == preset_command("services")? {
        if stdout.is_empty() && !stderr.is_empty() {
            format!("结论：服务列表未正常返回，可能不是 systemd 环境或权限不足。{detail_suffix}")
        } else {
            format!("结论：已获取运行中的服务/进程列表。{detail_suffix}")
        }
    } else if command == preset_command("docker")? {
        if stderr.contains("not found")
            || stderr.contains("Cannot connect")
            || stdout.contains("Cannot connect")
        {
            format!("结论：Docker 不可用或 daemon 未连接。{detail_suffix}")
        } else if stdout.is_empty() {
            format!("结论：Docker 没有返回容器/daemon 信息，可能未安装或没有运行。{detail_suffix}")
        } else {
            format!("结论：Docker 命令有返回，容器/daemon 状态已记录。{detail_suffix}")
        }
    } else if command == preset_command("openwrt_network")? {
        if stdout.contains("OpenWrt") || stdout.contains("default") || stdout.contains("nameserver")
        {
            format!(
                "结论：已获取路由器/OpenWrt 基础网络信息，可查看默认路由、接口和 DNS。{detail_suffix}"
            )
        } else {
            format!(
                "结论：未看到完整路由器网络信息，可能不是 OpenWrt 或缺少 ubus/ip 输出。{detail_suffix}"
            )
        }
    } else if command == preset_command("logs")? {
        if contains_log_warning(stdout) || contains_log_warning(stderr) {
            format!(
                "结论：最近日志中出现异常关键词，建议展开本条工具结果查看具体行。{detail_suffix}"
            )
        } else if stdout.is_empty() && stderr.is_empty() {
            format!("结论：未获取到日志输出。{detail_suffix}")
        } else {
            format!("结论：已获取最近日志，未在摘要中发现明显异常关键词。{detail_suffix}")
        }
    } else {
        return None;
    };

    Some(if timed_out {
        format!("{conclusion}\n注意：命令执行超时，结果可能不完整。退出码 {exit_code}。")
    } else if exit_code != "0" {
        format!("{conclusion}\n注意：命令退出码为 {exit_code}，结果可能只是部分输出。")
    } else {
        format!("{conclusion}\n退出码：{exit_code}。")
    })
}

fn first_non_empty_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

#[derive(Clone, Copy, Debug, Default)]
struct DiskUsageFindings {
    actionable_full: bool,
    ignored_rom_full: bool,
}

fn disk_usage_findings(text: &str) -> DiskUsageFindings {
    let mut findings = DiskUsageFindings::default();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        let Some((percent_index, percent)) =
            columns.iter().enumerate().find_map(|(index, column)| {
                column
                    .strip_suffix('%')
                    .and_then(|value| value.parse::<u8>().ok())
                    .map(|value| (index, value))
            })
        else {
            continue;
        };
        let mountpoint = columns
            .get(percent_index + 1)
            .or_else(|| columns.last())
            .copied()
            .unwrap_or("");
        let filesystem = columns.first().copied().unwrap_or("");
        if percent < 90 {
            continue;
        }
        if is_ignored_readonly_rom_mount(filesystem, mountpoint) {
            findings.ignored_rom_full = true;
        } else if is_actionable_disk_mount(mountpoint) {
            findings.actionable_full = true;
        }
    }
    findings
}

fn is_ignored_readonly_rom_mount(filesystem: &str, mountpoint: &str) -> bool {
    mountpoint == "/rom" || (filesystem == "/dev/root" && mountpoint == "/rom")
}

fn is_actionable_disk_mount(mountpoint: &str) -> bool {
    mountpoint == "/"
        || mountpoint == "/overlay"
        || mountpoint == "/tmp"
        || mountpoint == "/var"
        || mountpoint == "/home"
        || mountpoint == "/opt"
        || mountpoint == "/data"
        || mountpoint.starts_with("/mnt/")
        || mountpoint.starts_with("/overlay/")
}

fn contains_log_warning(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "error", "failed", "panic", "oom", "denied", "timeout", "异常", "失败", "错误",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn redact_text(text: &str) -> String {
    let mut redacted = text.lines().map(redact_line).collect::<Vec<_>>().join("\n");
    if text.ends_with('\n') {
        redacted.push('\n');
    }
    redacted
}

fn redact_line(line: &str) -> String {
    for separator in ['=', ':'] {
        if let Some((key, _value)) = line.split_once(separator)
            && is_secret_key(key.trim())
        {
            return format!("{}{}[REDACTED]", key, separator);
        }
    }
    let lower = line.to_ascii_lowercase();
    if ["api_key", "apikey", "token", "secret", "bearer"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return "[REDACTED]".to_string();
    }
    line.to_string()
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "nonce", "secret", "bearer", "lease", "idempotency"]
        .iter()
        .any(|needle| key.contains(needle))
}
