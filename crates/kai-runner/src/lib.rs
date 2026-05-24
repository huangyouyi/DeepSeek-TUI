use deepseek_mobile_agent_core::{CapabilitySet, RemoteToolCall, RemoteToolName, RemoteToolOutput};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub mod pairing;
pub mod server;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerRequest {
    Capabilities,
    ToolCall {
        call_id: String,
        name: String,
        #[serde(default)]
        arguments: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerResponse {
    Capabilities {
        mode: String,
        tools: Vec<String>,
    },
    ToolCall {
        call_id: String,
        success: bool,
        result: Value,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunnerCapabilities {
    pub tools: CapabilitySet,
}

impl Serialize for RunnerCapabilities {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        RunnerCapabilitiesReport {
            mode: "runner",
            tools: self.tools.tools.iter().map(|tool| tool.as_str()).collect(),
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct RunnerCapabilitiesReport {
    mode: &'static str,
    tools: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunnerToolResult {
    pub tool: String,
    pub status: RunnerToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RunnerToolError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerToolStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunnerToolError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerMaintenanceAction {
    SelfUpdate,
    Uninstall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerMaintenanceRisk {
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerMaintenancePlan {
    pub action: RunnerMaintenanceAction,
    pub commands: Vec<String>,
    pub preflight_steps: Vec<String>,
    pub maintenance_steps: Vec<String>,
    pub artifact_verification: Vec<RunnerArtifactVerificationPlan>,
    pub rollback_steps: Vec<String>,
    pub safety_guidance: Vec<String>,
    pub requires_approval: bool,
    pub dry_run: bool,
    pub risk: RunnerMaintenanceRisk,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerMaintenanceOptions {
    pub execute: bool,
    pub dry_run: bool,
    pub idempotency_key: Option<String>,
    pub secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerArtifactVerificationPlan {
    pub platform: String,
    pub artifact_name: String,
    pub expected_checksum_source: String,
    pub expected_signature_source: String,
    pub verify_step: String,
    pub verify_command: String,
    pub failure_rollback_guidance: String,
}

impl Default for RunnerMaintenanceOptions {
    fn default() -> Self {
        Self {
            execute: false,
            dry_run: true,
            idempotency_key: None,
            secret: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct KaiRunner;

#[derive(Debug, Clone, Default)]
pub struct BrowserSessionRegistry {
    state: Arc<Mutex<BrowserSessionRegistryState>>,
}

#[derive(Debug, Default)]
struct BrowserSessionRegistryState {
    next_id: u64,
    sessions: Vec<BrowserSession>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSession {
    pub id: String,
    pub url: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSessionMetadata {
    pub url: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserOpenRequest {
    pub url: String,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserOpenEvent {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserOpenResult {
    pub status: String,
    pub url: String,
    pub profile: Option<String>,
    pub events: Vec<BrowserOpenEvent>,
    pub next_approval_required: bool,
}

impl BrowserOpenResult {
    #[must_use]
    pub fn planned(url: String, profile: Option<String>, message: impl Into<String>) -> Self {
        Self {
            status: "planned".to_string(),
            url,
            profile,
            events: vec![BrowserOpenEvent {
                kind: "planned".to_string(),
                message: message.into(),
            }],
            next_approval_required: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserExtractTextRequest {
    pub session_id: String,
    pub url: Option<String>,
    pub selector: Option<String>,
    pub handle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserExtractTextResult {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserClickRequest {
    pub session_id: Option<String>,
    pub selector: Option<String>,
    pub label: Option<String>,
    pub action: String,
    pub metadata: Option<Value>,
    pub idempotency_key: Option<String>,
    pub approval_nonce: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserClickApproval {
    pub session_id: Option<String>,
    pub selector: Option<String>,
    pub label: Option<String>,
    pub action: String,
    pub metadata: Option<Value>,
    pub idempotency_key: Option<String>,
    pub approval_nonce: Option<String>,
    pub risk: String,
}

impl BrowserClickApproval {
    #[must_use]
    pub fn approval_required(request: BrowserClickRequest) -> Self {
        Self {
            session_id: request.session_id,
            selector: request.selector,
            label: request.label,
            action: request.action,
            metadata: request.metadata,
            idempotency_key: request.idempotency_key,
            approval_nonce: request.approval_nonce,
            risk: "state_changing".to_string(),
        }
    }
}

pub trait BrowserEngine: std::fmt::Debug + Send + Sync + 'static {
    fn open(&self, request: BrowserOpenRequest) -> Result<BrowserOpenResult, RunnerToolError>;

    fn extract_text(
        &self,
        request: BrowserExtractTextRequest,
    ) -> Result<BrowserExtractTextResult, RunnerToolError>;

    fn click(&self, request: BrowserClickRequest) -> Result<BrowserClickApproval, RunnerToolError>;
}

#[derive(Debug, Clone, Default)]
pub struct ScaffoldBrowserEngine;

impl BrowserEngine for ScaffoldBrowserEngine {
    fn open(&self, request: BrowserOpenRequest) -> Result<BrowserOpenResult, RunnerToolError> {
        Ok(BrowserOpenResult::planned(
            request.url,
            request.profile,
            "browser.open accepted but no browser was launched",
        ))
    }

    fn extract_text(
        &self,
        _request: BrowserExtractTextRequest,
    ) -> Result<BrowserExtractTextResult, RunnerToolError> {
        Err(RunnerToolError {
            code: "browser_unavailable",
            message: "kai-runner scaffold has no browser engine yet".to_string(),
        })
    }

    fn click(&self, request: BrowserClickRequest) -> Result<BrowserClickApproval, RunnerToolError> {
        Ok(BrowserClickApproval::approval_required(request))
    }
}

#[derive(Debug, Clone)]
pub struct BrowserEnabledKaiRunner {
    registry: BrowserSessionRegistry,
    engine: Arc<dyn BrowserEngine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpToolRequest {
    pub server: String,
    pub tool: String,
    pub arguments: Value,
}

pub trait McpToolHandler: std::fmt::Debug + Send + Sync + 'static {
    fn call(&self, request: McpToolRequest) -> Result<Value, RunnerToolError>;
}

#[derive(Debug, Clone, Default)]
pub struct McpEnabledKaiRunner {
    registry: HashMap<(String, String), Arc<dyn McpToolHandler>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Shell,
    PowerShell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellExecutionRequest {
    pub kind: ShellKind,
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_ms: Option<u64>,
    pub env: BTreeMap<String, String>,
    pub path: Vec<String>,
    pub clear_env: bool,
    pub lease: Option<CommandLeaseEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandLeaseEnvelope {
    pub id: String,
    pub idempotency_key: String,
    pub approved_action: CommandLeaseAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandLeaseAction {
    pub tool: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl CommandLeaseEnvelope {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        idempotency_key: impl Into<String>,
        approved_action: CommandLeaseAction,
    ) -> Self {
        Self {
            id: id.into(),
            idempotency_key: idempotency_key.into(),
            approved_action,
            expires_at_unix_ms: None,
        }
    }

    #[must_use]
    pub fn with_expires_at_unix_ms(mut self, expires_at_unix_ms: u64) -> Self {
        self.expires_at_unix_ms = Some(expires_at_unix_ms);
        self
    }
}

impl CommandLeaseAction {
    #[must_use]
    pub fn new(
        tool: impl Into<String>,
        command: impl Into<String>,
        cwd: Option<impl Into<String>>,
    ) -> Self {
        Self {
            tool: tool.into(),
            command: command.into(),
            cwd: cwd.map(Into::into),
        }
    }
}

impl CommandLeaseEnvelope {
    fn validate_for_action(
        &self,
        actual_action: &CommandLeaseAction,
    ) -> Result<(), RunnerToolError> {
        if self.id.trim().is_empty() || self.idempotency_key.trim().is_empty() {
            return Err(RunnerToolError {
                code: "approval_required",
                message: "shell execution requires a command lease id and idempotency key"
                    .to_string(),
            });
        }

        if self.approved_action != *actual_action {
            return Err(RunnerToolError {
                code: "approval_required",
                message: "shell execution command lease does not match the approved action"
                    .to_string(),
            });
        }

        if self
            .expires_at_unix_ms
            .is_some_and(|expires_at| current_unix_ms() >= expires_at)
        {
            return Err(RunnerToolError {
                code: "approval_expired",
                message: "shell execution command lease has expired".to_string(),
            });
        }

        Ok(())
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

pub trait ShellExecutor: std::fmt::Debug + Send + Sync + 'static {
    fn execute(&self, request: ShellExecutionRequest) -> ShellExecutionResult;

    fn supports_powershell(&self) -> bool {
        true
    }

    fn requires_explicit_execution_policy(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemShellExecutor {
    real_shell_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct ShellEnabledKaiRunner<E> {
    executor: E,
    approval_policy: ShellApprovalPolicy,
    execution_policy: ShellExecutionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellExecutionPolicy {
    configured: bool,
    cwd_roots: Vec<String>,
    allowed_env_keys: BTreeSet<String>,
    path_entries: Vec<String>,
    clear_env: bool,
}

#[derive(Debug, Clone)]
pub struct ShellApprovalPolicy {
    mode: ShellApprovalPolicyMode,
}

#[derive(Debug, Clone)]
enum ShellApprovalPolicyMode {
    StaticNonce { expected_nonce: Option<String> },
    ManagedNonces(ShellApprovalNonceManager),
}

#[derive(Debug, Clone, Default)]
pub struct ShellApprovalNonceManager {
    state: Arc<Mutex<HashMap<String, ShellApprovalNonceRecord>>>,
}

#[derive(Debug, Clone)]
struct ShellApprovalNonceRecord {
    status: ShellApprovalNonceStatus,
    expires_at: Option<Instant>,
    lease: Option<CommandLeaseEnvelope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellApprovalNonceStatus {
    Pending,
    Redeemed,
    Expired,
}

#[derive(Debug, Clone)]
pub struct WorkspaceKaiRunner {
    workspace: PathBuf,
}

impl KaiRunner {
    #[must_use]
    pub fn with_shell_executor<E>(executor: E) -> ShellEnabledKaiRunner<E>
    where
        E: ShellExecutor,
    {
        ShellEnabledKaiRunner {
            executor,
            approval_policy: ShellApprovalPolicy::default(),
            execution_policy: ShellExecutionPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_system_shell_executor() -> ShellEnabledKaiRunner<SystemShellExecutor> {
        Self::with_shell_executor(SystemShellExecutor::default())
    }

    #[must_use]
    pub fn with_mcp_handler<H>(
        server: impl Into<String>,
        tool: impl Into<String>,
        handler: H,
    ) -> McpEnabledKaiRunner
    where
        H: McpToolHandler,
    {
        McpEnabledKaiRunner::default().with_mcp_handler(server, tool, handler)
    }

    #[must_use]
    pub fn with_workspace(path: impl AsRef<Path>) -> WorkspaceKaiRunner {
        let path = path.as_ref();
        WorkspaceKaiRunner {
            workspace: path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
        }
    }

    #[must_use]
    pub fn with_browser_registry(registry: BrowserSessionRegistry) -> BrowserEnabledKaiRunner {
        BrowserEnabledKaiRunner {
            registry,
            engine: Arc::new(ScaffoldBrowserEngine),
        }
    }

    #[must_use]
    pub fn with_browser_engine(
        registry: BrowserSessionRegistry,
        engine: Arc<dyn BrowserEngine>,
    ) -> BrowserEnabledKaiRunner {
        BrowserEnabledKaiRunner { registry, engine }
    }

    #[must_use]
    pub fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            tools: CapabilitySet::runner(),
        }
    }

    pub fn plan_maintenance(
        action: RunnerMaintenanceAction,
        options: RunnerMaintenanceOptions,
    ) -> Result<RunnerMaintenancePlan, RunnerToolError> {
        runner_maintenance_plan(action, options)
    }

    pub fn execute(&self, call: RemoteToolCall) -> RemoteToolOutput {
        let tool = call.name.as_str();
        if call.name == RemoteToolName::DiagnoseSystem {
            return RemoteToolOutput {
                call_id: call.call_id,
                success: true,
                result: diagnose_system_value(tool, call.arguments, self.capabilities()),
            };
        }
        if call.name == RemoteToolName::PowerShellExec {
            return powershell_exec_scaffold(call);
        }
        if matches!(
            call.name,
            RemoteToolName::BrowserOpen
                | RemoteToolName::BrowserExtractText
                | RemoteToolName::BrowserClick
        ) {
            return browser_tool(call);
        }
        if call.name == RemoteToolName::PackageInstall {
            return package_install(call);
        }

        let (code, message) = if matches!(
            call.name,
            RemoteToolName::ShellExec | RemoteToolName::PowerShellExec
        ) {
            (
                "blocked",
                "kai-runner scaffold does not execute local shell commands".to_string(),
            )
        } else {
            (
                "unsupported",
                format!("{tool} is not implemented in the kai-runner scaffold"),
            )
        };

        RemoteToolOutput {
            call_id: call.call_id,
            success: false,
            result: runner_result_value(RunnerToolResult {
                tool: tool.to_string(),
                status: RunnerToolStatus::Error,
                data: None,
                error: Some(RunnerToolError { code, message }),
            }),
        }
    }

    pub fn handle_request(&self, request: RunnerRequest) -> RunnerResponse {
        match request {
            RunnerRequest::Capabilities => {
                let capabilities = self.capabilities();
                RunnerResponse::Capabilities {
                    mode: "runner".to_string(),
                    tools: capabilities
                        .tools
                        .tools
                        .iter()
                        .map(|tool| tool.as_str().to_string())
                        .collect(),
                }
            }
            RunnerRequest::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let Ok(name) = RemoteToolName::parse(&name) else {
                    return RunnerResponse::ToolCall {
                        call_id,
                        success: false,
                        result: runner_result_value(RunnerToolResult {
                            tool: name.clone(),
                            status: RunnerToolStatus::Error,
                            data: None,
                            error: Some(RunnerToolError {
                                code: "unknown_tool",
                                message: format!("unknown remote tool name: {name}"),
                            }),
                        }),
                    };
                };

                let output = self.execute(RemoteToolCall {
                    call_id,
                    name,
                    arguments,
                });

                RunnerResponse::ToolCall {
                    call_id: output.call_id,
                    success: output.success,
                    result: output.result,
                }
            }
        }
    }
}

impl BrowserSessionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn create_session(&self, metadata: BrowserSessionMetadata) -> BrowserSession {
        let mut state = self
            .state
            .lock()
            .expect("browser session registry mutex was poisoned");
        state.next_id += 1;
        let session = BrowserSession {
            id: format!("browser-session-{}", state.next_id),
            url: metadata.url,
            profile: metadata.profile,
        };
        state.sessions.push(session.clone());
        session
    }

    #[must_use]
    pub fn list_sessions(&self) -> Vec<BrowserSession> {
        self.state
            .lock()
            .expect("browser session registry mutex was poisoned")
            .sessions
            .clone()
    }

    pub fn close_session(&self, session_id: &str) -> Option<BrowserSession> {
        let mut state = self
            .state
            .lock()
            .expect("browser session registry mutex was poisoned");
        let index = state
            .sessions
            .iter()
            .position(|session| session.id == session_id)?;
        Some(state.sessions.remove(index))
    }

    #[must_use]
    pub fn get_session(&self, session_id: &str) -> Option<BrowserSession> {
        self.state
            .lock()
            .expect("browser session registry mutex was poisoned")
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .cloned()
    }
}

impl BrowserEnabledKaiRunner {
    #[must_use]
    pub fn registry(&self) -> BrowserSessionRegistry {
        self.registry.clone()
    }

    #[must_use]
    pub fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            tools: CapabilitySet::runner(),
        }
    }

    pub fn execute(&self, call: RemoteToolCall) -> RemoteToolOutput {
        if matches!(
            call.name,
            RemoteToolName::BrowserOpen
                | RemoteToolName::BrowserExtractText
                | RemoteToolName::BrowserClick
        ) {
            return browser_tool_with_registry(call, Some(&self.registry), self.engine.as_ref());
        }

        KaiRunner.execute(call)
    }

    pub fn handle_request(&self, request: RunnerRequest) -> RunnerResponse {
        match request {
            RunnerRequest::Capabilities => {
                let capabilities = self.capabilities();
                RunnerResponse::Capabilities {
                    mode: "runner".to_string(),
                    tools: capabilities
                        .tools
                        .tools
                        .iter()
                        .map(|tool| tool.as_str().to_string())
                        .collect(),
                }
            }
            RunnerRequest::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let Ok(name) = RemoteToolName::parse(&name) else {
                    return RunnerResponse::ToolCall {
                        call_id,
                        success: false,
                        result: runner_result_value(RunnerToolResult {
                            tool: name.clone(),
                            status: RunnerToolStatus::Error,
                            data: None,
                            error: Some(RunnerToolError {
                                code: "unknown_tool",
                                message: format!("unknown remote tool name: {name}"),
                            }),
                        }),
                    };
                };

                let output = self.execute(RemoteToolCall {
                    call_id,
                    name,
                    arguments,
                });

                RunnerResponse::ToolCall {
                    call_id: output.call_id,
                    success: output.success,
                    result: output.result,
                }
            }
        }
    }
}

impl McpEnabledKaiRunner {
    #[must_use]
    pub fn with_mcp_handler<H>(
        mut self,
        server: impl Into<String>,
        tool: impl Into<String>,
        handler: H,
    ) -> Self
    where
        H: McpToolHandler,
    {
        self.registry
            .insert((server.into(), tool.into()), Arc::new(handler));
        self
    }

    #[must_use]
    pub fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            tools: CapabilitySet::runner(),
        }
    }

    pub fn execute(&self, call: RemoteToolCall) -> RemoteToolOutput {
        if call.name == RemoteToolName::McpCall {
            return self.execute_mcp(call);
        }

        KaiRunner.execute(call)
    }

    pub fn handle_request(&self, request: RunnerRequest) -> RunnerResponse {
        match request {
            RunnerRequest::Capabilities => {
                let capabilities = self.capabilities();
                RunnerResponse::Capabilities {
                    mode: "runner".to_string(),
                    tools: capabilities
                        .tools
                        .tools
                        .iter()
                        .map(|tool| tool.as_str().to_string())
                        .collect(),
                }
            }
            RunnerRequest::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let Ok(name) = RemoteToolName::parse(&name) else {
                    return RunnerResponse::ToolCall {
                        call_id,
                        success: false,
                        result: runner_result_value(RunnerToolResult {
                            tool: name.clone(),
                            status: RunnerToolStatus::Error,
                            data: None,
                            error: Some(RunnerToolError {
                                code: "unknown_tool",
                                message: format!("unknown remote tool name: {name}"),
                            }),
                        }),
                    };
                };

                let output = self.execute(RemoteToolCall {
                    call_id,
                    name,
                    arguments,
                });

                RunnerResponse::ToolCall {
                    call_id: output.call_id,
                    success: output.success,
                    result: output.result,
                }
            }
        }
    }

    fn execute_mcp(&self, call: RemoteToolCall) -> RemoteToolOutput {
        let tool = call.name.as_str().to_string();
        let result = match mcp_tool_request(call.arguments) {
            Ok(request) => {
                let key = (request.server.clone(), request.tool.clone());
                match self.registry.get(&key) {
                    Some(handler) => match handler.call(request) {
                        Ok(data) => RunnerToolResult {
                            tool,
                            status: RunnerToolStatus::Ok,
                            data: Some(data),
                            error: None,
                        },
                        Err(error) => RunnerToolResult {
                            tool,
                            status: RunnerToolStatus::Error,
                            data: None,
                            error: Some(error),
                        },
                    },
                    None => RunnerToolResult {
                        tool,
                        status: RunnerToolStatus::Error,
                        data: None,
                        error: Some(RunnerToolError {
                            code: "unknown_mcp_tool",
                            message: format!("no registered MCP handler for {}/{}", key.0, key.1),
                        }),
                    },
                }
            }
            Err(error) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Error,
                data: None,
                error: Some(error),
            },
        };

        RemoteToolOutput {
            call_id: call.call_id,
            success: result.status == RunnerToolStatus::Ok,
            result: runner_result_value(result),
        }
    }
}

impl<E> ShellEnabledKaiRunner<E>
where
    E: ShellExecutor,
{
    #[must_use]
    pub fn with_approval_nonce(mut self, nonce: impl Into<String>) -> Self {
        self.approval_policy = ShellApprovalPolicy::static_nonce(nonce);
        self
    }

    #[must_use]
    pub fn with_approval_policy(mut self, policy: ShellApprovalPolicy) -> Self {
        self.approval_policy = policy;
        self
    }

    #[must_use]
    pub fn with_approval_nonce_manager(mut self, manager: ShellApprovalNonceManager) -> Self {
        self.approval_policy = ShellApprovalPolicy::nonce_manager(manager);
        self
    }

    #[must_use]
    pub fn with_shell_execution_policy(mut self, policy: ShellExecutionPolicy) -> Self {
        self.execution_policy = policy;
        self
    }

    #[must_use]
    pub fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            tools: CapabilitySet::runner(),
        }
    }

    pub fn execute(&self, call: RemoteToolCall) -> RemoteToolOutput {
        if matches!(
            call.name,
            RemoteToolName::ShellExec | RemoteToolName::PowerShellExec
        ) {
            return self.execute_shell(call);
        }

        KaiRunner.execute(call)
    }

    pub fn handle_request(&self, request: RunnerRequest) -> RunnerResponse {
        match request {
            RunnerRequest::Capabilities => {
                let capabilities = self.capabilities();
                RunnerResponse::Capabilities {
                    mode: "runner".to_string(),
                    tools: capabilities
                        .tools
                        .tools
                        .iter()
                        .map(|tool| tool.as_str().to_string())
                        .collect(),
                }
            }
            RunnerRequest::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let Ok(name) = RemoteToolName::parse(&name) else {
                    return RunnerResponse::ToolCall {
                        call_id,
                        success: false,
                        result: runner_result_value(RunnerToolResult {
                            tool: name.clone(),
                            status: RunnerToolStatus::Error,
                            data: None,
                            error: Some(RunnerToolError {
                                code: "unknown_tool",
                                message: format!("unknown remote tool name: {name}"),
                            }),
                        }),
                    };
                };

                let output = self.execute(RemoteToolCall {
                    call_id,
                    name,
                    arguments,
                });

                RunnerResponse::ToolCall {
                    call_id: output.call_id,
                    success: output.success,
                    result: output.result,
                }
            }
        }
    }

    fn execute_shell(&self, call: RemoteToolCall) -> RemoteToolOutput {
        let tool = call.name.as_str().to_string();
        let kind = if call.name == RemoteToolName::PowerShellExec {
            ShellKind::PowerShell
        } else {
            ShellKind::Shell
        };

        if self.executor.requires_explicit_execution_policy() && !self.execution_policy.configured {
            return RemoteToolOutput {
                call_id: call.call_id,
                success: false,
                result: runner_result_value(RunnerToolResult {
                    tool,
                    status: RunnerToolStatus::Error,
                    data: None,
                    error: Some(RunnerToolError {
                        code: "policy_required",
                        message: "real shell execution requires an explicit ShellExecutionPolicy"
                            .to_string(),
                    }),
                }),
            };
        }

        let arguments = call.arguments;
        let request = match shell_execution_request(kind, arguments.clone(), &self.execution_policy)
        {
            Ok(request) => request,
            Err(error) => {
                return RemoteToolOutput {
                    call_id: call.call_id,
                    success: false,
                    result: runner_result_value(RunnerToolResult {
                        tool,
                        status: RunnerToolStatus::Error,
                        data: None,
                        error: Some(error),
                    }),
                };
            }
        };

        if let Err(error) = self.approval_policy.check(&tool, &request, &arguments) {
            return RemoteToolOutput {
                call_id: call.call_id,
                success: false,
                result: runner_result_value(RunnerToolResult {
                    tool,
                    status: RunnerToolStatus::Error,
                    data: None,
                    error: Some(error),
                }),
            };
        }

        if request.kind == ShellKind::PowerShell && !self.executor.supports_powershell() {
            return RemoteToolOutput {
                call_id: call.call_id,
                success: false,
                result: shell_result_value(
                    &tool,
                    &request,
                    &ShellExecutionResult {
                        stdout: String::new(),
                        stderr: "PowerShell execution is not available on this platform"
                            .to_string(),
                        exit_code: None,
                        timed_out: false,
                    },
                    Some(RunnerToolError {
                        code: "not_available",
                        message: "PowerShell execution is not available on this platform"
                            .to_string(),
                    }),
                ),
            };
        }

        let result = self.executor.execute(request.clone());
        let success = !result.timed_out && result.exit_code == Some(0);

        RemoteToolOutput {
            call_id: call.call_id,
            success,
            result: shell_result_value(&tool, &request, &result, None),
        }
    }
}

impl Default for ShellApprovalPolicy {
    fn default() -> Self {
        Self {
            mode: ShellApprovalPolicyMode::StaticNonce {
                expected_nonce: None,
            },
        }
    }
}

impl Default for ShellExecutionPolicy {
    fn default() -> Self {
        Self {
            configured: false,
            cwd_roots: Vec::new(),
            allowed_env_keys: BTreeSet::new(),
            path_entries: Vec::new(),
            clear_env: true,
        }
    }
}

impl ShellExecutionPolicy {
    #[must_use]
    pub fn with_cwd_roots<I, S>(mut self, roots: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.configured = true;
        self.cwd_roots = roots.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_allowed_env_keys<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.configured = true;
        self.allowed_env_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_path_entries<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.configured = true;
        self.path_entries = entries.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_clear_env(mut self, clear_env: bool) -> Self {
        self.configured = true;
        self.clear_env = clear_env;
        self
    }

    fn sanitize(
        &self,
        arguments: ShellExecArguments,
    ) -> Result<ShellExecutionPolicyDecision, RunnerToolError> {
        let idempotency_key =
            optional_non_empty(arguments.idempotency_key.as_deref(), "idempotency_key")?;
        if let (Some(idempotency_key), Some(lease)) = (idempotency_key.as_deref(), &arguments.lease)
            && lease.idempotency_key != idempotency_key
        {
            return Err(RunnerToolError {
                code: "approval_required",
                message: "shell execution idempotency_key does not match command lease".to_string(),
            });
        }

        if arguments.path.is_some() {
            return Err(RunnerToolError {
                code: "path_denied",
                message: "shell PATH is fixed by runner policy and cannot be supplied by request"
                    .to_string(),
            });
        }

        let cwd = optional_non_empty(arguments.cwd.as_deref(), "cwd")?;
        if let Some(cwd) = cwd.as_deref()
            && !self.cwd_allowed(cwd)
        {
            return Err(RunnerToolError {
                code: "cwd_denied",
                message: "shell cwd is outside runner policy".to_string(),
            });
        }

        let mut env = BTreeMap::new();
        if let Some(requested_env) = arguments.env {
            for (key, value) in requested_env {
                if !valid_env_key(&key) || dangerous_env_key(&key) {
                    return Err(RunnerToolError {
                        code: "env_denied",
                        message: format!("shell env key {key} is not allowed by runner policy"),
                    });
                }
                if !self.allowed_env_keys.is_empty() && !self.allowed_env_keys.contains(&key) {
                    return Err(RunnerToolError {
                        code: "env_denied",
                        message: format!("shell env key {key} is not allowed by runner policy"),
                    });
                }
                env.insert(key, value);
            }
        }

        Ok(ShellExecutionPolicyDecision {
            cwd,
            env,
            path: self.path_entries.clone(),
            clear_env: self.clear_env,
            allowed_env: self.allowed_env_keys.iter().cloned().collect(),
        })
    }

    fn cwd_allowed(&self, cwd: &str) -> bool {
        if self.cwd_roots.is_empty() {
            return false;
        }
        let cwd = normalize_absolute_path_for_policy(cwd);
        self.cwd_roots
            .iter()
            .map(|root| normalize_absolute_path_for_policy(root))
            .any(|root| cwd == root || cwd.starts_with(&format!("{root}/")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellExecutionPolicyDecision {
    cwd: Option<String>,
    env: BTreeMap<String, String>,
    path: Vec<String>,
    clear_env: bool,
    allowed_env: Vec<String>,
}

impl ShellApprovalPolicy {
    #[must_use]
    pub fn static_nonce(nonce: impl Into<String>) -> Self {
        Self {
            mode: ShellApprovalPolicyMode::StaticNonce {
                expected_nonce: Some(nonce.into()),
            },
        }
    }

    #[must_use]
    pub fn nonce_manager(manager: ShellApprovalNonceManager) -> Self {
        Self {
            mode: ShellApprovalPolicyMode::ManagedNonces(manager),
        }
    }

    fn check(
        &self,
        tool: &str,
        request: &ShellExecutionRequest,
        arguments: &Value,
    ) -> Result<(), RunnerToolError> {
        match &self.mode {
            ShellApprovalPolicyMode::StaticNonce { expected_nonce } => {
                let Some(actual_nonce) = arguments
                    .get("approval_nonce")
                    .and_then(Value::as_str)
                    .filter(|nonce| !nonce.is_empty())
                else {
                    return Err(RunnerToolError {
                        code: "approval_required",
                        message: "shell execution requires approval_nonce".to_string(),
                    });
                };
                let Some(expected_nonce) = expected_nonce.as_deref() else {
                    return Err(RunnerToolError {
                        code: "approval_required",
                        message: "shell execution requires runner approval policy".to_string(),
                    });
                };

                if actual_nonce != expected_nonce {
                    return Err(RunnerToolError {
                        code: "approval_denied",
                        message: "shell execution approval_nonce was not accepted".to_string(),
                    });
                }

                Ok(())
            }
            ShellApprovalPolicyMode::ManagedNonces(manager) => {
                if let Some(lease) = &request.lease {
                    manager.redeem_lease(lease, &command_lease_action(tool, request))
                } else {
                    let Some(actual_nonce) = arguments
                        .get("approval_nonce")
                        .and_then(Value::as_str)
                        .filter(|nonce| !nonce.is_empty())
                    else {
                        return Err(RunnerToolError {
                            code: "approval_required",
                            message: "shell execution requires approval_nonce".to_string(),
                        });
                    };
                    manager.redeem(actual_nonce)
                }
            }
        }
    }
}

impl ShellApprovalNonceManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, nonce: impl Into<String>, ttl: Option<Duration>) {
        let expires_at = ttl.map(|ttl| Instant::now() + ttl);
        self.state.lock().unwrap().insert(
            nonce.into(),
            ShellApprovalNonceRecord {
                status: ShellApprovalNonceStatus::Pending,
                expires_at,
                lease: None,
            },
        );
    }

    pub fn create_lease(&self, lease: CommandLeaseEnvelope, ttl: Option<Duration>) {
        let expires_at = ttl.map(|ttl| Instant::now() + ttl);
        self.state.lock().unwrap().insert(
            lease.id.clone(),
            ShellApprovalNonceRecord {
                status: ShellApprovalNonceStatus::Pending,
                expires_at,
                lease: Some(lease),
            },
        );
    }

    pub fn redeem(&self, nonce: &str) -> Result<(), RunnerToolError> {
        let mut state = self.state.lock().unwrap();
        let Some(record) = state.get_mut(nonce) else {
            return Err(RunnerToolError {
                code: "approval_denied",
                message: "shell execution approval_nonce was not accepted".to_string(),
            });
        };

        match record.status {
            ShellApprovalNonceStatus::Pending => {
                if record
                    .expires_at
                    .is_some_and(|expires_at| Instant::now() >= expires_at)
                {
                    record.status = ShellApprovalNonceStatus::Expired;
                    return Err(RunnerToolError {
                        code: "approval_expired",
                        message: "shell execution approval_nonce has expired".to_string(),
                    });
                }

                record.status = ShellApprovalNonceStatus::Redeemed;
                Ok(())
            }
            ShellApprovalNonceStatus::Redeemed => Err(RunnerToolError {
                code: "approval_replayed",
                message: "shell execution approval_nonce was already used".to_string(),
            }),
            ShellApprovalNonceStatus::Expired => Err(RunnerToolError {
                code: "approval_expired",
                message: "shell execution approval_nonce has expired".to_string(),
            }),
        }
    }

    pub fn redeem_lease(
        &self,
        lease: &CommandLeaseEnvelope,
        actual_action: &CommandLeaseAction,
    ) -> Result<(), RunnerToolError> {
        lease.validate_for_action(actual_action)?;

        let mut state = self.state.lock().unwrap();
        let Some(record) = state.get_mut(&lease.id) else {
            return Err(RunnerToolError {
                code: "approval_denied",
                message: "shell execution command lease was not accepted".to_string(),
            });
        };

        let Some(expected_lease) = record.lease.as_ref() else {
            return Err(RunnerToolError {
                code: "approval_required",
                message: "shell execution requires a command lease for this approval".to_string(),
            });
        };

        if expected_lease != lease || expected_lease.approved_action != *actual_action {
            return Err(RunnerToolError {
                code: "approval_required",
                message: "shell execution command lease does not match the approved action"
                    .to_string(),
            });
        }

        match record.status {
            ShellApprovalNonceStatus::Pending => {
                if record
                    .expires_at
                    .is_some_and(|expires_at| Instant::now() >= expires_at)
                {
                    record.status = ShellApprovalNonceStatus::Expired;
                    return Err(RunnerToolError {
                        code: "approval_expired",
                        message: "shell execution command lease has expired".to_string(),
                    });
                }

                record.status = ShellApprovalNonceStatus::Redeemed;
                Ok(())
            }
            ShellApprovalNonceStatus::Redeemed => Err(RunnerToolError {
                code: "approval_replayed",
                message: "shell execution command lease was already used".to_string(),
            }),
            ShellApprovalNonceStatus::Expired => Err(RunnerToolError {
                code: "approval_expired",
                message: "shell execution command lease has expired".to_string(),
            }),
        }
    }

    pub fn expire(&self, nonce: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        let Some(record) = state.get_mut(nonce) else {
            return false;
        };
        record.status = ShellApprovalNonceStatus::Expired;
        true
    }
}

impl SystemShellExecutor {
    #[must_use]
    pub fn enabled_for_tests() -> Self {
        Self {
            real_shell_enabled: true,
        }
    }
}

impl ShellExecutor for SystemShellExecutor {
    fn execute(&self, request: ShellExecutionRequest) -> ShellExecutionResult {
        if !self.real_shell_enabled {
            return ShellExecutionResult {
                stdout: String::new(),
                stderr: "real shell execution is not enabled for kai-runner".to_string(),
                exit_code: None,
                timed_out: false,
            };
        }

        let mut command = match request.kind {
            ShellKind::Shell => system_shell_command(&request.command),
            ShellKind::PowerShell => powershell_command(&request.command),
        };

        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }
        if request.clear_env {
            command.env_clear();
        }
        for (key, value) in &request.env {
            command.env(key, value);
        }
        if !request.path.is_empty() {
            command.env(
                "PATH",
                std::env::join_paths(&request.path).unwrap_or_default(),
            );
        }

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return ShellExecutionResult {
                    stdout: String::new(),
                    stderr: format!("failed to start command: {error}"),
                    exit_code: None,
                    timed_out: false,
                };
            }
        };

        wait_for_shell_output(child, request.timeout_ms)
    }

    fn supports_powershell(&self) -> bool {
        cfg!(windows)
    }

    fn requires_explicit_execution_policy(&self) -> bool {
        true
    }
}

impl WorkspaceKaiRunner {
    #[must_use]
    pub fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            tools: CapabilitySet::runner(),
        }
    }

    pub fn execute(&self, call: RemoteToolCall) -> RemoteToolOutput {
        if call.name == RemoteToolName::FileRead {
            return self.read_file(call);
        }
        if call.name == RemoteToolName::FileWrite {
            return self.write_file(call);
        }

        KaiRunner.execute(call)
    }

    pub fn handle_request(&self, request: RunnerRequest) -> RunnerResponse {
        match request {
            RunnerRequest::Capabilities => {
                let capabilities = self.capabilities();
                RunnerResponse::Capabilities {
                    mode: "runner".to_string(),
                    tools: capabilities
                        .tools
                        .tools
                        .iter()
                        .map(|tool| tool.as_str().to_string())
                        .collect(),
                }
            }
            RunnerRequest::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let Ok(name) = RemoteToolName::parse(&name) else {
                    return RunnerResponse::ToolCall {
                        call_id,
                        success: false,
                        result: runner_result_value(RunnerToolResult {
                            tool: name.clone(),
                            status: RunnerToolStatus::Error,
                            data: None,
                            error: Some(RunnerToolError {
                                code: "unknown_tool",
                                message: format!("unknown remote tool name: {name}"),
                            }),
                        }),
                    };
                };

                let output = self.execute(RemoteToolCall {
                    call_id,
                    name,
                    arguments,
                });

                RunnerResponse::ToolCall {
                    call_id: output.call_id,
                    success: output.success,
                    result: output.result,
                }
            }
        }
    }

    fn read_file(&self, call: RemoteToolCall) -> RemoteToolOutput {
        let tool = call.name.as_str().to_string();
        let result = match read_file_from_workspace(&self.workspace, call.arguments) {
            Ok(data) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Ok,
                data: Some(data),
                error: None,
            },
            Err(error) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Error,
                data: None,
                error: Some(error),
            },
        };

        RemoteToolOutput {
            call_id: call.call_id,
            success: result.status == RunnerToolStatus::Ok,
            result: runner_result_value(result),
        }
    }

    fn write_file(&self, call: RemoteToolCall) -> RemoteToolOutput {
        let tool = call.name.as_str().to_string();
        let result = match write_file_in_workspace(&self.workspace, call.arguments) {
            Ok(data) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Ok,
                data: Some(data),
                error: None,
            },
            Err(error) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Error,
                data: None,
                error: Some(error),
            },
        };

        RemoteToolOutput {
            call_id: call.call_id,
            success: result.status == RunnerToolStatus::Ok,
            result: runner_result_value(result),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FileReadArguments {
    path: String,
    start_line: Option<usize>,
    max_lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FileWriteArguments {
    path: String,
    content: String,
    #[serde(default)]
    mode: FileWriteMode,
    backup: Option<bool>,
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PackageInstallArguments {
    manager: Option<String>,
    package: Option<String>,
    packages: Option<Vec<String>>,
    dry_run: Option<bool>,
    operation: Option<String>,
    version: Option<String>,
    allow_sudo: Option<bool>,
    idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BrowserOpenArguments {
    url: Option<String>,
    profile: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BrowserExtractTextArguments {
    session_id: Option<String>,
    url: Option<String>,
    handle: Option<String>,
    selector: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BrowserClickArguments {
    session_id: Option<String>,
    selector: Option<String>,
    label: Option<String>,
    action: Option<String>,
    metadata: Option<Value>,
    idempotency_key: Option<String>,
    approval_nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiagnoseSystemArguments {
    profile: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ShellExecArguments {
    command: String,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
    env: Option<BTreeMap<String, String>>,
    path: Option<Value>,
    idempotency_key: Option<String>,
    lease: Option<CommandLeaseEnvelope>,
}

#[derive(Debug, Deserialize)]
struct PowerShellExecArguments {
    script: Option<String>,
    command: Option<String>,
    working_directory: Option<String>,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
    execution_policy: Option<String>,
    idempotency_key: Option<String>,
    risk_hint: Option<String>,
    approval_nonce: Option<String>,
    lease: Option<CommandLeaseEnvelope>,
}

#[derive(Debug, Deserialize)]
struct McpCallArguments {
    server: String,
    tool: String,
    arguments: Value,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileWriteMode {
    #[default]
    Overwrite,
    Append,
}

impl FileWriteMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Overwrite => "overwrite",
            Self::Append => "append",
        }
    }
}

fn mcp_tool_request(arguments: Value) -> Result<McpToolRequest, RunnerToolError> {
    let arguments: McpCallArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid MCP call arguments: {error}"),
        })?;

    let server =
        optional_non_empty(arguments.server.as_str().into(), "server")?.ok_or_else(|| {
            RunnerToolError {
                code: "invalid_arguments",
                message: "MCP call arguments must include server".to_string(),
            }
        })?;
    let tool = optional_non_empty(arguments.tool.as_str().into(), "tool")?.ok_or_else(|| {
        RunnerToolError {
            code: "invalid_arguments",
            message: "MCP call arguments must include tool".to_string(),
        }
    })?;

    if !arguments.arguments.is_object() {
        return Err(RunnerToolError {
            code: "invalid_arguments",
            message: "MCP call arguments.arguments must be an object".to_string(),
        });
    }

    Ok(McpToolRequest {
        server,
        tool,
        arguments: arguments.arguments,
    })
}

fn diagnose_system_value(tool: &str, arguments: Value, capabilities: RunnerCapabilities) -> Value {
    let profile = serde_json::from_value::<DiagnoseSystemArguments>(arguments)
        .ok()
        .and_then(|arguments| arguments.profile);
    let mut value = json!({
        "tool": tool,
        "status": RunnerToolStatus::Ok,
        "system": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
        },
        "capabilities": capabilities,
    });

    if profile.as_deref() == Some("windows_rescue") {
        value["diagnostic_plan"] = windows_rescue_diagnostic_plan();
    }

    value
}

fn windows_rescue_diagnostic_plan() -> Value {
    json!({
        "profile": "windows_rescue",
        "status": "planned",
        "would_execute": false,
        "checks": [
            windows_rescue_check(
                "path_environment",
                "Inspect user, machine, and effective PATH entries for missing installer locations.",
                "[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::GetEnvironmentVariable('Path','Machine'); $env:Path -split ';'",
                "low",
                false,
                "medium",
            ),
            windows_rescue_check(
                "winget_health",
                "Check whether winget exists and can report package manager state.",
                "Get-Command winget -ErrorAction SilentlyContinue; if (Get-Command winget -ErrorAction SilentlyContinue) { winget --info }",
                "low",
                false,
                "medium",
            ),
            windows_rescue_check(
                "event_log_installer_errors",
                "Collect recent Windows Installer and application install errors from Event Log.",
                "Get-WinEvent -FilterHashtable @{LogName='Application'; StartTime=(Get-Date).AddDays(-7)} -MaxEvents 50 | Where-Object { $_.ProviderName -eq 'MsiInstaller' -or $_.Message -match 'install|setup|winget|python|node|visual studio code|vscode' } | Select-Object TimeCreated,ProviderName,Id,LevelDisplayName,Message",
                "low",
                false,
                "high",
            ),
            windows_rescue_check(
                "uac_elevation_state",
                "Read UAC policy signals that explain admin prompts or blocked installers.",
                "Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies\\System' | Select-Object EnableLUA,ConsentPromptBehaviorAdmin,PromptOnSecureDesktop",
                "low",
                true,
                "high",
            ),
            windows_rescue_check(
                "python_install_failure",
                "Check Python launcher, PATH visibility, winget registration, and common install log locations.",
                "Get-Command python,py -ErrorAction SilentlyContinue; if (Get-Command winget -ErrorAction SilentlyContinue) { winget list --name Python --accept-source-agreements }; Get-ChildItem \"$env:TEMP\" -Filter '*python*log*' -ErrorAction SilentlyContinue | Select-Object FullName,Length,LastWriteTime",
                "low",
                true,
                "medium",
            ),
            windows_rescue_check(
                "node_install_failure",
                "Check Node.js and npm visibility, winget registration, and npm prefix state.",
                "Get-Command node,npm -ErrorAction SilentlyContinue; if (Get-Command winget -ErrorAction SilentlyContinue) { winget list --name Node --accept-source-agreements }; npm config get prefix 2>$null",
                "low",
                true,
                "medium",
            ),
            windows_rescue_check(
                "vscode_install_failure",
                "Check VS Code command registration, winget state, and setup logs.",
                "Get-Command code -ErrorAction SilentlyContinue; if (Get-Command winget -ErrorAction SilentlyContinue) { winget list --id Microsoft.VisualStudioCode --accept-source-agreements }; Get-ChildItem \"$env:LOCALAPPDATA\\Temp\" -Filter '*vscode*log*' -ErrorAction SilentlyContinue | Select-Object FullName,Length,LastWriteTime",
                "low",
                true,
                "medium",
            )
        ],
    })
}

fn windows_rescue_check(
    code: &str,
    description: &str,
    script: &str,
    risk: &str,
    requires_approval_for_fix: bool,
    risk_for_fix: &str,
) -> Value {
    json!({
        "code": code,
        "description": description,
        "tool": "remote.powershell.exec",
        "script": script,
        "status": "planned",
        "would_execute": false,
        "risk": risk,
        "requires_approval_for_fix": requires_approval_for_fix,
        "risk_for_fix": risk_for_fix,
    })
}

fn powershell_exec_scaffold(call: RemoteToolCall) -> RemoteToolOutput {
    let tool = call.name.as_str().to_string();
    let result = match powershell_scaffold_value(call.arguments) {
        Ok(result) => result,
        Err(error) => runner_result_value(RunnerToolResult {
            tool,
            status: RunnerToolStatus::Error,
            data: None,
            error: Some(error),
        }),
    };

    RemoteToolOutput {
        call_id: call.call_id,
        success: false,
        result,
    }
}

fn powershell_scaffold_value(arguments: Value) -> Result<Value, RunnerToolError> {
    let arguments: PowerShellExecArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid PowerShell execution arguments: {error}"),
        })?;
    let script = optional_non_empty(
        arguments.script.as_deref().or(arguments.command.as_deref()),
        "script",
    )?
    .ok_or_else(|| RunnerToolError {
        code: "invalid_arguments",
        message: "PowerShell execution arguments must include script".to_string(),
    })?;
    let working_directory = optional_non_empty(
        arguments
            .working_directory
            .as_deref()
            .or(arguments.cwd.as_deref()),
        "working_directory",
    )?;
    let risk = powershell_risk(&script, arguments.risk_hint.as_deref());
    let mut approval_reasons = powershell_approval_reasons(&script);
    let lease_action = CommandLeaseAction::new(
        "remote.powershell.exec",
        script.clone(),
        working_directory.clone(),
    );
    let lease_error = arguments
        .lease
        .as_ref()
        .and_then(|lease| lease.validate_for_action(&lease_action).err());
    if lease_error.is_some() {
        approval_reasons.push("lease_action_mismatch");
    }
    let requires_approval = risk == "high" || lease_error.is_some();
    let mut result = json!({
        "tool": "remote.powershell.exec",
        "status": "planned",
        "blocked": true,
        "would_execute": false,
        "script": script,
        "risk": risk,
        "requires_approval": requires_approval,
        "stdout": "",
        "stderr": "",
        "exit_code": null,
        "timed_out": false,
        "error": {
            "code": if requires_approval { "approval_required" } else { "blocked" },
            "message": if requires_approval {
                "PowerShell repair is high risk and requires approval before any future live execution"
            } else {
                "kai-runner scaffold does not execute local shell commands; PowerShell script was planned only"
            }
        }
    });

    if let Some(working_directory) = working_directory {
        result["working_directory"] = json!(working_directory);
    }
    if let Some(timeout_ms) = arguments.timeout_ms {
        result["timeout_ms"] = json!(timeout_ms);
    }
    if let Some(execution_policy) = arguments.execution_policy {
        result["execution_policy"] = json!(execution_policy);
    }
    if let Some(idempotency_key) = arguments.idempotency_key {
        result["idempotency_key"] = json!(idempotency_key);
    }
    if let Some(lease) = arguments.lease {
        result["lease"] = json!(lease);
    }
    if arguments
        .approval_nonce
        .as_deref()
        .is_some_and(|nonce| !nonce.is_empty())
    {
        result["approval"]["provided"] = json!(true);
    }
    if requires_approval {
        result["approval"] = json!({
            "required": true,
            "risk": "high",
            "reasons": approval_reasons,
        });
    }

    Ok(result)
}

fn powershell_risk(script: &str, risk_hint: Option<&str>) -> &'static str {
    if risk_hint.is_some_and(|hint| hint.eq_ignore_ascii_case("high"))
        || !powershell_approval_reasons(script).is_empty()
    {
        "high"
    } else if risk_hint.is_some_and(|hint| hint.eq_ignore_ascii_case("medium")) {
        "medium"
    } else {
        "low"
    }
}

fn powershell_approval_reasons(script: &str) -> Vec<&'static str> {
    let lower = script.to_ascii_lowercase();
    let mut reasons = Vec::new();
    if lower.contains("-verb runas")
        || lower.contains("start-process") && lower.contains("runas")
        || lower.contains("promptforcredential")
    {
        reasons.push("uac_elevation");
    }
    if lower.contains("set-executionpolicy") || lower.contains("executionpolicy bypass") {
        reasons.push("execution_policy_change");
    }
    if lower.contains("hklm:\\")
        || lower.contains("hkcu:\\")
        || lower.contains("set-itemproperty")
        || lower.contains("new-itemproperty")
        || lower.contains("reg add")
        || lower.contains("reg delete")
    {
        reasons.push("registry_edit");
    }
    if lower.contains("new-service")
        || lower.contains("set-service")
        || lower.contains("start-service")
        || lower.contains("stop-service")
        || lower.contains("delete service")
    {
        reasons.push("service_change");
    }
    if lower.contains("invoke-webrequest")
        || lower.contains("iwr ")
        || lower.contains("curl ")
        || lower.contains("start-bitstransfer")
    {
        reasons.push("script_download");
    }
    reasons
}

fn shell_execution_request(
    kind: ShellKind,
    arguments: Value,
    policy: &ShellExecutionPolicy,
) -> Result<ShellExecutionRequest, RunnerToolError> {
    if kind == ShellKind::PowerShell {
        let arguments: PowerShellExecArguments =
            serde_json::from_value(arguments).map_err(|error| RunnerToolError {
                code: "invalid_arguments",
                message: format!("invalid PowerShell execution arguments: {error}"),
            })?;
        let command = optional_non_empty(
            arguments.script.as_deref().or(arguments.command.as_deref()),
            "script",
        )?
        .ok_or_else(|| RunnerToolError {
            code: "invalid_arguments",
            message: "PowerShell execution arguments must include script".to_string(),
        })?;
        let shell_arguments = ShellExecArguments {
            command: command.clone(),
            cwd: arguments.working_directory.or(arguments.cwd),
            timeout_ms: arguments.timeout_ms,
            env: None,
            path: None,
            idempotency_key: arguments.idempotency_key,
            lease: arguments.lease,
        };
        let timeout_ms = shell_arguments.timeout_ms;
        let lease = shell_arguments.lease.clone();
        let decision = policy.sanitize(shell_arguments)?;

        return Ok(ShellExecutionRequest {
            kind,
            command,
            cwd: decision.cwd,
            timeout_ms,
            env: decision.env,
            path: decision.path,
            clear_env: decision.clear_env,
            lease,
        });
    }

    let arguments: ShellExecArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid shell execution arguments: {error}"),
        })?;

    let command =
        optional_non_empty(arguments.command.as_str().into(), "command")?.ok_or_else(|| {
            RunnerToolError {
                code: "invalid_arguments",
                message: "shell execution arguments must include command".to_string(),
            }
        })?;
    let timeout_ms = arguments.timeout_ms;
    let lease = arguments.lease.clone();
    let decision = policy.sanitize(arguments)?;

    Ok(ShellExecutionRequest {
        kind,
        command,
        cwd: decision.cwd,
        timeout_ms,
        env: decision.env,
        path: decision.path,
        clear_env: decision.clear_env,
        lease,
    })
}

fn shell_result_value(
    tool: &str,
    request: &ShellExecutionRequest,
    result: &ShellExecutionResult,
    error: Option<RunnerToolError>,
) -> Value {
    let success = !result.timed_out && result.exit_code == Some(0) && error.is_none();
    let mut value = json!({
        "tool": tool,
        "status": if success { "ok" } else { "error" },
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exit_code": result.exit_code,
        "timed_out": result.timed_out,
        "command": request.command,
    });

    if let Some(cwd) = &request.cwd {
        value["cwd"] = json!(cwd);
    }
    if !request.env.is_empty() || !request.path.is_empty() || !request.clear_env {
        value["policy"] = json!({
            "clear_env": request.clear_env,
            "allowed_env": policy_env_keys(request),
            "path": request.path,
        });
    }
    if let Some(error) = error {
        value["error"] = json!(error);
    }
    if let Some(lease) = &request.lease {
        value["lease"] = json!(lease);
    }

    value
}

fn command_lease_action(tool: &str, request: &ShellExecutionRequest) -> CommandLeaseAction {
    CommandLeaseAction {
        tool: tool.to_string(),
        command: request.command.clone(),
        cwd: request.cwd.clone(),
    }
}

fn policy_env_keys(request: &ShellExecutionRequest) -> Vec<&str> {
    request.env.keys().map(String::as_str).collect()
}

fn valid_env_key(key: &str) -> bool {
    !key.is_empty()
        && key != "PATH"
        && key
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn dangerous_env_key(key: &str) -> bool {
    key == "PATH"
        || key == "IFS"
        || key == "BASH_ENV"
        || key == "ENV"
        || key == "SHELLOPTS"
        || key == "SSH_AUTH_SOCK"
        || key == "GIT_SSH_COMMAND"
        || key == "RUSTC_WRAPPER"
        || key.starts_with("LD_")
        || key.starts_with("DYLD_")
        || key.starts_with("AWS_")
        || key.ends_with("_API_KEY")
        || key.ends_with("_ACCESS_KEY")
        || key.ends_with("_SECRET_KEY")
        || key.ends_with("_TOKEN")
}

fn normalize_absolute_path_for_policy(path: &str) -> String {
    let mut parts = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::RootDir | Component::Prefix(_) => parts.clear(),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
        }
    }
    format!("/{}", parts.join("/"))
        .trim_end_matches('/')
        .to_string()
}

fn system_shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut process = Command::new("cmd.exe");
        process.args(["/C", command]);
        process
    }

    #[cfg(not(windows))]
    {
        let mut process = Command::new("sh");
        process.args(["-c", command]);
        process
    }
}

fn powershell_command(command: &str) -> Command {
    let mut process = Command::new("powershell.exe");
    process.args(["-NoProfile", "-Command", command]);
    process
}

fn wait_for_shell_output(
    mut child: std::process::Child,
    timeout_ms: Option<u64>,
) -> ShellExecutionResult {
    if let Some(timeout_ms) = timeout_ms {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return collect_shell_output(child, false),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    return collect_shell_output(child, true);
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let _ = child.kill();
                    return ShellExecutionResult {
                        stdout: String::new(),
                        stderr: format!("failed to wait for command: {error}"),
                        exit_code: None,
                        timed_out: false,
                    };
                }
            }
        }
    }

    collect_shell_output(child, false)
}

fn collect_shell_output(child: std::process::Child, timed_out: bool) -> ShellExecutionResult {
    match child.wait_with_output() {
        Ok(output) => ShellExecutionResult {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code(),
            timed_out,
        },
        Err(error) => ShellExecutionResult {
            stdout: String::new(),
            stderr: format!("failed to collect command output: {error}"),
            exit_code: None,
            timed_out,
        },
    }
}

fn read_file_from_workspace(workspace: &Path, arguments: Value) -> Result<Value, RunnerToolError> {
    let arguments: FileReadArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid file read arguments: {error}"),
        })?;

    let relative_path = Path::new(&arguments.path);
    if arguments.path.is_empty() || relative_path.is_absolute() {
        return Err(RunnerToolError {
            code: "path_escape",
            message: "file path must be relative to the runner workspace".to_string(),
        });
    }

    let relative_path = normalize_relative_path(relative_path)?;
    let target = workspace.join(relative_path);
    let canonical_target = target.canonicalize().map_err(|error| RunnerToolError {
        code: "read_failed",
        message: format!("failed to read {}: {error}", arguments.path),
    })?;

    if !canonical_target.starts_with(workspace) {
        return Err(RunnerToolError {
            code: "path_escape",
            message: "file path escapes the runner workspace".to_string(),
        });
    }

    let bytes = std::fs::read(&canonical_target).map_err(|error| RunnerToolError {
        code: "read_failed",
        message: format!("failed to read {}: {error}", arguments.path),
    })?;
    let content = String::from_utf8(bytes).map_err(|_| RunnerToolError {
        code: "not_utf8",
        message: format!("{} is not valid UTF-8 text", arguments.path),
    })?;

    let total_lines = content.lines().count();
    let start_line = arguments.start_line.unwrap_or(1).max(1);
    let max_lines = arguments.max_lines.unwrap_or(usize::MAX);
    let selected_content = select_lines(&content, start_line, max_lines);
    let selected_count = selected_content.lines().count();
    let next_start_line = start_line.saturating_add(selected_count);
    let truncated = next_start_line <= total_lines;

    Ok(json!({
        "path": arguments.path,
        "content": selected_content,
        "truncated": truncated,
        "total_lines": total_lines,
        "start_line": start_line,
        "next_start_line": if truncated {
            Value::from(next_start_line)
        } else {
            Value::Null
        },
    }))
}

fn write_file_in_workspace(workspace: &Path, arguments: Value) -> Result<Value, RunnerToolError> {
    let arguments: FileWriteArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid file write arguments: {error}"),
        })?;
    let idempotency_key =
        optional_non_empty(arguments.idempotency_key.as_deref(), "idempotency_key")?;

    let relative_path = validate_workspace_relative_path(&arguments.path)?;
    let target = workspace.join(&relative_path);
    let parent = target.parent().ok_or_else(|| RunnerToolError {
        code: "path_escape",
        message: "file path must be relative to the runner workspace".to_string(),
    })?;

    std::fs::create_dir_all(parent).map_err(|error| RunnerToolError {
        code: "write_failed",
        message: format!(
            "failed to create parent directories for {}: {error}",
            arguments.path
        ),
    })?;

    let canonical_parent = parent.canonicalize().map_err(|error| RunnerToolError {
        code: "write_failed",
        message: format!(
            "failed to inspect parent directory for {}: {error}",
            arguments.path
        ),
    })?;
    if !canonical_parent.starts_with(workspace) {
        return Err(RunnerToolError {
            code: "path_escape",
            message: "file path escapes the runner workspace".to_string(),
        });
    }

    let target_exists = target.exists();
    if target_exists {
        let canonical_target = target.canonicalize().map_err(|error| RunnerToolError {
            code: "write_failed",
            message: format!("failed to inspect {}: {error}", arguments.path),
        })?;
        if !canonical_target.starts_with(workspace) {
            return Err(RunnerToolError {
                code: "path_escape",
                message: "file path escapes the runner workspace".to_string(),
            });
        }
    }

    let backup_path = match arguments.mode {
        FileWriteMode::Overwrite if target_exists && arguments.backup.unwrap_or(true) => Some(
            create_backup(workspace, &relative_path, &target, &arguments.path)?,
        ),
        _ => None,
    };

    match arguments.mode {
        FileWriteMode::Overwrite => {
            std::fs::write(&target, &arguments.content).map_err(|error| RunnerToolError {
                code: "write_failed",
                message: format!("failed to write {}: {error}", arguments.path),
            })?;
        }
        FileWriteMode::Append => {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target)
                .map_err(|error| RunnerToolError {
                    code: "write_failed",
                    message: format!("failed to open {} for append: {error}", arguments.path),
                })?;
            file.write_all(arguments.content.as_bytes())
                .map_err(|error| RunnerToolError {
                    code: "write_failed",
                    message: format!("failed to append to {}: {error}", arguments.path),
                })?;
        }
    }

    let mut data = json!({
        "path": arguments.path,
        "mode": arguments.mode.as_str(),
        "bytes_written": arguments.content.len(),
        "backup_path": backup_path,
    });
    if let Some(idempotency_key) = idempotency_key {
        data["idempotency_key"] = json!(idempotency_key);
    }

    Ok(data)
}

fn browser_tool(call: RemoteToolCall) -> RemoteToolOutput {
    browser_tool_with_registry(call, None, &ScaffoldBrowserEngine)
}

fn browser_tool_with_registry(
    call: RemoteToolCall,
    registry: Option<&BrowserSessionRegistry>,
    engine: &dyn BrowserEngine,
) -> RemoteToolOutput {
    let tool = call.name.as_str().to_string();
    let result = match call.name {
        RemoteToolName::BrowserOpen => match browser_open_preview(call.arguments, registry, engine) {
            Ok(data) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Ok,
                data: Some(data),
                error: None,
            },
            Err(error) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Error,
                data: None,
                error: Some(error),
            },
        },
        RemoteToolName::BrowserExtractText => {
            browser_extract_text_result(tool, call.arguments, registry, engine)
        }
        RemoteToolName::BrowserClick => match browser_click_approval(call.arguments, registry, engine) {
            Ok(approval) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Error,
                data: Some(browser_click_approval_data(approval)),
                error: Some(RunnerToolError {
                    code: "approval_required",
                    message: "browser.click requires approval because click or submit actions can change page state".to_string(),
                }),
            },
            Err(error) => RunnerToolResult {
                tool,
                status: RunnerToolStatus::Error,
                data: None,
                error: Some(error),
            },
        },
        _ => unreachable!("browser_tool only handles browser remote tools"),
    };

