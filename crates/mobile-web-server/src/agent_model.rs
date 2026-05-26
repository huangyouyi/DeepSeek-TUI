use std::{fmt, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{diagnostics::preset_command, model_config::MobileModelConfig};

const SHELL_TOOL_NAME: &str = "remote.shell.exec";
const SHELL_TOOL_WIRE_NAME: &str = "remote_shell_exec";
const REDACTED: &str = "<redacted>";

pub trait AgentModel: Send + Sync {
    fn complete(&self, request: &AgentModelRequest) -> Result<AgentModelResponse, AgentModelError>;
    fn redacted_status(&self) -> AgentModelStatus;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentModelRequest {
    pub session_id: String,
    pub message: String,
    pub context: Vec<AgentContextMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentContextMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentModelResponse {
    pub assistant_text: String,
    pub tool_calls: Vec<AgentToolCall>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentToolCall {
    pub tool: String,
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AgentModelStatus {
    pub model_mode: String,
    pub model: String,
    pub base_url: String,
    pub api_key_present: bool,
}

#[derive(Clone, PartialEq)]
pub struct AgentHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

impl AgentHttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

impl fmt::Debug for AgentHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers = self
            .headers
            .iter()
            .map(|(name, value)| {
                if name.eq_ignore_ascii_case("authorization") {
                    (name.as_str(), REDACTED)
                } else {
                    (name.as_str(), value.as_str())
                }
            })
            .collect::<Vec<_>>();

        formatter
            .debug_struct("AgentHttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &headers)
            .field("body", &self.body)
            .finish()
    }
}

pub trait AgentHttpTransport: Send + Sync {
    fn send(&self, request: AgentHttpRequest) -> Result<String, AgentModelError>;
}

#[derive(Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct AgentModelError {
    message: String,
}

impl AgentModelError {
    pub fn transport(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn parse(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn redact_secret(mut self, secret: Option<&str>) -> Self {
        if let Some(secret) = secret.and_then(non_empty) {
            self.message = self.message.replace(secret, REDACTED);
        }
        self
    }
}

impl fmt::Debug for AgentModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentModelError")
            .field("message", &self.message)
            .finish()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MockAgentModel;

impl MockAgentModel {
    pub fn new() -> Self {
        Self
    }
}

impl AgentModel for MockAgentModel {
    fn complete(&self, request: &AgentModelRequest) -> Result<AgentModelResponse, AgentModelError> {
        let message = request.message.to_ascii_lowercase();
        let context = request
            .context
            .iter()
            .map(|message| message.content.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("\n");
        if message.contains("根据刚才远程命令结果") {
            return Ok(AgentModelResponse {
                assistant_text: mock_final_answer_from_context(&context),
                tool_calls: Vec::new(),
            });
        }
        let command = if is_continue_message(&message) {
            diagnostic_command_from_context(context.as_str())
        } else {
            command_from_direct_message(message.as_str())
        };

        Ok(match command {
            Some(command) => AgentModelResponse {
                assistant_text: "I'll check the remote system.".to_string(),
                tool_calls: vec![AgentToolCall {
                    tool: SHELL_TOOL_NAME.to_string(),
                    command: command.to_string(),
                }],
            },
            None => AgentModelResponse {
                assistant_text: "I can help inspect the remote Linux host.".to_string(),
                tool_calls: Vec::new(),
            },
        })
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "mock".to_string(),
            model: "mock".to_string(),
            base_url: String::new(),
            api_key_present: false,
        }
    }
}

pub struct DeepSeekAgentModel {
    config: MobileModelConfig,
    transport: Arc<dyn AgentHttpTransport>,
}

impl fmt::Debug for DeepSeekAgentModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepSeekAgentModel")
            .field("provider", &self.config.provider)
            .field("base_url", &self.config.base_url)
            .field("model", &self.config.model)
            .field("api_key_present", &api_key_present(&self.config))
            .finish_non_exhaustive()
    }
}

impl DeepSeekAgentModel {
    pub fn new(config: MobileModelConfig) -> Self {
        Self::with_transport(config, Arc::new(ReqwestAgentHttpTransport::new()))
    }

    pub fn with_transport(
        config: MobileModelConfig,
        transport: Arc<dyn AgentHttpTransport>,
    ) -> Self {
        Self { config, transport }
    }

    fn build_http_request(&self, request: &AgentModelRequest) -> AgentHttpRequest {
        let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
        if let Some(api_key) = self.config.api_key.as_deref().and_then(non_empty) {
            headers.push(("authorization".to_string(), format!("Bearer {api_key}")));
        }
        let mut messages = vec![json!({
            "role": "system",
            "content": "You are a remote Linux agent. Use the remote_shell_exec tool for shell commands and otherwise answer concisely."
        })];
        messages.extend(
            request
                .context
                .iter()
                .filter_map(agent_context_message_to_openai),
        );
        messages.push(json!({
            "role": "user",
            "content": request.message
        }));

        AgentHttpRequest {
            method: "POST".to_string(),
            url: format!(
                "{}/chat/completions",
                self.config.base_url.trim_end_matches('/')
            ),
            headers,
            body: json!({
                "model": self.config.model,
                "messages": messages,
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": SHELL_TOOL_WIRE_NAME,
                            "description": "Execute a shell command on the remote Linux host.",
                            "parameters": {
                                "type": "object",
                                "properties": {
                                    "command": {
                                        "type": "string",
                                        "description": "The shell command to execute."
                                    }
                                },
                                "required": ["command"],
                                "additionalProperties": false
                            }
                        }
                    }
                ]
            }),
        }
    }
}

