use crate::remote_schema::{RemoteToolCall, RemoteToolName};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ModelResponse {
    pub text: String,
    pub tool_calls: Vec<RemoteToolCall>,
}

pub trait ModelClient {
    fn complete(&mut self, input: &str) -> ModelResponse;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelStreamEvent {
    ContentDelta(String),
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStreamError {
    message: String,
}

impl ModelStreamError {
    fn malformed_data(payload: &str, source: serde_json::Error) -> Self {
        Self {
            message: format!("malformed SSE data `{payload}`: {source}"),
        }
    }

    fn unterminated_event(buffer: &str) -> Self {
        Self {
            message: format!("unterminated SSE event `{buffer}`"),
        }
    }
}

impl fmt::Display for ModelStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ModelStreamError {}

pub fn parse_model_stream_chunks(
    chunks: &[&str],
) -> Result<Vec<ModelStreamEvent>, ModelStreamError> {
    let mut parser = ModelStreamParser::default();
    let mut events = Vec::new();

    for chunk in chunks {
        events.extend(parser.push_chunk(chunk)?);
    }

    parser.finish()?;
    Ok(events)
}

pub fn aggregate_model_stream_chunks(chunks: &[&str]) -> Result<ModelResponse, ModelStreamError> {
    let mut text = String::new();

    for event in parse_model_stream_chunks(chunks)? {
        match event {
            ModelStreamEvent::ContentDelta(delta) => text.push_str(&delta),
            ModelStreamEvent::Done => break,
        }
    }

    Ok(ModelResponse {
        text,
        tool_calls: Vec::new(),
    })
}

#[derive(Debug, Default)]
pub struct ModelStreamParser {
    buffer: String,
}

impl ModelStreamParser {
    pub fn push_chunk(&mut self, chunk: &str) -> Result<Vec<ModelStreamEvent>, ModelStreamError> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();

        while let Some(separator_start) = self.buffer.find("\n\n") {
            let raw_event = self.buffer[..separator_start].to_string();
            self.buffer.drain(..separator_start + 2);
            events.extend(parse_sse_event(&raw_event)?);
        }

        Ok(events)
    }

    pub fn finish(&self) -> Result<(), ModelStreamError> {
        if self.buffer.trim().is_empty() {
            Ok(())
        } else {
            Err(ModelStreamError::unterminated_event(&self.buffer))
        }
    }
}

#[derive(Debug, Default)]
pub struct FakeModelClient {
    responses: VecDeque<ModelResponse>,
    prompts: Vec<String>,
}

impl FakeModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: responses.into(),
            prompts: Vec::new(),
        }
    }

    pub fn prompts(&self) -> &[String] {
        &self.prompts
    }
}