    RemoteToolOutput {
        call_id: call.call_id,
        success: result.status == RunnerToolStatus::Ok,
        result: runner_result_value(result),
    }
}

fn browser_extract_text_result(
    tool: String,
    arguments: Value,
    registry: Option<&BrowserSessionRegistry>,
    engine: &dyn BrowserEngine,
) -> RunnerToolResult {
    let target = match validate_browser_extract_text(arguments, registry) {
        Ok(target) => target,
        Err(error) => {
            return RunnerToolResult {
                tool,
                status: RunnerToolStatus::Error,
                data: None,
                error: Some(error),
            };
        }
    };

    let Some(request) = target.request.as_ref() else {
        return RunnerToolResult {
            tool,
            status: RunnerToolStatus::Error,
            data: None,
            error: Some(RunnerToolError {
                code: "browser_unavailable",
                message: "kai-runner scaffold has no browser engine yet".to_string(),
            }),
        };
    };

    match engine.extract_text(request.clone()) {
        Ok(extract_result) => RunnerToolResult {
            tool,
            status: RunnerToolStatus::Ok,
            data: Some(browser_extract_text_data(request, extract_result)),
            error: None,
        },
        Err(error) => RunnerToolResult {
            tool,
            status: RunnerToolStatus::Error,
            data: Some(browser_extract_text_metadata(request)),
            error: Some(error),
        },
    }
}

