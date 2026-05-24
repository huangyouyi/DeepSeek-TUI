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
fn send_returns_assistant_text_from_chat_completion() {
    let mut transport = FakeHttpTransport::new(vec![
        r#"{
            "choices": [
                {
                    "message": {
                        "content": "Inspect battery health."
                    }
                }
            ]
        }"#,
    ]);
    let client = cloud_client();

    let response = client
        .send(&mut transport, "why is this phone draining?", &[])
        .expect("send should parse the model response");

    assert_eq!(response.text, "Inspect battery health.");
    assert!(response.tool_calls.is_empty());

    let request = transport
        .last_request()
        .expect("send should issue one HTTP request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://api.deepseek.com/chat/completions");
    assert_eq!(
        request.body["messages"][0]["content"],
        "why is this phone draining?"
    );
}

#[test]
fn send_converts_http_transport_error_into_diagnostic_error() {
    let mut transport = FakeHttpTransport::new(Vec::new());
    transport.push_error("connection refused");
    let client = cloud_client();

    let err = client
        .send(&mut transport, "hello", &[])
        .expect_err("HTTP transport failure should be returned to caller");

    assert!(
        err.to_string().contains("chat completion request failed"),
        "error should name the failed operation, got: {err}"
    );
    assert!(
        err.to_string().contains("connection refused"),
        "error should include transport detail, got: {err}"
    );
}

#[test]
fn send_request_does_not_leak_real_api_key() {
    let mut transport = FakeHttpTransport::new(vec![
        r#"{
            "choices": [
                {
                    "message": {
                        "content": "ok"
                    }
                }
            ]
        }"#,
    ]);
    let client = cloud_client();

    client
        .send(
            &mut transport,
            "use remote diagnostics",
            &[RemoteToolName::DiagnoseSystem],
        )
        .expect("send should succeed");

    let request = transport
        .last_request()
        .expect("send should issue one HTTP request");
    assert_eq!(
        request.header("authorization"),
        Some("Bearer <api-key-present>")
    );
    assert!(
        !serde_json::to_string(request)
            .unwrap()
            .contains("real-secret-key"),
        "request spec must never contain a real API key"
    );
    assert_eq!(
        request.body["tools"][0]["function"]["name"],
        "remote.diagnose.system"
    );
}