impl ModelClient for FakeModelClient {
    fn complete(&mut self, input: &str) -> ModelResponse {
        self.prompts.push(input.to_string());
        self.responses.pop_front().unwrap_or_else(|| ModelResponse {
            text: format!("FakeModelClient exhausted responses for prompt: {input}"),
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudModelConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HttpRequestSpec {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

impl HttpRequestSpec {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub trait HttpTransport {
    fn send(&mut self, request: HttpRequestSpec) -> Result<String, HttpTransportError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpTransportError {
    message: String,
}

impl HttpTransportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for HttpTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for HttpTransportError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSendError {
    message: String,
}

impl ModelSendError {
    fn chat_completion_failed(source: HttpTransportError) -> Self {
        Self {
            message: format!("chat completion request failed: {source}"),
        }
    }
}

impl fmt::Display for ModelSendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ModelSendError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FakeHttpOutcome {
    Response(String),
    Error(HttpTransportError),
}

#[derive(Debug, Default)]
pub struct FakeHttpTransport {
    outcomes: VecDeque<FakeHttpOutcome>,
    requests: Vec<HttpRequestSpec>,
}

impl FakeHttpTransport {
    pub fn new(responses: Vec<&str>) -> Self {
        Self {
            outcomes: responses
                .into_iter()
                .map(|response| FakeHttpOutcome::Response(response.to_string()))
                .collect(),
            requests: Vec::new(),
        }
    }

    pub fn push_error(&mut self, message: impl Into<String>) {
        self.outcomes
            .push_back(FakeHttpOutcome::Error(HttpTransportError::new(message)));
    }

    pub fn requests(&self) -> &[HttpRequestSpec] {
        &self.requests
    }

    pub fn last_request(&self) -> Option<&HttpRequestSpec> {
        self.requests.last()
    }
}

impl HttpTransport for FakeHttpTransport {
    fn send(&mut self, request: HttpRequestSpec) -> Result<String, HttpTransportError> {
        self.requests.push(request);
        match self.outcomes.pop_front().unwrap_or_else(|| {
            FakeHttpOutcome::Response(r#"{"choices":[{"message":{"content":""}}]}"#.to_string())
        }) {
            FakeHttpOutcome::Response(response) => Ok(response),
            FakeHttpOutcome::Error(error) => Err(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    pub tools: Vec<ChatCompletionTool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatCompletionMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatCompletionTool {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ChatCompletionToolFunction,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatCompletionToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudModelClient {
    config: CloudModelConfig,
}

impl CloudModelClient {
    pub fn new(config: CloudModelConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &CloudModelConfig {
        &self.config
    }

    pub fn build_request(&self, prompt: &str, tools: &[RemoteToolName]) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            tools: tools.iter().copied().map(chat_completion_tool).collect(),
        }
    }

    pub fn complete_with_tools(
        &self,
        transport: &mut impl HttpTransport,
        prompt: &str,
        tools: &[RemoteToolName],
    ) -> ModelResponse {
        self.send(transport, prompt, tools)
            .unwrap_or_else(|error| ModelResponse {
                text: error.to_string(),
                tool_calls: Vec::new(),
            })
    }

    pub fn send(
        &self,
        transport: &mut impl HttpTransport,
        prompt: &str,
        tools: &[RemoteToolName],
    ) -> Result<ModelResponse, ModelSendError> {
        let request = self.build_http_request(prompt, tools);
        let response_body = transport
            .send(request)
            .map_err(ModelSendError::chat_completion_failed)?;
        Ok(parse_chat_completion_response(&response_body))
    }

    pub fn build_http_request(&self, prompt: &str, tools: &[RemoteToolName]) -> HttpRequestSpec {
        let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
        if self.config.api_key_present {
            headers.push((
                "authorization".to_string(),
                "Bearer <api-key-present>".to_string(),
            ));
        }

        HttpRequestSpec {
            method: "POST".to_string(),
            url: format!(
                "{}/chat/completions",
                self.config.base_url.trim_end_matches('/')
            ),
            headers,
            body: serde_json::to_value(self.build_request(prompt, tools))
                .expect("chat completion request should serialize"),
        }
    }
}

fn chat_completion_tool(name: RemoteToolName) -> ChatCompletionTool {
    ChatCompletionTool {
        kind: "function".to_string(),
        function: ChatCompletionToolFunction {
            name: name.as_str().to_string(),
            description: format!("Remote mobile-agent tool: {}", name.as_str()),
            parameters: json!({
                "type": "object",
                "additionalProperties": true
            }),
        },
    }
}

fn parse_chat_completion_response(body: &str) -> ModelResponse {
    let response = serde_json::from_str::<OpenAiChatCompletionResponse>(body).ok();
    let message = response
        .and_then(|response| response.choices.into_iter().next())
        .map(|choice| choice.message);

    ModelResponse {
        text: message
            .as_ref()
            .and_then(|message| message.content.clone())
            .unwrap_or_default(),
        tool_calls: message
            .map(|message| {
                message
                    .tool_calls
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(remote_tool_call_from_openai)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn parse_sse_event(raw_event: &str) -> Result<Vec<ModelStreamEvent>, ModelStreamError> {
    let data = raw_event
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            line.strip_prefix("data:")
                .map(|data| data.strip_prefix(' ').unwrap_or(data))
        })
        .collect::<Vec<_>>()
        .join("\n");

    if data.is_empty() {
        return Ok(Vec::new());
    }

    if data == "[DONE]" {
        return Ok(vec![ModelStreamEvent::Done]);
    }

    let payload = serde_json::from_str::<OpenAiChatCompletionStreamResponse>(&data)
        .map_err(|source| ModelStreamError::malformed_data(&data, source))?;
    Ok(payload
        .choices
        .into_iter()
        .filter_map(|choice| choice.delta.content)
        .map(ModelStreamEvent::ContentDelta)
        .collect())
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
struct OpenAiChatCompletionStreamResponse {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiToolFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolFunction {
    name: String,
    arguments: String,
}

fn remote_tool_call_from_openai(tool_call: OpenAiToolCall) -> Option<RemoteToolCall> {
    let name = RemoteToolName::parse(&tool_call.function.name).ok()?;
    let arguments = serde_json::from_str(&tool_call.function.arguments).unwrap_or(Value::Null);

    Some(RemoteToolCall {
        call_id: tool_call.id,
        name,
        arguments,
    })
}