fn browser_open_preview(
    arguments: Value,
    registry: Option<&BrowserSessionRegistry>,
    engine: &dyn BrowserEngine,
) -> Result<Value, RunnerToolError> {
    let arguments: BrowserOpenArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid browser.open arguments: {error}"),
        })?;

    let url = required_browser_argument(arguments.url.as_deref(), "url")?;
    let profile = optional_browser_argument(arguments.profile.as_deref(), "profile")?;

    let open_result = engine.open(BrowserOpenRequest {
        url: url.to_string(),
        profile: profile.clone(),
    });
    let open_result = open_result?;

    let session_url = open_result.url.clone();
    let session_profile = open_result.profile.clone();
    let mut data = browser_open_data(open_result);

    if let Some(registry) = registry {
        let session = registry.create_session(BrowserSessionMetadata {
            url: Some(session_url),
            profile: session_profile,
        });
        data["session_id"] = json!(session.id);
    }

    Ok(data)
}

fn browser_open_data(result: BrowserOpenResult) -> Value {
    json!({
        "status": result.status,
        "url": result.url,
        "profile": result.profile,
        "events": result
            .events
            .into_iter()
            .map(|event| json!({
                "type": event.kind,
                "message": event.message,
            }))
            .collect::<Vec<_>>(),
        "next_approval_required": result.next_approval_required,
    })
}

