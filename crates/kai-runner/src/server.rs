use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::pairing::{PairingError, PairingManager};
use crate::{
    BrowserEnabledKaiRunner, KaiRunner, McpEnabledKaiRunner, RunnerMaintenanceAction,
    RunnerMaintenanceOptions, RunnerRequest, RunnerResponse, RunnerToolError,
    ShellEnabledKaiRunner, ShellExecutor,
};

const AUDIT_RING_CAPACITY: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct RunnerApiResponse {
    pub status: u16,
    pub body: Value,
}

#[derive(Debug, Default)]
pub struct RunnerApiHandler {
    runner: RunnerBackend,
    auth_token: Option<String>,
    pairing_manager: Option<Mutex<PairingManager<'static>>>,
    pairing_tokens: Mutex<HashSet<String>>,
    approval_nonce_counter: Mutex<u64>,
    audit: Mutex<VecDeque<Value>>,
}

impl RunnerApiHandler {
    #[must_use]
    pub fn new(runner: KaiRunner) -> Self {
        Self {
            runner: RunnerBackend::Default(runner),
            auth_token: None,
            pairing_manager: None,
            pairing_tokens: Mutex::new(HashSet::new()),
            approval_nonce_counter: Mutex::new(0),
            audit: Mutex::new(VecDeque::with_capacity(AUDIT_RING_CAPACITY)),
        }
    }

    #[must_use]
    pub fn new_with_token(runner: KaiRunner, token: impl Into<String>) -> Self {
        Self {
            runner: RunnerBackend::Default(runner),
            auth_token: Some(token.into()),
            pairing_manager: None,
            pairing_tokens: Mutex::new(HashSet::new()),
            approval_nonce_counter: Mutex::new(0),
            audit: Mutex::new(VecDeque::with_capacity(AUDIT_RING_CAPACITY)),
        }
    }

    #[must_use]
    pub fn new_with_pairing_manager(
        runner: KaiRunner,
        pairing_manager: PairingManager<'static>,
    ) -> Self {
        Self {
            runner: RunnerBackend::Default(runner),
            auth_token: None,
            pairing_manager: Some(Mutex::new(pairing_manager)),
            pairing_tokens: Mutex::new(HashSet::new()),
            approval_nonce_counter: Mutex::new(0),
            audit: Mutex::new(VecDeque::with_capacity(AUDIT_RING_CAPACITY)),
        }
    }

    #[must_use]
    pub fn new_with_mcp_runner(runner: McpEnabledKaiRunner) -> Self {
        Self {
            runner: RunnerBackend::Mcp(runner),
            auth_token: None,
            pairing_manager: None,
            pairing_tokens: Mutex::new(HashSet::new()),
            approval_nonce_counter: Mutex::new(0),
            audit: Mutex::new(VecDeque::with_capacity(AUDIT_RING_CAPACITY)),
        }
    }

    #[must_use]
    pub fn new_with_mcp_runner_and_token(
        runner: McpEnabledKaiRunner,
        token: impl Into<String>,
    ) -> Self {
        Self {
            runner: RunnerBackend::Mcp(runner),
            auth_token: Some(token.into()),
            pairing_manager: None,
            pairing_tokens: Mutex::new(HashSet::new()),
            approval_nonce_counter: Mutex::new(0),
            audit: Mutex::new(VecDeque::with_capacity(AUDIT_RING_CAPACITY)),
        }
    }

    #[must_use]
    pub fn new_with_browser_runner(runner: BrowserEnabledKaiRunner) -> Self {
        Self {
            runner: RunnerBackend::Browser(runner),
            auth_token: None,
            pairing_manager: None,
            pairing_tokens: Mutex::new(HashSet::new()),
            approval_nonce_counter: Mutex::new(0),
            audit: Mutex::new(VecDeque::with_capacity(AUDIT_RING_CAPACITY)),
        }
    }

