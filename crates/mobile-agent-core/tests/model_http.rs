use deepseek_mobile_agent_core::RemoteToolName;
use deepseek_mobile_agent_core::model::{CloudModelClient, CloudModelConfig, FakeHttpTransport};
use pretty_assertions::assert_eq;

fn cloud_client() -> CloudModelClient {
    CloudModelClient::new(CloudModelConfig {
        provider: "deepseek".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-v4-flash".to_string(),
        api_key_present: true,
    })
}

#[test]
fn cloud_model_sends_openai_chat_completion_request_through_transport() {
    let mut transport = FakeHttpTransport::new(vec![
        r#"{
        "choices": [
            {
                "message": {
                    "content": "Use the shell tool."
                }
            }
        ]
    }"#,
    ]);
    let client = cloud_client();

    let response = client.complete_with_tools(
        &mut transport,
        "diagnose this laptop",
        &[RemoteToolName::ShellExec],
    );

    assert_eq!(response.text, "Use the shell tool.");
    assert!(response.tool_calls.is_empty());

    let request = transport
        .requests()
        .first()
        .expect("transport should receive one request");
    assert_eq!(
        request.url, "https://api.deepseek.com/chat/completions",
        "cloud model must target OpenAI-compatible chat completions"
    );
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.header("authorization"),
        Some("Bearer <api-key-present>")
    );
    assert!(
        !serde_json::to_string(request)
            .unwrap()
            .contains("real-secret-key"),
        "request spec must not leak a real API key"
    );
    assert_eq!(request.body["model"], "deepseek-v4-flash");
    assert_eq!(
        request.body["messages"][0]["content"],
        "diagnose this laptop"
    );
    assert_eq!(
        request.body["tools"][0]["function"]["name"],
        "remote.shell.exec"
    );
}

#[test]
fn cloud_model_omits_authorization_when_api_key_is_absent() {
    let mut transport = FakeHttpTransport::new(vec![
        r#"{
        "choices": [
            {
                "message": {
                    "content": "No token was sent."
                }
            }
        ]
    }"#,
    ]);
    let client = CloudModelClient::new(CloudModelConfig {
        provider: "deepseek".to_string(),
        base_url: "https://api.deepseek.com/".to_string(),
        model: "deepseek-v4-pro".to_string(),
        api_key_present: false,
    });

    let response = client.complete_with_tools(&mut transport, "hello", &[]);

    assert_eq!(response.text, "No token was sent.");
    let request = transport
        .requests()
        .first()
        .expect("transport should receive one request");
    assert_eq!(request.url, "https://api.deepseek.com/chat/completions");
    assert_eq!(request.header("authorization"), None);
}

#[test]
fn cloud_model_parses_openai_tool_calls() {
    let mut transport = FakeHttpTransport::new(vec![
        r#"{
        "choices": [
            {
                "message": {
                    "content": "",
                    "tool_calls": [
                        {
                            "id": "call-1",
                            "type": "function",
                            "function": {
                                "name": "remote.shell.exec",
                                "arguments": "{\"command\":\"pwd\"}"
                            }
                        }
                    ]
                }
            }
        ]
    }"#,
    ]);
    let client = cloud_client();

    let response =
        client.complete_with_tools(&mut transport, "run pwd", &[RemoteToolName::ShellExec]);

    assert_eq!(response.text, "");
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].call_id, "call-1");
    assert_eq!(response.tool_calls[0].name, RemoteToolName::ShellExec);
    assert_eq!(
        response.tool_calls[0].arguments,
        serde_json::json!({ "command": "pwd" })
    );
}