#[derive(Debug, Clone)]
struct BrowserExtractTextTarget {
    request: Option<BrowserExtractTextRequest>,
}

fn validate_browser_extract_text(
    arguments: Value,
    registry: Option<&BrowserSessionRegistry>,
) -> Result<BrowserExtractTextTarget, RunnerToolError> {
    let arguments: BrowserExtractTextArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid browser.extract_text arguments: {error}"),
        })?;

    let session = match registry {
        Some(registry) => Some(validate_browser_session(
            arguments.session_id.as_deref(),
            registry,
        )?),
        None => None,
    };
    let url = optional_browser_argument(arguments.url.as_deref(), "url")?;
    let handle = optional_browser_argument(arguments.handle.as_deref(), "handle")?;
    let selector = optional_browser_argument(arguments.selector.as_deref(), "selector")?;

    if session.is_none() && url.is_none() && handle.is_none() {
        return Err(RunnerToolError {
            code: "invalid_arguments",
            message: "browser.extract_text arguments must include url or handle".to_string(),
        });
    }

    Ok(BrowserExtractTextTarget {
        request: session.map(|session| BrowserExtractTextRequest {
            session_id: session.id,
            url: url.or(session.url),
            selector,
            handle,
        }),
    })
}

fn browser_click_approval(
    arguments: Value,
    registry: Option<&BrowserSessionRegistry>,
    engine: &dyn BrowserEngine,
) -> Result<BrowserClickApproval, RunnerToolError> {
    let arguments: BrowserClickArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid browser.click arguments: {error}"),
        })?;

    let session_id = registry
        .map(|registry| validate_browser_session_id(arguments.session_id.as_deref(), registry))
        .transpose()?;
    let selector = optional_browser_argument(arguments.selector.as_deref(), "selector")?;
    let label = optional_browser_argument(arguments.label.as_deref(), "label")?;
    let action = optional_browser_argument(arguments.action.as_deref(), "action")?
        .unwrap_or_else(|| "click".to_string());

    engine.click(BrowserClickRequest {
        session_id,
        selector,
        label,
        action,
        metadata: arguments.metadata,
        idempotency_key: optional_non_empty(
            arguments.idempotency_key.as_deref(),
            "idempotency_key",
        )?,
        approval_nonce: optional_non_empty(arguments.approval_nonce.as_deref(), "approval_nonce")?,
    })
}