    #[must_use]
    pub fn new_with_shell_runner_and_token<E>(
        runner: ShellEnabledKaiRunner<E>,
        token: impl Into<String>,
    ) -> Self
    where
        E: ShellExecutor,
    {
        Self {
            runner: RunnerBackend::Shell(Box::new(runner)),
            auth_token: Some(token.into()),
            pairing_manager: None,
            pairing_tokens: Mutex::new(HashSet::new()),
            approval_nonce_counter: Mutex::new(0),
            audit: Mutex::new(VecDeque::with_capacity(AUDIT_RING_CAPACITY)),
        }
    }

    #[must_use]
    pub fn handle(&self, method: &str, path: &str, body: Option<Value>) -> RunnerApiResponse {
        self.handle_with_bearer_token(method, path, body, None)
    }

    #[must_use]
    pub fn handle_with_bearer_token(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        bearer_token: Option<&str>,
    ) -> RunnerApiResponse {
        if self.requires_auth(method, path) && !self.is_authorized(bearer_token) {
            return unauthorized();
        }

        match (method, path) {
            ("GET", "/health") => RunnerApiResponse {
                status: 200,
                body: json!({
                    "status": "ok",
                    "mode": "runner",
                }),
            },
            ("POST", "/pairing/code") => self.pairing_code_response(),
            ("POST", "/pairing/redeem") => self.pairing_redeem_response(body),
            ("POST", "/approval/nonce") => self.approval_nonce_response(),
            ("GET", "/capabilities") => self.runner_response(RunnerRequest::Capabilities),
            ("GET", "/audit/recent") => self.audit_recent_response(),
            ("POST", "/tool-call") => self.tool_call_response(body),
            ("POST", "/mcp/call") => self.mcp_call_response(body),
            ("POST", "/maintenance/plan") => self.maintenance_plan_response(body),
            _ => RunnerApiResponse {
                status: 404,
                body: json!({
                    "error": {
                        "code": "not_found",
                        "message": format!("{method} {path} is not supported by kai-runner"),
                    }
                }),
            },
        }
    }

    fn requires_auth(&self, method: &str, path: &str) -> bool {
        (self.auth_token.is_some() || self.pairing_manager.is_some())
            && matches!(
                (method, path),
                ("GET", "/capabilities")
                    | ("GET", "/audit/recent")
                    | ("POST", "/tool-call")
                    | ("POST", "/mcp/call")
                    | ("POST", "/maintenance/plan")
                    | ("POST", "/approval/nonce")
            )
    }

    fn is_authorized(&self, bearer_token: Option<&str>) -> bool {
        match bearer_token {
            Some(actual) => {
                self.auth_token.as_deref() == Some(actual)
                    || self
                        .pairing_tokens
                        .lock()
                        .expect("pairing token store must not be poisoned")
                        .contains(actual)
            }
            None => self.auth_token.is_none() && self.pairing_manager.is_none(),
        }
    }

    fn pairing_code_response(&self) -> RunnerApiResponse {
        let Some(pairing_manager) = &self.pairing_manager else {
            return RunnerApiResponse {
                status: 404,
                body: json!({
                    "error": {
                        "code": "not_found",
                        "message": "pairing is not enabled for kai-runner",
                    }
                }),
            };
        };

        let now = Instant::now();
        let code = pairing_manager
            .lock()
            .expect("pairing manager must not be poisoned")
            .create_at(now);
        let ttl_seconds = code.expires_at().duration_since(now).as_secs();

        RunnerApiResponse {
            status: 200,
            body: json!({
                "code": code.value(),
                "ttl_seconds": ttl_seconds,
            }),
        }
    }

    fn pairing_redeem_response(&self, body: Option<Value>) -> RunnerApiResponse {
        let Some(pairing_manager) = &self.pairing_manager else {
            return RunnerApiResponse {
                status: 404,
                body: json!({
                    "error": {
                        "code": "not_found",
                        "message": "pairing is not enabled for kai-runner",
                    }
                }),
            };
        };
        let Some(body) = body else {
            return bad_request("missing JSON body");
        };
        let Ok(request) = serde_json::from_value::<PairingRedeemRequest>(body) else {
            return bad_request("invalid pairing redeem JSON body");
        };

        match pairing_manager
            .lock()
            .expect("pairing manager must not be poisoned")
            .redeem(&request.code)
        {
            Ok(token) => {
                self.pairing_tokens
                    .lock()
                    .expect("pairing token store must not be poisoned")
                    .insert(token.value().to_string());

                RunnerApiResponse {
                    status: 200,
                    body: json!({
                        "token": token.value(),
                        "label": token.descriptor().label(),
                    }),
                }
            }
            Err(error) => pairing_error_response(error),
        }
    }