impl AgentModel for DeepSeekAgentModel {
    fn complete(&self, request: &AgentModelRequest) -> Result<AgentModelResponse, AgentModelError> {
        let response_body = self
            .transport
            .send(self.build_http_request(request))
            .map_err(|error| error.redact_secret(self.config.api_key.as_deref()))?;
        parse_chat_completion_response(&response_body)
            .map_err(|error| error.redact_secret(self.config.api_key.as_deref()))
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "deepseek".to_string(),
            model: self.config.model.clone(),
            base_url: self.config.base_url.clone(),
            api_key_present: api_key_present(&self.config),
        }
    }
}

#[derive(Debug, Default)]
pub struct ReqwestAgentHttpTransport;

impl ReqwestAgentHttpTransport {
    pub fn new() -> Self {
        Self
    }
}

impl AgentHttpTransport for ReqwestAgentHttpTransport {
    fn send(&self, request: AgentHttpRequest) -> Result<String, AgentModelError> {
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::spawn(move || send_blocking_request(request))
                .join()
                .map_err(|_| AgentModelError::transport("request worker thread panicked"))?
        } else {
            send_blocking_request(request)
        }
    }
}

fn send_blocking_request(request: AgentHttpRequest) -> Result<String, AgentModelError> {
    let client = reqwest::blocking::Client::new();
    let mut builder = client.post(&request.url).json(&request.body);
    for (name, value) in request.headers {
        builder = builder.header(name, value);
    }
    let response = builder
        .send()
        .map_err(|source| AgentModelError::transport(format!("request failed: {source}")))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|source| AgentModelError::transport(format!("read response failed: {source}")))?;

    if !status.is_success() {
        return Err(AgentModelError::transport(format!(
            "chat completion failed with status {status}"
        )));
    }

    Ok(body)
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    function: OpenAiToolFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ShellExecArguments {
    command: String,
}

fn parse_chat_completion_response(body: &str) -> Result<AgentModelResponse, AgentModelError> {
    let response =
        serde_json::from_str::<OpenAiChatCompletionResponse>(body).map_err(|source| {
            AgentModelError::parse(format!("invalid chat completion JSON: {source}"))
        })?;
    let Some(message) = response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message)
    else {
        return Err(AgentModelError::parse(
            "chat completion response did not include a choice",
        ));
    };

    Ok(AgentModelResponse {
        assistant_text: message.content.unwrap_or_default(),
        tool_calls: message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .filter_map(agent_tool_call_from_openai)
            .collect(),
    })
}