fn validate_browser_session_id(
    session_id: Option<&str>,
    registry: &BrowserSessionRegistry,
) -> Result<String, RunnerToolError> {
    let session_id = required_browser_argument(session_id, "session_id")?;
    if registry.get_session(&session_id).is_none() {
        return Err(RunnerToolError {
            code: "unknown_browser_session",
            message: format!("unknown browser session: {session_id}"),
        });
    }

    Ok(session_id)
}

fn validate_browser_session(
    session_id: Option<&str>,
    registry: &BrowserSessionRegistry,
) -> Result<BrowserSession, RunnerToolError> {
    let session_id = required_browser_argument(session_id, "session_id")?;
    registry
        .get_session(&session_id)
        .ok_or_else(|| RunnerToolError {
            code: "unknown_browser_session",
            message: format!("unknown browser session: {session_id}"),
        })
}

fn browser_extract_text_data(
    request: &BrowserExtractTextRequest,
    result: BrowserExtractTextResult,
) -> Value {
    let mut data = browser_extract_text_metadata(request);
    data["text"] = json!(result.text);
    data
}

fn browser_extract_text_metadata(request: &BrowserExtractTextRequest) -> Value {
    let mut data = json!({
        "session_id": request.session_id,
    });
    if let Some(selector) = &request.selector {
        data["selector"] = json!(selector);
    }
    if let Some(url) = &request.url {
        data["url"] = json!(url);
    }
    if let Some(handle) = &request.handle {
        data["handle"] = json!(handle);
    }
    data
}