    fn tool_call_response(&self, body: Option<Value>) -> RunnerApiResponse {
        let Some(body) = body else {
            return bad_request("missing JSON body");
        };

        let Ok(request) = serde_json::from_value::<ToolCallRequest>(body) else {
            return bad_request("invalid tool-call JSON body");
        };

        self.runner_response(RunnerRequest::ToolCall {
            call_id: request.call_id,
            name: request.name,
            arguments: request.arguments.unwrap_or_else(|| json!({})),
        })
    }

    fn mcp_call_response(&self, body: Option<Value>) -> RunnerApiResponse {
        let Some(body) = body else {
            return bad_request("missing JSON body");
        };

        let Ok(request) = serde_json::from_value::<McpCallRequest>(body) else {
            return bad_request("invalid mcp-call JSON body");
        };

        self.runner_response(RunnerRequest::ToolCall {
            call_id: request.call_id,
            name: "remote.mcp.call".to_string(),
            arguments: request.arguments.unwrap_or_else(|| json!({})),
        })
    }

    fn maintenance_plan_response(&self, body: Option<Value>) -> RunnerApiResponse {
        let Some(body) = body else {
            return bad_request("missing JSON body");
        };

        let Ok(request) = serde_json::from_value::<MaintenancePlanRequest>(body) else {
            return bad_request("invalid maintenance plan JSON body");
        };

        let execute = request.execute.unwrap_or(false);
        let mut audit = Vec::new();
        let approval_audit = match self.maintenance_approval_audit(execute, request.approval) {
            Ok(approval_audit) => {
                if approval_audit.is_some() {
                    audit.push(approval_nonce_audit_event("consumed", "consumed"));
                }
                approval_audit
            }
            Err((error, error_audit)) => {
                self.record_audit_events(&error_audit);
                return runner_tool_error_response_with_audit(error, error_audit);
            }
        };

        if execute && approval_audit.is_some() {
            audit.push(maintenance_execute_audit_event("not_executable"));
            self.record_audit_events(&audit);
            return runner_tool_error_response_with_approval(
                RunnerToolError {
                    code: "not_executable",
                    message: "runner maintenance approval nonce was consumed, but maintenance execution is disabled in this scaffold".to_string(),
                },
                approval_audit,
                audit,
            );
        }

        match KaiRunner::plan_maintenance(
            request.action,
            RunnerMaintenanceOptions {
                execute,
                dry_run: request.dry_run,
                idempotency_key: request.idempotency_key,
                secret: None,
            },
        ) {
            Ok(plan) => {
                let mut body =
                    serde_json::to_value(plan).expect("maintenance plan must be serializable");
                if let Some(approval_audit) = approval_audit {
                    body["approval"] = approval_audit;
                }
                if !execute {
                    audit.push(maintenance_execute_audit_event("dry_run"));
                }
                if !audit.is_empty() {
                    body["audit"] = Value::Array(audit);
                }
                if let Some(audit) = body["audit"].as_array() {
                    self.record_audit_events(audit);
                }
                RunnerApiResponse { status: 200, body }
            }
            Err(error) => {
                self.record_audit_events(&audit);
                runner_tool_error_response_with_approval(error, approval_audit, audit)
            }
        }
    }