fn agent_tool_call_from_openai(tool_call: OpenAiToolCall) -> Option<AgentToolCall> {
    if tool_call.function.name != SHELL_TOOL_WIRE_NAME && tool_call.function.name != SHELL_TOOL_NAME
    {
        return None;
    }
    let arguments =
        serde_json::from_str::<ShellExecArguments>(&tool_call.function.arguments).ok()?;
    Some(AgentToolCall {
        tool: SHELL_TOOL_NAME.to_string(),
        command: arguments.command,
    })
}

fn mock_final_answer_from_context(context: &str) -> String {
    if context.contains("df -h") && context.contains("100% /rom") {
        "结论：未看到明显满盘迹象。/rom 是只读系统镜像，显示 100% 通常是 OpenWrt 正常现象；请重点关注 /overlay、/ 和外接挂载点。".to_string()
    } else if context.contains("exit_code: 0") {
        "结论：远程命令已执行成功，未看到命令级失败。请根据工具结果中的输出继续判断业务状态。"
            .to_string()
    } else if context.contains("exit_code:") {
        "结论：远程命令已返回结果，但退出码非 0，结果可能不完整，需要根据缺失段落补充检查。"
            .to_string()
    } else {
        "结论：远程命令结果已返回，但缺少明确退出码，需要结合工具输出继续确认。".to_string()
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn command_from_direct_message(message: &str) -> Option<&'static str> {
    if contains_any(
        message,
        &["update", "install", "opkg", "apt", "更新", "安装"],
    ) {
        Some("opkg update")
    } else if contains_any(message, &["dns", "resolver", "resolve", "解析", "域名"]) {
        preset_command("dns")
    } else if contains_any(message, &["docker", "container", "containers", "容器"]) {
        preset_command("docker")
    } else if contains_any(message, &["openwrt", "router", "routing", "路由器", "路由"]) {
        preset_command("openwrt_network")
    } else if contains_any(
        message,
        &["log", "logs", "journal", "dmesg", "日志", "摘要"],
    ) {
        preset_command("logs")
    } else if contains_any(
        message,
        &[
            "system",
            "operating system",
            "kernel",
            "linux",
            "版本",
            "系统",
        ],
    ) {
        preset_command("system_info")
    } else if contains_any(
        message,
        &["service", "services", "daemon", "running", "服务", "运行"],
    ) {
        preset_command("services")
    } else if contains_any(
        message,
        &[
            "cpu",
            "memory",
            "mem",
            "load",
            "内存",
            "处理器",
            "负载",
            "使用率",
        ],
    ) {
        preset_command("cpu_memory")
    } else if contains_any(
        message,
        &[
            "network",
            "connectivity",
            "interface",
            "ip address",
            "网络",
            "联网",
        ],
    ) {
        preset_command("network")
    } else if contains_any(message, &["user", "identity", "who am i", "用户", "身份"]) {
        preset_command("current_user")
    } else if contains_any(message, &["disk", "space", "filesystem", "磁盘", "空间"]) {
        preset_command("disk_usage")
    } else {
        None
    }
}

fn diagnostic_command_from_context(context: &str) -> Option<&'static str> {
    if contains_any(
        context,
        &[
            "network",
            "connectivity",
            "interface",
            "ip address",
            "网络",
            "联网",
        ],
    ) {
        preset_command("network")
    } else if contains_any(context, &["disk", "space", "filesystem", "磁盘", "空间"]) {
        preset_command("disk_usage")
    } else if contains_any(
        context,
        &[
            "system",
            "operating system",
            "kernel",
            "linux",
            "版本",
            "系统",
        ],
    ) {
        preset_command("system_info")
    } else {
        None
    }
}

fn is_continue_message(message: &str) -> bool {
    contains_any(message, &["继续", "刚才", "continue"])
}

fn agent_context_message_to_openai(message: &AgentContextMessage) -> Option<Value> {
    let role = match message.role.as_str() {
        "system" | "user" | "assistant" => message.role.as_str(),
        _ => return None,
    };
    let content = message.content.trim();
    if content.is_empty() {
        return None;
    }
    Some(json!({
        "role": role,
        "content": content
    }))
}

fn api_key_present(config: &MobileModelConfig) -> bool {
    config.api_key.as_deref().and_then(non_empty).is_some()
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value) }
}
