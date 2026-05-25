use std::{
    collections::VecDeque,
    fmt,
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::audit::{
    RunnerBrowserAuditRecord, RunnerMaintenanceAuditNonce, RunnerMaintenanceAuditRecord,
};
use crate::capabilities::CapabilitySet;
use crate::remote_schema::{RemoteToolCall, RemoteToolName, RemoteToolOutput};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("remote tool failed: {0}")]
    Failed(String),
}

pub trait RemoteToolTransport {
    fn execute(&mut self, call: RemoteToolCall) -> Result<RemoteToolOutput, TransportError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualBootstrapTransportConfig {
    pub instructions: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshTransportConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerTransportConfig {
    pub endpoint: String,
    pub token_present: bool,
}

impl RunnerTransportConfig {
    #[must_use]
    pub fn from_pairing(endpoint: impl Into<String>, pairing_token: impl AsRef<str>) -> Self {
        Self {
            endpoint: endpoint.into().trim().trim_end_matches('/').to_string(),
            token_present: !pairing_token.as_ref().trim().is_empty(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnerTransportState {
    Disconnected,
    Connected { endpoint: String },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunnerToolCallRequest {
    pub call_id: String,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerMaintenanceApproval {
    pub nonce: Option<String>,
    pub metadata: Option<Value>,
}

impl RunnerMaintenanceApproval {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nonce: None,
            metadata: None,
        }
    }

    #[must_use]
    pub fn with_nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    fn is_empty(&self) -> bool {
        self.nonce.is_none() && self.metadata.is_none()
    }

    #[must_use]
    fn into_json(self) -> Value {
        let mut approval = serde_json::Map::new();

        if let Some(nonce) = self.nonce {
            approval.insert("nonce".to_string(), Value::String(nonce));
        }

        if let Some(metadata) = self.metadata {
            approval.insert("metadata".to_string(), metadata);
        }

        Value::Object(approval)
    }
}

impl Default for RunnerMaintenanceApproval {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerMaintenancePlanRequest {
    pub action: String,
    pub dry_run: bool,
    pub idempotency_key: Option<String>,
    pub approval: Option<RunnerMaintenanceApproval>,
}

impl RunnerMaintenancePlanRequest {
    #[must_use]
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            dry_run: true,
            idempotency_key: None,
            approval: None,
        }
    }

    #[must_use]
    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    #[must_use]
    pub fn with_approval(mut self, approval: RunnerMaintenanceApproval) -> Self {
        self.approval = Some(approval);
        self
    }

    #[must_use]
    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    #[must_use]
    fn into_body(self) -> Value {
        let mut body = serde_json::Map::new();
        body.insert("action".to_string(), Value::String(self.action));
        body.insert("dry_run".to_string(), Value::Bool(self.dry_run));
        if let Some(idempotency_key) = self.idempotency_key
            && !idempotency_key.trim().is_empty()
        {
            body.insert(
                "idempotency_key".to_string(),
                Value::String(idempotency_key),
            );
        }

        if let Some(approval) = self.approval
            && !approval.is_empty()
        {
            body.insert("approval".to_string(), approval.into_json());
        }

        Value::Object(body)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RunnerBearerToken(String);

impl RunnerBearerToken {
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    #[must_use]
    pub fn authorization_header_value(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

impl fmt::Debug for RunnerBearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnerHttpRequestAuth {
    Bearer { token: RunnerBearerToken },
}

impl RunnerHttpRequestAuth {
    #[must_use]
    pub const fn scheme(&self) -> &'static str {
        match self {
            Self::Bearer { .. } => "Bearer",
        }
    }

    #[must_use]
    pub fn authorization_header_value(&self) -> String {
        match self {
            Self::Bearer { token } => token.authorization_header_value(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerHttpRequestSpec {
    pub method: String,
    pub url: String,
    pub token_present: bool,
    pub auth: Option<RunnerHttpRequestAuth>,
    pub body: Value,
}

impl RunnerHttpRequestSpec {
    #[must_use]
    pub fn auth_scheme(&self) -> Option<&'static str> {
        self.auth.as_ref().map(RunnerHttpRequestAuth::scheme)
    }

    #[must_use]
    pub fn authorization_header_value(&self) -> Option<String> {
        self.auth
            .as_ref()
            .map(RunnerHttpRequestAuth::authorization_header_value)
    }
}

pub trait RunnerHttpExchange {
    fn exchange(&mut self, request: RunnerHttpRequestSpec) -> Result<Value, TransportError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerHttpResponseSpec {
    pub status: u16,
    pub body: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerMaintenanceResponse {
    pub body: Value,
    pub audit: Vec<RunnerMaintenanceAuditRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerBrowserToolResponse {
    pub output: RemoteToolOutput,
    pub audit: Vec<RunnerBrowserAuditRecord>,
}

impl RunnerHttpResponseSpec {
    #[must_use]
    pub fn json(status: u16, body: Value) -> Self {
        Self { status, body }
    }

    #[must_use]
    pub fn ok_json(body: Value) -> Self {
        Self { status: 200, body }
    }

    pub fn into_success_json(self) -> Result<Value, TransportError> {
        if (200..300).contains(&self.status) {
            return Ok(self.body);
        }

        Err(TransportError::Failed(format!(
            "runner http backend returned unsuccessful status {}",
            self.status
        )))
    }
}

pub trait RunnerHttpBackend {
    fn execute(
        &mut self,
        request: RunnerHttpRequestSpec,
    ) -> Result<RunnerHttpResponseSpec, TransportError>;
}

pub struct RunnerHttpClosureBackend<F> {
    execute: F,
}

impl<F> RunnerHttpClosureBackend<F> {
    #[must_use]
    pub const fn new(execute: F) -> Self {
        Self { execute }
    }
}

impl<F> fmt::Debug for RunnerHttpClosureBackend<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RunnerHttpClosureBackend")
    }
}

impl<F> RunnerHttpBackend for RunnerHttpClosureBackend<F>
where
    F: FnMut(RunnerHttpRequestSpec) -> Result<RunnerHttpResponseSpec, TransportError>,
{
    fn execute(
        &mut self,
        request: RunnerHttpRequestSpec,
    ) -> Result<RunnerHttpResponseSpec, TransportError> {
        (self.execute)(request)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerTcpHttpBackend {
    timeout: Duration,
}

impl RunnerTcpHttpBackend {
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }

    #[must_use]
    pub const fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for RunnerTcpHttpBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerHttpBackend for RunnerTcpHttpBackend {
    fn execute(
        &mut self,
        request: RunnerHttpRequestSpec,
    ) -> Result<RunnerHttpResponseSpec, TransportError> {
        match request.method.as_str() {
            "GET" | "POST" => {}
            method => {
                return Err(TransportError::Failed(format!(
                    "runner http backend does not support {method} requests"
                )));
            }
        }

        execute_runner_tcp_http_request(request, self.timeout)
    }
}

fn execute_runner_tcp_http_request(
    request: RunnerHttpRequestSpec,
    timeout: Duration,
) -> Result<RunnerHttpResponseSpec, TransportError> {
    let target = RunnerHttpTarget::parse(&request.url)?;
    let mut stream = TcpStream::connect((target.host.as_str(), target.port)).map_err(|error| {
        TransportError::Failed(format!("runner http backend connection failed: {error}"))
    })?;
    stream.set_read_timeout(Some(timeout)).map_err(|error| {
        TransportError::Failed(format!("runner http backend setup failed: {error}"))
    })?;
    stream.set_write_timeout(Some(timeout)).map_err(|error| {
        TransportError::Failed(format!("runner http backend setup failed: {error}"))
    })?;

    let body = if request.method == "POST" {
        serde_json::to_string(&request.body).map_err(|error| {
            TransportError::Failed(format!(
                "runner http backend request body was invalid: {error}"
            ))
        })?
    } else {
        String::new()
    };

    let mut raw_request = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n",
        request.method, target.path, target.host_header
    );
    if let Some(authorization) = request.authorization_header_value() {
        raw_request.push_str("Authorization: ");
        raw_request.push_str(&authorization);
        raw_request.push_str("\r\n");
    }
    if request.method == "POST" {
        raw_request.push_str("Content-Type: application/json\r\n");
        raw_request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    raw_request.push_str("\r\n");
    raw_request.push_str(&body);

    stream.write_all(raw_request.as_bytes()).map_err(|error| {
        TransportError::Failed(format!("runner http backend request write failed: {error}"))
    })?;
    stream.flush().map_err(|error| {
        TransportError::Failed(format!("runner http backend request flush failed: {error}"))
    })?;

    let mut raw_response = String::new();
    stream.read_to_string(&mut raw_response).map_err(|error| {
        TransportError::Failed(format!(
            "runner http backend response body read failed: {error}"
        ))
    })?;

    parse_runner_tcp_http_response(&raw_response)
}

#[derive(Debug, PartialEq, Eq)]
struct RunnerHttpTarget {
    host: String,
    host_header: String,
    port: u16,
    path: String,
}

impl RunnerHttpTarget {
    fn parse(url: &str) -> Result<Self, TransportError> {
        let without_scheme = url.strip_prefix("http://").ok_or_else(|| {
            TransportError::Failed(
                "runner http backend only supports http:// local endpoints".to_string(),
            )
        })?;
        let (authority, path) = without_scheme
            .split_once('/')
            .map_or((without_scheme, "/"), |(authority, path)| {
                (authority, path.strip_prefix('/').unwrap_or(path))
            });
        if authority.is_empty() {
            return Err(TransportError::Failed(
                "runner http backend URL is missing host".to_string(),
            ));
        }

        let (host, port) = authority
            .rsplit_once(':')
            .map_or((authority, 80), |(host, port)| {
                (host, port.parse::<u16>().unwrap_or(0))
            });
        if host.is_empty() || port == 0 {
            return Err(TransportError::Failed(
                "runner http backend URL is invalid".to_string(),
            ));
        }

        Ok(Self {
            host: host.to_string(),
            host_header: authority.to_string(),
            port,
            path: format!("/{path}"),
        })
    }
}

fn parse_runner_tcp_http_response(
    raw_response: &str,
) -> Result<RunnerHttpResponseSpec, TransportError> {
    let (head, body) = raw_response.split_once("\r\n\r\n").ok_or_else(|| {
        TransportError::Failed("runner http backend response was invalid".to_string())
    })?;
    let status = head
        .lines()
        .next()
        .and_then(|status_line| status_line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| {
            TransportError::Failed("runner http backend response status was invalid".to_string())
        })?;
    let body = if body.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(body).map_err(|error| {
            TransportError::Failed(format!(
                "runner http backend response body was invalid JSON: {error}"
            ))
        })?
    };

    Ok(RunnerHttpResponseSpec { status, body })
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerHttpClient<B> {
    transport: RunnerHttpTransport,
    backend: B,
}

impl<B> RunnerHttpClient<B>
where
    B: RunnerHttpBackend,
{
    #[must_use]
    pub fn new(transport: RunnerHttpTransport, backend: B) -> Self {
        Self { transport, backend }
    }

    #[must_use]
    pub const fn transport(&self) -> &RunnerHttpTransport {
        &self.transport
    }

    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    #[must_use]
    pub const fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn discover_capabilities(&mut self) -> Result<CapabilitySet, TransportError> {
        let request = self.transport.prepare_capabilities_request()?;
        let body = self.backend.execute(request)?.into_success_json()?;
        RunnerHttpTransport::parse_capabilities_response(body)
    }

    pub fn execute_tool_call(
        &mut self,
        call: RemoteToolCall,
    ) -> Result<RemoteToolOutput, TransportError> {
        let request = self.transport.prepare_tool_call_request(&call)?;
        let body = self.backend.execute(request)?.into_success_json()?;
        RunnerHttpTransport::parse_tool_call_response(body)
    }

    pub fn execute_tool_call_with_browser_audit(
        &mut self,
        call: RemoteToolCall,
    ) -> Result<RunnerBrowserToolResponse, TransportError> {
        let request = self.transport.prepare_tool_call_request(&call)?;
        let body = self.backend.execute(request)?.into_success_json()?;
        RunnerHttpTransport::parse_tool_call_response_with_browser_audit(body)
    }

    pub fn plan_maintenance(
        &mut self,
        action: &str,
        dry_run: bool,
    ) -> Result<Value, TransportError> {
        let request = self
            .transport
            .prepare_maintenance_plan_request(action, dry_run)?;
        self.backend.execute(request)?.into_success_json()
    }

    pub fn plan_maintenance_request(
        &mut self,
        plan: RunnerMaintenancePlanRequest,
    ) -> Result<Value, TransportError> {
        let request = self
            .transport
            .prepare_maintenance_plan_request_with_options(plan)?;
        self.backend.execute(request)?.into_success_json()
    }

    pub fn fetch_recent_audit(&mut self) -> Result<Value, TransportError> {
        let request = self.transport.prepare_recent_audit_request()?;
        self.backend.execute(request)?.into_success_json()
    }
}

impl<B> RemoteToolTransport for RunnerHttpClient<B>
where
    B: RunnerHttpBackend,
{
    fn execute(&mut self, call: RemoteToolCall) -> Result<RemoteToolOutput, TransportError> {
        self.execute_tool_call(call)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    ManualBootstrap,
    Ssh,
    RunnerWebSocket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportCapability {
    BootstrapInstructions,
    RemoteShell,
    RemoteFileSystem,
    Mcp,
}

const MANUAL_BOOTSTRAP_CAPABILITIES: &[TransportCapability] =
    &[TransportCapability::BootstrapInstructions];
const SSH_CAPABILITIES: &[TransportCapability] = &[
    TransportCapability::RemoteShell,
    TransportCapability::RemoteFileSystem,
];
const RUNNER_WEBSOCKET_CAPABILITIES: &[TransportCapability] = &[
    TransportCapability::RemoteShell,
    TransportCapability::RemoteFileSystem,
    TransportCapability::Mcp,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportProfile {
    ManualBootstrap(ManualBootstrapTransportConfig),
    Ssh(SshTransportConfig),
    RunnerWebSocket(RunnerTransportConfig),
}

impl TransportProfile {
    #[must_use]
    pub fn manual_bootstrap(config: ManualBootstrapTransportConfig) -> Self {
        Self::ManualBootstrap(config)
    }

    #[must_use]
    pub fn ssh(config: SshTransportConfig) -> Self {
        Self::Ssh(config)
    }

    #[must_use]
    pub fn runner(config: RunnerTransportConfig) -> Self {
        Self::RunnerWebSocket(config)
    }

    #[must_use]
    pub const fn kind(&self) -> TransportKind {
        match self {
            Self::ManualBootstrap(_) => TransportKind::ManualBootstrap,
            Self::Ssh(_) => TransportKind::Ssh,
            Self::RunnerWebSocket(_) => TransportKind::RunnerWebSocket,
        }
    }

    #[must_use]
    pub const fn execution_mode(&self) -> &'static str {
        match self {
            Self::ManualBootstrap(_) => "manual-bootstrap",
            Self::Ssh(_) => "ssh",
            Self::RunnerWebSocket(_) => "runner-websocket",
        }
    }

    #[must_use]
    pub const fn capabilities(&self) -> &'static [TransportCapability] {
        match self {
            Self::ManualBootstrap(_) => MANUAL_BOOTSTRAP_CAPABILITIES,
            Self::Ssh(_) => SSH_CAPABILITIES,
            Self::RunnerWebSocket(_) => RUNNER_WEBSOCKET_CAPABILITIES,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerWebSocketTransport {
    config: RunnerTransportConfig,
    state: RunnerTransportState,
    responses: VecDeque<RemoteToolOutput>,
}

impl RunnerWebSocketTransport {
    #[must_use]
    pub fn new(config: RunnerTransportConfig) -> Self {
        Self {
            config,
            state: RunnerTransportState::Disconnected,
            responses: VecDeque::new(),
        }
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    #[must_use]
    pub const fn token_present(&self) -> bool {
        self.config.token_present
    }

    #[must_use]
    pub const fn config(&self) -> &RunnerTransportConfig {
        &self.config
    }

    #[must_use]
    pub const fn state(&self) -> &RunnerTransportState {
        &self.state
    }

    #[must_use]
    pub fn prepare_tool_call_request(&self, call: &RemoteToolCall) -> RunnerToolCallRequest {
        RunnerToolCallRequest {
            call_id: call.call_id.clone(),
            tool: call.name.to_string(),
            arguments: call.arguments.clone(),
        }
    }

    pub fn connect_fake(&mut self) {
        self.state = RunnerTransportState::Connected {
            endpoint: self.config.endpoint.clone(),
        };
    }

    pub fn push_response(&mut self, output: RemoteToolOutput) {
        self.responses.push_back(output);
    }
}

impl RemoteToolTransport for RunnerWebSocketTransport {
    fn execute(&mut self, call: RemoteToolCall) -> Result<RemoteToolOutput, TransportError> {
        match &self.state {
            RunnerTransportState::Disconnected => Err(TransportError::Failed(
                "runner websocket transport is not connected".to_string(),
            )),
            RunnerTransportState::Connected { .. } => self.responses.pop_front().ok_or_else(|| {
                TransportError::Failed(format!(
                    "runner websocket transport has no fake response for {}",
                    call.call_id
                ))
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerHttpTransport {
    config: RunnerTransportConfig,
    bearer_token: Option<RunnerBearerToken>,
    state: RunnerTransportState,
    requests: Vec<RunnerHttpRequestSpec>,
    response_bodies: VecDeque<Value>,
}

impl RunnerHttpTransport {
    #[must_use]
    pub fn new(config: RunnerTransportConfig) -> Self {
        Self {
            config,
            bearer_token: None,
            state: RunnerTransportState::Disconnected,
            requests: Vec::new(),
            response_bodies: VecDeque::new(),
        }
    }

    #[must_use]
    pub fn with_bearer_token(endpoint: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            config: RunnerTransportConfig {
                endpoint: endpoint.into(),
                token_present: true,
            },
            bearer_token: Some(RunnerBearerToken::new(token)),
            state: RunnerTransportState::Disconnected,
            requests: Vec::new(),
            response_bodies: VecDeque::new(),
        }
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    #[must_use]
    pub const fn token_present(&self) -> bool {
        self.config.token_present
    }

    #[must_use]
    pub const fn config(&self) -> &RunnerTransportConfig {
        &self.config
    }

    #[must_use]
    pub const fn state(&self) -> &RunnerTransportState {
        &self.state
    }

    fn request_auth(&self) -> Option<RunnerHttpRequestAuth> {
        self.bearer_token
            .clone()
            .map(|token| RunnerHttpRequestAuth::Bearer { token })
    }

    pub fn prepare_tool_call_request(
        &self,
        call: &RemoteToolCall,
    ) -> Result<RunnerHttpRequestSpec, TransportError> {
        let endpoint = self.config.endpoint.trim().trim_end_matches('/');
        if endpoint.is_empty() {
            return Err(TransportError::Failed(
                "runner http transport is not configured".to_string(),
            ));
        }

        Ok(RunnerHttpRequestSpec {
            method: "POST".to_string(),
            url: format!("{endpoint}/tool-call"),
            token_present: self.config.token_present,
            auth: self.request_auth(),
            body: serde_json::json!({
                "call_id": call.call_id,
                "name": call.name,
                "arguments": call.arguments,
            }),
        })
    }

    pub fn prepare_capabilities_request(&self) -> Result<RunnerHttpRequestSpec, TransportError> {
        let endpoint = self.config.endpoint.trim().trim_end_matches('/');
        if endpoint.is_empty() {
            return Err(TransportError::Failed(
                "runner http transport is not configured".to_string(),
            ));
        }

        Ok(RunnerHttpRequestSpec {
            method: "GET".to_string(),
            url: format!("{endpoint}/capabilities"),
            token_present: self.config.token_present,
            auth: self.request_auth(),
            body: Value::Null,
        })
    }

    pub fn prepare_recent_audit_request(&self) -> Result<RunnerHttpRequestSpec, TransportError> {
        let endpoint = self.config.endpoint.trim().trim_end_matches('/');
        if endpoint.is_empty() {
            return Err(TransportError::Failed(
                "runner audit transport is not configured".to_string(),
            ));
        }

        Ok(RunnerHttpRequestSpec {
            method: "GET".to_string(),
            url: format!("{endpoint}/audit/recent"),
            token_present: self.config.token_present,
            auth: self.request_auth(),
            body: Value::Null,
        })
    }

    pub fn prepare_maintenance_plan_request(
        &self,
        action: &str,
        dry_run: bool,
    ) -> Result<RunnerHttpRequestSpec, TransportError> {
        self.prepare_maintenance_plan_request_with_options(
            RunnerMaintenancePlanRequest::new(action).with_dry_run(dry_run),
        )
    }

    pub fn prepare_maintenance_plan_request_with_options(
        &self,
        plan: RunnerMaintenancePlanRequest,
    ) -> Result<RunnerHttpRequestSpec, TransportError> {
        let endpoint = self.config.endpoint.trim().trim_end_matches('/');
        if endpoint.is_empty() {
            return Err(TransportError::Failed(
                "runner maintenance transport is not configured".to_string(),
            ));
        }

        Ok(RunnerHttpRequestSpec {
            method: "POST".to_string(),
            url: format!("{endpoint}/maintenance/plan"),
            token_present: self.config.token_present,
            auth: self.request_auth(),
            body: plan.into_body(),
        })
    }

    pub fn prepare_mcp_call_request(
        &self,
        call: &RemoteToolCall,
    ) -> Result<RunnerHttpRequestSpec, TransportError> {
        if call.name != RemoteToolName::McpCall {
            return Err(TransportError::Failed(format!(
                "runner MCP call request expected remote.mcp.call but received {}",
                call.name
            )));
        }

        let endpoint = self.config.endpoint.trim().trim_end_matches('/');
        if endpoint.is_empty() {
            return Err(TransportError::Failed(
                "runner MCP call transport is not configured".to_string(),
            ));
        }

        Ok(RunnerHttpRequestSpec {
            method: "POST".to_string(),
            url: format!("{endpoint}/mcp/call"),
            token_present: self.config.token_present,
            auth: self.request_auth(),
            body: serde_json::json!({
                "call_id": call.call_id,
                "arguments": call.arguments,
            }),
        })
    }

    pub fn parse_capabilities_response(body: Value) -> Result<CapabilitySet, TransportError> {
        let tools = body
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                TransportError::Failed(
                    "runner http capabilities response was invalid: missing tools array"
                        .to_string(),
                )
            })?
            .iter()
            .filter_map(Value::as_str);

        Ok(CapabilitySet::from_runner_reported_tools(tools))
    }

    pub fn parse_tool_call_response(body: Value) -> Result<RemoteToolOutput, TransportError> {
        serde_json::from_value(body).map_err(|err| {
            TransportError::Failed(format!("runner http transport response was invalid: {err}"))
        })
    }

    pub fn parse_tool_call_response_with_browser_audit(
        body: Value,
    ) -> Result<RunnerBrowserToolResponse, TransportError> {
        let mut output = Self::parse_tool_call_response(body)?;
        let audit = parse_runner_browser_audit_records(&mut output.result)?;

        Ok(RunnerBrowserToolResponse { output, audit })
    }

    pub fn parse_maintenance_response(
        mut body: Value,
    ) -> Result<RunnerMaintenanceResponse, TransportError> {
        let audit = match body.get("audit") {
            Some(Value::Array(entries)) => entries
                .iter()
                .map(parse_runner_maintenance_audit_record)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(TransportError::Failed(
                    "runner maintenance response was invalid: audit must be an array".to_string(),
                ));
            }
            None => Vec::new(),
        };

        if let Some(object) = body.as_object_mut() {
            object.remove("audit");
        }

        Ok(RunnerMaintenanceResponse { body, audit })
    }

    pub fn discover_capabilities<E>(
        &self,
        exchange: &mut E,
    ) -> Result<CapabilitySet, TransportError>
    where
        E: RunnerHttpExchange,
    {
        let request = self.prepare_capabilities_request()?;
        let body = exchange.exchange(request)?;
        Self::parse_capabilities_response(body)
    }

    pub fn execute_tool_call_exchange<E>(
        &self,
        call: RemoteToolCall,
        exchange: &mut E,
    ) -> Result<RemoteToolOutput, TransportError>
    where
        E: RunnerHttpExchange,
    {
        let request = self.prepare_tool_call_request(&call)?;
        let body = exchange.exchange(request)?;
        Self::parse_tool_call_response(body)
    }

    pub fn connect_fake(&mut self) {
        self.state = RunnerTransportState::Connected {
            endpoint: self.config.endpoint.clone(),
        };
    }

    pub fn push_response_body(&mut self, body: Value) {
        self.response_bodies.push_back(body);
    }

    #[must_use]
    pub fn requests(&self) -> &[RunnerHttpRequestSpec] {
        &self.requests
    }
}

fn parse_runner_maintenance_audit_record(
    entry: &Value,
) -> Result<RunnerMaintenanceAuditRecord, TransportError> {
    let event = required_string_field(entry, "event")?;
    let status = required_string_field(entry, "status")?;
    let nonce = entry
        .get("nonce")
        .map(parse_runner_maintenance_audit_nonce)
        .transpose()?;
    let reason = entry
        .get("reason")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(RunnerMaintenanceAuditRecord {
        event,
        status,
        nonce,
        reason,
    })
}

fn parse_runner_maintenance_audit_nonce(
    nonce: &Value,
) -> Result<RunnerMaintenanceAuditNonce, TransportError> {
    Ok(RunnerMaintenanceAuditNonce {
        redacted: nonce.get("redacted").and_then(Value::as_bool),
        label: required_string_field(nonce, "label")?,
        status: required_string_field(nonce, "status")?,
    })
}

fn parse_runner_browser_audit_records(
    result: &mut Value,
) -> Result<Vec<RunnerBrowserAuditRecord>, TransportError> {
    let Some(data) = result.get_mut("data") else {
        return Ok(Vec::new());
    };

    sanitize_browser_value(data);

    let action = data
        .get("action")
        .and_then(Value::as_str)
        .map(str::to_string);
    let risk = data.get("risk").and_then(Value::as_str).map(str::to_string);
    let approval_required = data.get("approval_required").and_then(Value::as_bool);
    let metadata = data.get("metadata").cloned().unwrap_or(Value::Null);

    let audit_value = data.get("audit").cloned();
    if let Some(object) = data.as_object_mut() {
        object.remove("audit");
    }

    let Some(audit_value) = audit_value else {
        return Ok(Vec::new());
    };
    let Value::Array(entries) = audit_value else {
        return Err(TransportError::Failed(
            "runner browser response was invalid: audit must be an array".to_string(),
        ));
    };

    entries
        .iter()
        .map(|entry| {
            parse_runner_browser_audit_record(
                entry,
                action.clone(),
                risk.clone(),
                approval_required,
                metadata.clone(),
            )
        })
        .collect()
}

fn parse_runner_browser_audit_record(
    entry: &Value,
    fallback_action: Option<String>,
    risk: Option<String>,
    approval_required: Option<bool>,
    metadata: Value,
) -> Result<RunnerBrowserAuditRecord, TransportError> {
    Ok(RunnerBrowserAuditRecord {
        record_type: required_browser_string_field(entry, "type")?,
        tool: required_browser_string_field(entry, "tool")?,
        action: entry
            .get("action")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(fallback_action),
        session_id: entry
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        selector: entry
            .get("selector")
            .and_then(Value::as_str)
            .map(str::to_string),
        reason: entry
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        risk,
        approval_required,
        metadata,
    })
}

fn sanitize_browser_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| !is_sensitive_browser_key(key));
            for value in object.values_mut() {
                sanitize_browser_value(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                sanitize_browser_value(value);
            }
        }
        _ => {}
    }
}

fn is_sensitive_browser_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower == "authorization"
        || lower == "nonce"
        || lower == "raw_nonce"
        || lower == "raw"
        || lower == "bearer_token"
        || lower == "token"
}

fn required_browser_string_field(value: &Value, field: &str) -> Result<String, TransportError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            TransportError::Failed(format!(
                "runner browser response was invalid: missing {field}"
            ))
        })
}

fn required_string_field(value: &Value, field: &str) -> Result<String, TransportError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            TransportError::Failed(format!(
                "runner maintenance response was invalid: missing {field}"
            ))
        })
}

impl RemoteToolTransport for RunnerHttpTransport {
    fn execute(&mut self, call: RemoteToolCall) -> Result<RemoteToolOutput, TransportError> {
        match &self.state {
            RunnerTransportState::Disconnected => Err(TransportError::Failed(
                "runner http transport is not connected".to_string(),
            )),
            RunnerTransportState::Connected { .. } => {
                let request = self.prepare_tool_call_request(&call)?;
                self.requests.push(request);

                let body = self.response_bodies.pop_front().ok_or_else(|| {
                    TransportError::Failed(format!(
                        "runner http transport has no fake response for {}",
                        call.call_id
                    ))
                })?;

                Self::parse_tool_call_response(body)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshTransport {
    config: SshTransportConfig,
}

impl SshTransport {
    #[must_use]
    pub fn new(config: SshTransportConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub const fn config(&self) -> &SshTransportConfig {
        &self.config
    }
}

impl RemoteToolTransport for SshTransport {
    fn execute(&mut self, _call: RemoteToolCall) -> Result<RemoteToolOutput, TransportError> {
        Err(TransportError::Failed(
            "ssh transport is not connected".to_string(),
        ))
    }
}

#[derive(Debug, Default)]
pub struct FakeTransport {
    calls: Vec<RemoteToolCall>,
    responses: VecDeque<Result<RemoteToolOutput, TransportError>>,
}

impl FakeTransport {
    pub fn push_output(&mut self, output: RemoteToolOutput) {
        self.responses.push_back(Ok(output));
    }

    pub fn push_error(&mut self, message: impl Into<String>) {
        self.responses
            .push_back(Err(TransportError::Failed(message.into())));
    }

    pub fn calls(&self) -> &[RemoteToolCall] {
        &self.calls
    }
}

impl RemoteToolTransport for FakeTransport {
    fn execute(&mut self, call: RemoteToolCall) -> Result<RemoteToolOutput, TransportError> {
        self.calls.push(call.clone());

        if let Some(response) = self.responses.pop_front() {
            return response;
        }

        Ok(RemoteToolOutput {
            call_id: call.call_id,
            success: true,
            result: call.arguments,
        })
    }
}