    fn maintenance_approval_audit(
        &self,
        _execute: bool,
        approval: Option<MaintenanceApprovalRequest>,
    ) -> Result<Option<Value>, (RunnerToolError, Vec<Value>)> {
        let Some(approval) = approval else {
            return Ok(None);
        };

        let Some(nonce) = approval.nonce.filter(|nonce| !nonce.is_empty()) else {
            return Err((
                RunnerToolError {
                    code: "approval_required",
                    message: "maintenance approval requires nonce".to_string(),
                },
                vec![approval_nonce_audit_event("missing", "missing")],
            ));
        };

        if let Err(error) = self.runner.redeem_approval_nonce(&nonce) {
            let (status, nonce_status) = match error.code {
                "approval_replayed" => ("replay", "replayed"),
                "approval_expired" => ("expired", "expired"),
                _ => ("wrong", "rejected"),
            };
            return Err((
                error,
                vec![approval_nonce_audit_event(status, nonce_status)],
            ));
        }

        let mut audit = json!({
            "status": "consumed",
        });
        if let Some(metadata) = approval.metadata {
            audit["metadata"] = metadata;
        }
        Ok(Some(audit))
    }

    fn approval_nonce_response(&self) -> RunnerApiResponse {
        let mut counter = self
            .approval_nonce_counter
            .lock()
            .expect("approval nonce counter must not be poisoned");
        *counter += 1;
        let nonce = format!("approval-nonce-{counter}");
        drop(counter);

        if !self.runner.issue_approval_nonce(nonce.clone()) {
            return RunnerApiResponse {
                status: 404,
                body: json!({
                    "error": {
                        "code": "not_found",
                        "message": "approval nonce issuance is not enabled for kai-runner",
                    }
                }),
            };
        }

        self.record_audit_event(approval_nonce_audit_event("issued", "issued"));

        RunnerApiResponse {
            status: 200,
            body: json!({
                "approval_nonce": nonce,
            }),
        }
    }

    fn audit_recent_response(&self) -> RunnerApiResponse {
        let audit: Vec<Value> = self
            .audit
            .lock()
            .expect("audit ring buffer must not be poisoned")
            .iter()
            .cloned()
            .collect();

        RunnerApiResponse {
            status: 200,
            body: json!({
                "type": "audit_recent",
                "mode": "runner",
                "persistence": "memory_scaffold",
                "audit": audit
            }),
        }
    }

    fn runner_response(&self, request: RunnerRequest) -> RunnerApiResponse {
        let response = self.runner.handle_request(request.clone());
        self.record_runner_audit(&request, &response);

        RunnerApiResponse {
            status: 200,
            body: serde_json::to_value(response)
                .expect("runner response must be JSON serializable"),
        }
    }

    fn record_runner_audit(&self, request: &RunnerRequest, response: &RunnerResponse) {
        match (request, response) {
            (RunnerRequest::Capabilities, RunnerResponse::Capabilities { tools, .. }) => {
                self.record_audit_event(json!({
                    "event": "capabilities.snapshot",
                    "status": "available",
                    "source": "runner",
                    "metadata": {
                        "tools": tools,
                    },
                }));
            }
            (
                RunnerRequest::ToolCall {
                    call_id,
                    name,
                    arguments,
                },
                RunnerResponse::ToolCall {
                    success, result, ..
                },
            ) => {
                let error_code = result
                    .pointer("/error/code")
                    .and_then(Value::as_str)
                    .map(str::to_string);

                let mut metadata = json!({
                    "call_id": call_id,
                    "tool": name,
                });
                if let Some(error_code) = error_code {
                    metadata["error_code"] = json!(error_code);
                }

                if arguments.get("lease").is_some() {
                    for event in command_lease_audit_events(
                        call_id,
                        name,
                        *success,
                        metadata.get("error_code").and_then(Value::as_str),
                    ) {
                        self.record_audit_event(event);
                    }
                }

                let (event, status) = if name.starts_with("remote.browser.") {
                    ("browser.action", if *success { "ok" } else { "blocked" })
                } else {
                    ("tool_call", if *success { "ok" } else { "error" })
                };

                self.record_audit_event(json!({
                    "event": event,
                    "status": status,
                    "source": "runner",
                    "metadata": metadata,
                }));
            }
            _ => {}
        }
    }