fn browser_click_approval_data(approval: BrowserClickApproval) -> Value {
    let mut data = json!({
        "action": approval.action,
        "approval_required": true,
        "risk": approval.risk,
    });
    if let Some(session_id) = &approval.session_id {
        data["session_id"] = json!(session_id);
    }
    if let Some(selector) = &approval.selector {
        data["selector"] = json!(selector);
    }
    if let Some(label) = &approval.label {
        data["label"] = json!(label);
    }
    if let Some(metadata) = approval.metadata {
        data["metadata"] = metadata;
    }
    if let Some(idempotency_key) = approval.idempotency_key {
        data["idempotency_key"] = json!(idempotency_key);
    }
    if approval.approval_nonce.is_some() {
        data["approval"] = json!({
            "nonce": {
                "label": "approval_nonce",
                "status": "provided",
                "redacted": true
            }
        });
    }

    let mut audit = json!({
        "type": "browser_action_blocked",
        "tool": "remote.browser.click",
        "action": data["action"].clone(),
        "reason": "approval_required",
    });
    if let Some(session_id) = data.get("session_id") {
        audit["session_id"] = session_id.clone();
    }
    if let Some(selector) = data.get("selector") {
        audit["selector"] = selector.clone();
    }
    data["audit"] = json!([audit]);

    data
}

