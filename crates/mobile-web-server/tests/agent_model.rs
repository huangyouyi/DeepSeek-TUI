use std::sync::Arc;

use deepseek_mobile_web_server::model_config::{
    MobileModelConfig, MobileModelConfigSource, MobileModelMode,
};
use deepseek_mobile_web_server::{
    agent_model::{
        AgentContextMessage, AgentHttpRequest, AgentHttpTransport, AgentModel, AgentModelError,
        AgentModelRequest, AgentToolCall, DeepSeekAgentModel, MockAgentModel,
    },
    diagnostics::preset_command,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn mock_request(message: &str) -> AgentModelRequest {
    AgentModelRequest {
        session_id: "session-1".to_string(),
        message: message.to_string(),
        context: Vec::new(),
    }
}

fn mock_request_with_context(
    message: &str,
    context: Vec<AgentContextMessage>,
) -> AgentModelRequest {
    AgentModelRequest {
        session_id: "session-1".to_string(),
        message: message.to_string(),
        context,
    }
}

#[test]
fn mock_agent_model_can_be_used_as_arc_trait_object_and_maps_system_prompts_to_uname() {
    let model: Arc<dyn AgentModel> = Arc::new(MockAgentModel::new());

    let response = model
        .complete(&mock_request("检查 Linux 系统版本"))
        .expect("mock model should complete");

    assert_eq!(response.assistant_text, "I'll check the remote system.");
    assert_eq!(
        response.tool_calls,
        vec![AgentToolCall {
            tool: "remote.shell.exec".to_string(),
            command: "uname -a".to_string(),
        }]
    );
}

#[test]
fn mock_agent_model_maps_common_linux_questions_to_shell_commands() {
    let model = MockAgentModel::new();

    assert_eq!(
        model
            .complete(&mock_request("check disk space"))
            .unwrap()
            .tool_calls[0]
            .command,
        "df -h"
    );
    assert_eq!(
        model
            .complete(&mock_request("who am i on this host?"))
            .unwrap()
            .tool_calls[0]
            .command,
        "id"
    );
    assert_eq!(
        model
            .complete(&mock_request("当前用户和身份是什么？"))
            .unwrap()
            .tool_calls[0]
            .command,
        "id"
    );

    let unknown = model
        .complete(&mock_request("tell me a short joke"))
        .expect("mock model should answer unknown prompt");
    assert!(unknown.tool_calls.is_empty());
    assert!(!unknown.assistant_text.is_empty());
}

#[test]
fn mock_agent_model_routes_common_diagnostics_to_exact_preset_commands() {
    let model = MockAgentModel::new();

    for (message, preset) in [
        ("当前系统状态", "system_info"),
        ("show current system", "system_info"),
        ("检查网络", "network"),
        ("network connectivity", "network"),
        ("DNS 解析是否正常", "dns"),
        ("check dns resolution", "dns"),
        ("磁盘空间", "disk_usage"),
        ("disk usage", "disk_usage"),
        ("内存和 CPU 使用率", "cpu_memory"),
        ("cpu and memory usage", "cpu_memory"),
        ("服务是否运行", "services"),
        ("running services status", "services"),
        ("Docker 状态", "docker"),
        ("docker status", "docker"),
        ("OpenWrt 路由器基础网络", "openwrt_network"),
        ("router basic network", "openwrt_network"),
        ("日志摘要", "logs"),
        ("log summary", "logs"),
    ] {
        let response = model
            .complete(&mock_request(message))
            .expect("mock model should complete diagnostic prompt");

        assert_eq!(
            response.tool_calls,
            vec![AgentToolCall {
                tool: "remote.shell.exec".to_string(),
                command: preset_command(preset).unwrap().to_string(),
            }],
            "{message:?} should route to {preset}"
        );
    }
}

#[test]
fn mock_agent_model_can_continue_from_compact_context() {
    let model = MockAgentModel::new();

    let disk_response = model
        .complete(&mock_request_with_context(
            "继续",
            vec![AgentContextMessage {
                role: "user".to_string(),
                content: "check disk space".to_string(),
            }],
        ))
        .expect("mock model should use compact context");
    assert_eq!(disk_response.tool_calls[0].command, "df -h");

    let network_response = model
        .complete(&mock_request_with_context(
            "continue",
            vec![AgentContextMessage {
                role: "assistant".to_string(),
                content: "We were checking network connectivity.".to_string(),
            }],
        ))
        .expect("mock model should use compact context");
    assert_eq!(
        network_response.tool_calls[0].command,
        "ip addr || ifconfig"
    );
}

#[derive(Default)]
struct RecordingTransport {
    response: String,
    requests: std::sync::Mutex<Vec<AgentHttpRequest>>,
}

impl RecordingTransport {
    fn returning(response: &str) -> Self {
        Self {
            response: response.to_string(),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn last_request(&self) -> AgentHttpRequest {
        self.requests
            .lock()
            .unwrap()
            .last()
            .expect("transport should receive request")
            .clone()
    }
}

impl AgentHttpTransport for RecordingTransport {
    fn send(&self, request: AgentHttpRequest) -> Result<String, AgentModelError> {
        self.requests.lock().unwrap().push(request);
        Ok(self.response.clone())
    }
}

fn deepseek_config() -> MobileModelConfig {
    MobileModelConfig {
        provider: "deepseek".to_string(),
        base_url: "https://api.deepseek.com/".to_string(),
        model: "deepseek-v4-flash".to_string(),
        api_key: Some("real-secret-key".to_string()),
        source: MobileModelConfigSource::MissingDefaultPath("/tmp/missing".into()),
    }
}

#[test]
fn deepseek_agent_model_sends_openai_compatible_tool_request_and_parses_tool_call() {
    let transport = Arc::new(RecordingTransport::returning(
        r#"{
          "choices": [{
            "message": {
              "content": "I'll inspect the disk.",
              "tool_calls": [{
                "id": "call-1",
                "type": "function",
                "function": {
                  "name": "remote_shell_exec",
                  "arguments": "{\"command\":\"df -h\"}"
                }
              }]
            }
          }]
        }"#,
    ));
    let model = DeepSeekAgentModel::with_transport(deepseek_config(), transport.clone());

    let response = model
        .complete(&mock_request("disk status"))
        .expect("deepseek model should complete");

    assert_eq!(response.assistant_text, "I'll inspect the disk.");
    assert_eq!(
        response.tool_calls,
        vec![AgentToolCall {
            tool: "remote.shell.exec".to_string(),
            command: "df -h".to_string(),
        }]
    );

    let request = transport.last_request();
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://api.deepseek.com/chat/completions");
    assert_eq!(
        request.header("authorization"),
        Some("Bearer real-secret-key")
    );
    assert_eq!(request.body["model"], "deepseek-v4-flash");
    assert_eq!(request.body["messages"][0]["role"], "system");
    assert!(
        request.body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("remote Linux agent")
    );
    assert_eq!(request.body["messages"][1]["content"], "disk status");
    assert_eq!(
        request.body["tools"][0]["function"]["name"],
        "remote_shell_exec"
    );
    assert!(
        request.body["tools"][0]["function"]["name"]
            .as_str()
            .unwrap()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    );
}

#[test]
fn deepseek_agent_model_includes_compact_context_before_current_user_message() {
    let transport = Arc::new(RecordingTransport::returning(
        r#"{"choices":[{"message":{"content":"ok"}}]}"#,
    ));
    let model = DeepSeekAgentModel::with_transport(deepseek_config(), transport.clone());

    model
        .complete(&mock_request_with_context(
            "what now?",
            vec![
                AgentContextMessage {
                    role: "user".to_string(),
                    content: "check disk space".to_string(),
                },
                AgentContextMessage {
                    role: "assistant".to_string(),
                    content: "Disk looked nearly full.".to_string(),
                },
                AgentContextMessage {
                    role: "tool".to_string(),
                    content: "df -h output".to_string(),
                },
                AgentContextMessage {
                    role: "reasoning_content".to_string(),
                    content: "private thinking".to_string(),
                },
            ],
        ))
        .expect("deepseek model should complete");

    let request = transport.last_request();
    assert_eq!(
        request.body["messages"],
        json!([
            {
                "role": "system",
                "content": "You are a remote Linux agent. Use the remote_shell_exec tool for shell commands and otherwise answer concisely."
            },
            {
                "role": "user",
                "content": "check disk space"
            },
            {
                "role": "assistant",
                "content": "Disk looked nearly full."
            },
            {
                "role": "user",
                "content": "what now?"
            }
        ])
    );
}

#[test]
fn agent_http_request_debug_redacts_authorization_header() {
    let request = AgentHttpRequest {
        method: "POST".to_string(),
        url: "https://api.deepseek.com/chat/completions".to_string(),
        headers: vec![
            ("content-type".to_string(), "application/json".to_string()),
            (
                "authorization".to_string(),
                "Bearer real-secret-key".to_string(),
            ),
        ],
        body: json!({ "model": "deepseek-v4-flash" }),
    };

    let debug = format!("{request:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("real-secret-key"));
}

#[test]
fn deepseek_agent_model_redacted_status_and_debug_never_expose_api_key() {
    let transport = Arc::new(RecordingTransport::returning(
        r#"{"choices":[{"message":{"content":"ok"}}]}"#,
    ));
    let model = DeepSeekAgentModel::with_transport(deepseek_config(), transport);

    let status = serde_json::to_value(model.redacted_status()).expect("status must serialize");
    assert_eq!(
        status,
        json!({
            "model_mode": "deepseek",
            "model": "deepseek-v4-flash",
            "base_url": "https://api.deepseek.com/",
            "api_key_present": true
        })
    );

    let status_debug = format!("{:?}", model.redacted_status());
    let model_debug = format!("{model:?}");
    assert!(!status_debug.contains("real-secret-key"));
    assert!(!model_debug.contains("real-secret-key"));
}

#[test]
fn deepseek_agent_model_error_messages_redact_api_key() {
    struct FailingTransport;

    impl AgentHttpTransport for FailingTransport {
        fn send(&self, _request: AgentHttpRequest) -> Result<String, AgentModelError> {
            Err(AgentModelError::transport(
                "request failed with token real-secret-key",
            ))
        }
    }

    let model = DeepSeekAgentModel::with_transport(deepseek_config(), Arc::new(FailingTransport));
    let error = model.complete(&mock_request("hello")).unwrap_err();
    let display = error.to_string();
    let debug = format!("{error:?}");

    assert!(display.contains("<redacted>"));
    assert!(!display.contains("real-secret-key"));
    assert!(!debug.contains("real-secret-key"));
}

#[tokio::test]
async fn deepseek_agent_model_can_be_dropped_inside_tokio_runtime() {
    let model = DeepSeekAgentModel::new(deepseek_config());
    drop(model);
}

#[test]
fn mock_agent_model_status_is_browser_safe() {
    let model = MockAgentModel::new();

    assert_eq!(
        serde_json::to_value(model.redacted_status()).expect("status must serialize"),
        json!({
            "model_mode": "mock",
            "model": "mock",
            "base_url": "",
            "api_key_present": false
        })
    );
    assert_eq!(MobileModelMode::Mock, MobileModelMode::Mock);
}