    fn record_audit_events(&self, events: &[Value]) {
        for event in events {
            self.record_audit_event(event.clone());
        }
    }

    fn record_audit_event(&self, event: Value) {
        let mut audit = self
            .audit
            .lock()
            .expect("audit ring buffer must not be poisoned");
        if audit.len() == AUDIT_RING_CAPACITY {
            audit.pop_front();
        }
        audit.push_back(event);
    }
}

#[derive(Debug)]
enum RunnerBackend {
    Default(KaiRunner),
    Browser(BrowserEnabledKaiRunner),
    Mcp(McpEnabledKaiRunner),
    Shell(Box<dyn RunnerBackendHandler>),
}

impl Default for RunnerBackend {
    fn default() -> Self {
        Self::Default(KaiRunner)
    }
}

impl RunnerBackend {
    fn handle_request(&self, request: RunnerRequest) -> RunnerResponse {
        match self {
            Self::Default(runner) => runner.handle_request(request),
            Self::Browser(runner) => runner.handle_request(request),
            Self::Mcp(runner) => runner.handle_request(request),
            Self::Shell(runner) => runner.handle_request(request),
        }
    }

    fn issue_approval_nonce(&self, nonce: String) -> bool {
        match self {
            Self::Shell(runner) => runner.issue_approval_nonce(nonce),
            Self::Default(_) | Self::Browser(_) | Self::Mcp(_) => false,
        }
    }

    fn redeem_approval_nonce(&self, nonce: &str) -> Result<(), RunnerToolError> {
        match self {
            Self::Shell(runner) => runner.redeem_approval_nonce(nonce),
            Self::Default(_) | Self::Browser(_) | Self::Mcp(_) => Err(RunnerToolError {
                code: "approval_denied",
                message: "maintenance approval nonce was not accepted".to_string(),
            }),
        }
    }
}

trait RunnerBackendHandler: std::fmt::Debug + Send + Sync {
    fn handle_request(&self, request: RunnerRequest) -> RunnerResponse;
    fn issue_approval_nonce(&self, nonce: String) -> bool;
    fn redeem_approval_nonce(&self, nonce: &str) -> Result<(), RunnerToolError>;
}

impl<E> RunnerBackendHandler for ShellEnabledKaiRunner<E>
where
    E: ShellExecutor,
{
    fn handle_request(&self, request: RunnerRequest) -> RunnerResponse {
        self.handle_request(request)
    }

    fn issue_approval_nonce(&self, nonce: String) -> bool {
        match &self.approval_policy.mode {
            crate::ShellApprovalPolicyMode::ManagedNonces(manager) => {
                manager.create(nonce, None);
                true
            }
            crate::ShellApprovalPolicyMode::StaticNonce { .. } => false,
        }
    }

    fn redeem_approval_nonce(&self, nonce: &str) -> Result<(), RunnerToolError> {
        match &self.approval_policy.mode {
            crate::ShellApprovalPolicyMode::ManagedNonces(manager) => manager.redeem(nonce),
            crate::ShellApprovalPolicyMode::StaticNonce { .. } => Err(RunnerToolError {
                code: "approval_denied",
                message: "maintenance approval nonce was not accepted".to_string(),
            }),
        }
    }
}

pub fn build_router(runner: KaiRunner) -> Router {
    let handler = Arc::new(RunnerApiHandler::new(runner));

    router_with_handler(handler)
}

pub fn build_router_with_token(runner: KaiRunner, token: impl Into<String>) -> Router {
    let handler = Arc::new(RunnerApiHandler::new_with_token(runner, token));

    router_with_handler(handler)
}

pub fn build_router_with_pairing_manager(
    runner: KaiRunner,
    pairing_manager: PairingManager<'static>,
) -> Router {
    let handler = Arc::new(RunnerApiHandler::new_with_pairing_manager(
        runner,
        pairing_manager,
    ));

    router_with_handler(handler)
}

pub fn build_router_with_mcp_runner(runner: McpEnabledKaiRunner) -> Router {
    let handler = Arc::new(RunnerApiHandler::new_with_mcp_runner(runner));

    router_with_handler(handler)
}