fn runner_maintenance_plan(
    action: RunnerMaintenanceAction,
    options: RunnerMaintenanceOptions,
) -> Result<RunnerMaintenancePlan, RunnerToolError> {
    if options.execute {
        return Err(RunnerToolError {
            code: "approval_required",
            message: "runner maintenance actions are not executable in this scaffold".to_string(),
        });
    }

    if !options.dry_run {
        return Err(RunnerToolError {
            code: "not_executable",
            message: "runner maintenance actions are dry-run only in this scaffold".to_string(),
        });
    }

    let (preflight_steps, maintenance_steps, artifact_verification, rollback_steps, commands) =
        match action {
        RunnerMaintenanceAction::SelfUpdate => (
            vec![
                "confirm installed deepseek and deepseek-tui binary paths",
                "record current versions and modification times",
                "resolve the trusted release source without using stored credentials",
                "capture rollback location for the currently installed binaries",
            ],
            vec![
                "download candidate kai-runner, deepseek, and deepseek-tui artifacts into a temporary staging directory",
                "verify checksums or signatures before any replacement is allowed",
                "stage replacement binaries next to the current install without mutating the active install",
                "replace binaries only after explicit local approval, preserving previous binaries for rollback",
                "run post-update version and health checks before declaring the update complete",
            ],
            vec![
                RunnerArtifactVerificationPlan {
                    platform: "macos".to_string(),
                    artifact_name: "kai-runner-aarch64-apple-darwin.tar.gz".to_string(),
                    expected_checksum_source:
                        "release manifest sha256 entry for kai-runner-aarch64-apple-darwin.tar.gz"
                            .to_string(),
                    expected_signature_source:
                        "release manifest signature or detached .sig published beside the artifact"
                            .to_string(),
                    verify_step:
                        "verify sha256 checksum, then verify signature before staging kai-runner-aarch64-apple-darwin.tar.gz"
                            .to_string(),
                    verify_command:
                        "shasum -a 256 -c SHA256SUMS && cosign verify-blob --signature kai-runner-aarch64-apple-darwin.tar.gz.sig kai-runner-aarch64-apple-darwin.tar.gz"
                            .to_string(),
                    failure_rollback_guidance:
                        "abort self-update, keep active binaries unchanged, preserve staged artifact and verification output for inspection"
                            .to_string(),
                },
                RunnerArtifactVerificationPlan {
                    platform: "windows".to_string(),
                    artifact_name: "kai-runner-x86_64-pc-windows-msvc.zip".to_string(),
                    expected_checksum_source:
                        "release manifest sha256 entry for kai-runner-x86_64-pc-windows-msvc.zip"
                            .to_string(),
                    expected_signature_source:
                        "release manifest signature or detached .sig published beside the artifact"
                            .to_string(),
                    verify_step:
                        "verify sha256 checksum, then verify signature before staging kai-runner-x86_64-pc-windows-msvc.zip"
                            .to_string(),
                    verify_command:
                        "powershell -NoProfile -Command \"Get-FileHash kai-runner-x86_64-pc-windows-msvc.zip -Algorithm SHA256\"; cosign verify-blob --signature kai-runner-x86_64-pc-windows-msvc.zip.sig kai-runner-x86_64-pc-windows-msvc.zip"
                            .to_string(),
                    failure_rollback_guidance:
                        "abort self-update, keep active binaries unchanged, preserve staged artifact and verification output for inspection"
                            .to_string(),
                },
            ],
            vec![
                "restore the preserved previous binaries if verification or post-update checks fail",
                "leave the temporary staging directory available for inspection until cleanup is approved",
            ],
            vec![
                "deepseek --version",
                "deepseek-tui --version",
                "download kai-runner release artifact to a temporary staging directory",
                "verify release checksum or signature",
                "replace deepseek and deepseek-tui from verified staging after explicit approval",
                "rollback to preserved binaries if health checks fail",
            ],
        ),
        RunnerMaintenanceAction::Uninstall => (
            vec![
                "confirm installed deepseek and deepseek-tui binary paths",
                "record current versions and modification times",
                "confirm user data and config retention policy before removing binaries",
                "capture reinstall instructions and current binary metadata for rollback",
            ],
            vec![
                "stop any active kai-runner or deepseek-tui process after explicit local approval",
                "remove deepseek and deepseek-tui binaries only after confirming the resolved install paths",
                "cleanup runner launch agents, shims, or temporary installer files without touching user data",
                "check that removed binaries are absent and retained config paths still exist",
            ],
            Vec::new(),
            vec![
                "reinstall the previously recorded deepseek and deepseek-tui versions if removal was unintended",
                "restore launch metadata from the recorded preflight state when available",
            ],
            vec![
                "deepseek --version",
                "deepseek-tui --version",
                "stop active kai-runner or deepseek-tui process after explicit approval",
                "remove resolved deepseek and deepseek-tui binaries after explicit approval",
                "cleanup runner shims and temporary installer files",
                "check resolved binary paths are absent while user data remains",
            ],
        ),
    };

    let safety_guidance = vec![
        match action {
            RunnerMaintenanceAction::SelfUpdate => {
                "dry-run only: no download, install, replacement, or deletion is executed by this plan"
            }
            RunnerMaintenanceAction::Uninstall => {
                "dry-run only: no process stop, file removal, cleanup, or uninstall is executed by this plan"
            }
        },
        match action {
            RunnerMaintenanceAction::SelfUpdate => {
                "high risk: requires explicit local approval before any future maintenance executor may mutate binaries"
            }
            RunnerMaintenanceAction::Uninstall => {
                "high risk: requires explicit local approval before any future maintenance executor may remove files"
            }
        },
        "exclude credentials, pairing material, and authorization headers from maintenance output",
    ];

    Ok(RunnerMaintenancePlan {
        action,
        commands: commands.into_iter().map(str::to_string).collect(),
        preflight_steps: preflight_steps.into_iter().map(str::to_string).collect(),
        maintenance_steps: maintenance_steps.into_iter().map(str::to_string).collect(),
        artifact_verification,
        rollback_steps: rollback_steps.into_iter().map(str::to_string).collect(),
        safety_guidance: safety_guidance.into_iter().map(str::to_string).collect(),
        requires_approval: true,
        dry_run: true,
        risk: RunnerMaintenanceRisk::High,
        idempotency_key: options.idempotency_key.filter(|key| !key.trim().is_empty()),
    })
}

fn package_install(call: RemoteToolCall) -> RemoteToolOutput {
    let tool = call.name.as_str().to_string();
    let result = match package_install_preview(call.arguments) {
        Ok(data) => RunnerToolResult {
            tool,
            status: RunnerToolStatus::Ok,
            data: Some(data),
            error: None,
        },
        Err(error) => RunnerToolResult {
            tool,
            status: RunnerToolStatus::Error,
            data: None,
            error: Some(error),
        },
    };

    RemoteToolOutput {
        call_id: call.call_id,
        success: result.status == RunnerToolStatus::Ok,
        result: runner_result_value(result),
    }
}

fn package_install_preview(arguments: Value) -> Result<Value, RunnerToolError> {
    let arguments: PackageInstallArguments =
        serde_json::from_value(arguments).map_err(|error| RunnerToolError {
            code: "invalid_arguments",
            message: format!("invalid package install arguments: {error}"),
        })?;

    let manager = required_non_empty(arguments.manager.as_deref(), "manager")?;
    let operation = optional_non_empty(arguments.operation.as_deref(), "operation")?
        .unwrap_or_else(|| "install".to_string());
    let packages = package_install_packages(arguments.package, arguments.packages)?;
    let idempotency_key =
        optional_non_empty(arguments.idempotency_key.as_deref(), "idempotency_key")?;
    let _version = optional_non_empty(arguments.version.as_deref(), "version")?;

    if !arguments.dry_run.unwrap_or(true) {
        return Err(RunnerToolError {
            code: "approval_required",
            message: "kai-runner scaffold does not execute package installs".to_string(),
        });
    }

    let mut command_args = Vec::with_capacity(packages.len() + 2);
    command_args.push(manager.clone());
    command_args.push(operation.clone());
    command_args.extend(packages.iter().cloned());
    let command_preview = command_args.join(" ");
    let commands = vec![command_preview.clone()];
    let requires_sudo = arguments.allow_sudo.unwrap_or(false);
    let mut data = json!({
        "status": "planned",
        "commands": commands,
        "requires_sudo": requires_sudo,
        "log": format!("planned package install: {command_preview}"),
        "next_approval_required": true,
        "manager": manager,
        "packages": packages,
        "operation": operation,
        "dry_run": true,
        "args": command_args,
        "command_preview": command_preview,
    });

    if let Some(idempotency_key) = idempotency_key {
        data["idempotency_key"] = json!(idempotency_key);
    }

    Ok(data)
}

fn package_install_packages(
    package: Option<String>,
    packages: Option<Vec<String>>,
) -> Result<Vec<String>, RunnerToolError> {
    let mut normalized = Vec::new();
    if let Some(package) = package {
        normalized.push(package);
    }
    if let Some(packages) = packages {
        normalized.extend(packages);
    }

    let packages: Vec<String> = normalized
        .into_iter()
        .map(|package| package.trim().to_string())
        .filter(|package| !package.is_empty())
        .collect();

    if packages.is_empty() {
        return Err(RunnerToolError {
            code: "invalid_arguments",
            message: "package install arguments must include package or packages".to_string(),
        });
    }

    Ok(packages)
}

fn required_non_empty(value: Option<&str>, field: &'static str) -> Result<String, RunnerToolError> {
    optional_non_empty(value, field)?.ok_or_else(|| RunnerToolError {
        code: "invalid_arguments",
        message: format!("package install arguments must include {field}"),
    })
}

fn required_browser_argument(
    value: Option<&str>,
    field: &'static str,
) -> Result<String, RunnerToolError> {
    optional_browser_argument(value, field)?.ok_or_else(|| RunnerToolError {
        code: "invalid_arguments",
        message: format!("browser arguments must include {field}"),
    })
}

fn optional_browser_argument(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, RunnerToolError> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty() {
                Err(RunnerToolError {
                    code: "invalid_arguments",
                    message: format!("browser argument {field} must not be empty"),
                })
            } else {
                Ok(value.to_string())
            }
        })
        .transpose()
}

fn optional_non_empty(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, RunnerToolError> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty() {
                Err(RunnerToolError {
                    code: "invalid_arguments",
                    message: format!("package install argument {field} must not be empty"),
                })
            } else {
                Ok(value.to_string())
            }
        })
        .transpose()
}

fn create_backup(
    workspace: &Path,
    relative_path: &Path,
    target: &Path,
    display_path: &str,
) -> Result<String, RunnerToolError> {
    let file_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RunnerToolError {
            code: "invalid_arguments",
            message: "file write path must name a file".to_string(),
        })?;
    let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let backup_relative = parent.join(format!("{file_name}.{timestamp}.bak"));
    let backup_target = workspace.join(&backup_relative);

    std::fs::copy(target, &backup_target).map_err(|error| RunnerToolError {
        code: "write_failed",
        message: format!("failed to back up {display_path}: {error}"),
    })?;

    Ok(backup_relative.to_string_lossy().into_owned())
}

fn validate_workspace_relative_path(path: &str) -> Result<PathBuf, RunnerToolError> {
    let relative_path = Path::new(path);
    if path.is_empty() || relative_path.is_absolute() {
        return Err(RunnerToolError {
            code: "path_escape",
            message: "file path must be relative to the runner workspace".to_string(),
        });
    }

    let normalized = normalize_relative_path(relative_path)?;
    if normalized.as_os_str().is_empty() {
        return Err(RunnerToolError {
            code: "path_escape",
            message: "file path must be relative to the runner workspace".to_string(),
        });
    }

    Ok(normalized)
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, RunnerToolError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => normalized.push(name),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(RunnerToolError {
                        code: "path_escape",
                        message: "file path escapes the runner workspace".to_string(),
                    });
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(RunnerToolError {
                    code: "path_escape",
                    message: "file path must be relative to the runner workspace".to_string(),
                });
            }
        }
    }

    Ok(normalized)
}

fn select_lines(content: &str, start_line: usize, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }

    content
        .split_inclusive('\n')
        .skip(start_line.saturating_sub(1))
        .take(max_lines)
        .collect()
}

fn runner_result_value(result: RunnerToolResult) -> Value {
    serde_json::to_value(result).expect("runner tool result must be JSON serializable")
}