pub fn build_router_with_browser_runner(runner: BrowserEnabledKaiRunner) -> Router {
    let handler = Arc::new(RunnerApiHandler::new_with_browser_runner(runner));

    router_with_handler(handler)
}

pub fn build_router_with_mcp_runner_and_token(
    runner: McpEnabledKaiRunner,
    token: impl Into<String>,
) -> Router {
    let handler = Arc::new(RunnerApiHandler::new_with_mcp_runner_and_token(
        runner, token,
    ));

    router_with_handler(handler)
}

pub fn build_router_with_shell_runner_and_token<E>(
    runner: ShellEnabledKaiRunner<E>,
    token: impl Into<String>,
) -> Router
where
    E: ShellExecutor,
{
    let handler = Arc::new(RunnerApiHandler::new_with_shell_runner_and_token(
        runner, token,
    ));

    router_with_handler(handler)
}

fn router_with_handler(handler: Arc<RunnerApiHandler>) -> Router {
    Router::new()
        .route("/health", get(http_health))
        .route("/pairing/code", post(http_pairing_code))
        .route("/pairing/redeem", post(http_pairing_redeem))
        .route("/approval/nonce", post(http_approval_nonce))
        .route("/capabilities", get(http_capabilities))
        .route("/audit/recent", get(http_audit_recent))
        .route("/tool-call", post(http_tool_call))
        .route("/mcp/call", post(http_mcp_call))
        .route("/maintenance/plan", post(http_maintenance_plan))
        .with_state(handler)
}

async fn http_health(State(handler): State<Arc<RunnerApiHandler>>) -> Response {
    api_response(handler.handle("GET", "/health", None))
}

async fn http_pairing_code(State(handler): State<Arc<RunnerApiHandler>>) -> Response {
    api_response(handler.handle("POST", "/pairing/code", None))
}

async fn http_pairing_redeem(
    State(handler): State<Arc<RunnerApiHandler>>,
    Json(body): Json<Value>,
) -> Response {
    api_response(handler.handle("POST", "/pairing/redeem", Some(body)))
}

async fn http_approval_nonce(
    State(handler): State<Arc<RunnerApiHandler>>,
    headers: HeaderMap,
) -> Response {
    api_response(handler.handle_with_bearer_token(
        "POST",
        "/approval/nonce",
        None,
        bearer_token(&headers),
    ))
}

async fn http_capabilities(
    State(handler): State<Arc<RunnerApiHandler>>,
    headers: HeaderMap,
) -> Response {
    api_response(handler.handle_with_bearer_token(
        "GET",
        "/capabilities",
        None,
        bearer_token(&headers),
    ))
}

async fn http_audit_recent(
    State(handler): State<Arc<RunnerApiHandler>>,
    headers: HeaderMap,
) -> Response {
    api_response(handler.handle_with_bearer_token(
        "GET",
        "/audit/recent",
        None,
        bearer_token(&headers),
    ))
}

async fn http_tool_call(
    State(handler): State<Arc<RunnerApiHandler>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    api_response(handler.handle_with_bearer_token(
        "POST",
        "/tool-call",
        Some(body),
        bearer_token(&headers),
    ))
}

async fn http_mcp_call(
    State(handler): State<Arc<RunnerApiHandler>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    api_response(handler.handle_with_bearer_token(
        "POST",
        "/mcp/call",
        Some(body),
        bearer_token(&headers),
    ))
}

async fn http_maintenance_plan(
    State(handler): State<Arc<RunnerApiHandler>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    api_response(handler.handle_with_bearer_token(
        "POST",
        "/maintenance/plan",
        Some(body),
        bearer_token(&headers),
    ))
}

fn api_response(response: RunnerApiResponse) -> Response {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(response.body)).into_response()
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

#[derive(Debug, Deserialize)]
struct ToolCallRequest {
    call_id: String,
    name: String,
    arguments: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct McpCallRequest {
    call_id: String,
    arguments: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct MaintenancePlanRequest {
    action: RunnerMaintenanceAction,
    dry_run: bool,
    idempotency_key: Option<String>,
    execute: Option<bool>,
    approval: Option<MaintenanceApprovalRequest>,
}

#[derive(Debug, Deserialize)]
struct MaintenanceApprovalRequest {
    nonce: Option<String>,
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct PairingRedeemRequest {
    code: String,
}

fn bad_request(message: &'static str) -> RunnerApiResponse {
    RunnerApiResponse {
        status: 400,
        body: json!({
            "error": {
                "code": "bad_request",
                "message": message,
            }
        }),
    }
}

fn runner_tool_error_response_with_audit(
    error: RunnerToolError,
    audit: Vec<Value>,
) -> RunnerApiResponse {
    runner_tool_error_response_with_approval(error, None, audit)
}

fn runner_tool_error_response_with_approval(
    error: RunnerToolError,
    approval: Option<Value>,
    audit: Vec<Value>,
) -> RunnerApiResponse {
    let status = match error.code {
        "approval_required" => 403,
        "not_executable" => 400,
        _ => 400,
    };

    let mut body = json!({
        "error": {
            "code": error.code,
            "message": error.message,
        }
    });
    if let Some(approval) = approval {
        body["approval"] = approval;
    }
    if !audit.is_empty() {
        body["audit"] = Value::Array(audit);
    }

    RunnerApiResponse { status, body }
}

fn approval_nonce_audit_event(status: &'static str, nonce_status: &'static str) -> Value {
    json!({
        "event": "maintenance.approval_nonce",
        "status": status,
        "nonce": {
            "label": "approval_nonce",
            "status": nonce_status,
        },
    })
}

fn maintenance_execute_audit_event(reason: &'static str) -> Value {
    json!({
        "event": "maintenance.execute",
        "status": "no_op",
        "reason": reason,
    })
}

fn command_lease_audit_events(
    call_id: &str,
    tool: &str,
    success: bool,
    error_code: Option<&str>,
) -> Vec<Value> {
    if success {
        return vec![
            command_lease_audit_event(call_id, tool, "accepted", "accepted", None),
            command_lease_audit_event(call_id, tool, "consumed", "consumed", None),
        ];
    }

    match error_code {
        Some("approval_replayed") => vec![command_lease_audit_event(
            call_id,
            tool,
            "replay_rejected",
            "replayed",
            error_code,
        )],
        Some("approval_expired") => vec![command_lease_audit_event(
            call_id,
            tool,
            "expired_rejected",
            "expired",
            error_code,
        )],
        Some("approval_required") => vec![command_lease_audit_event(
            call_id,
            tool,
            "invalid_action_rejected",
            "invalid_action",
            error_code,
        )],
        _ => Vec::new(),
    }
}

fn command_lease_audit_event(
    call_id: &str,
    tool: &str,
    status: &'static str,
    lease_status: &'static str,
    error_code: Option<&str>,
) -> Value {
    let mut metadata = json!({
        "call_id": call_id,
        "tool": tool,
    });
    if let Some(error_code) = error_code {
        metadata["error_code"] = json!(error_code);
    }

    json!({
        "event": "shell.command_lease",
        "status": status,
        "lease": {
            "label": "command_lease",
            "status": lease_status,
        },
        "metadata": metadata,
    })
}

fn pairing_error_response(error: PairingError) -> RunnerApiResponse {
    let (status, code, message) = match error {
        PairingError::Unknown => (404, "unknown", "pairing code was not found"),
        PairingError::Expired => (400, "expired", "pairing code has expired"),
        PairingError::AlreadyUsed => (409, "already_used", "pairing code was already used"),
    };

    RunnerApiResponse {
        status,
        body: json!({
            "error": {
                "code": code,
                "message": message,
            }
        }),
    }
}

fn unauthorized() -> RunnerApiResponse {
    RunnerApiResponse {
        status: 401,
        body: json!({
            "error": {
                "code": "unauthorized",
                "message": "missing or invalid bearer token",
            }
        }),
    }
}
