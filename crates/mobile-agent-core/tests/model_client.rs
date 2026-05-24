use deepseek_mobile_agent_core::RemoteToolName;
use deepseek_mobile_agent_core::model::{CloudModelClient, CloudModelConfig};
use pretty_assertions::assert_eq;

#[test]
fn cloud_model_builds_openai_compatible_chat_completion_request() {
    let client = CloudModelClient::new(CloudModelConfig {
        provider: "deepseek".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-v4-flash".to_string(),
        api_key_present: true,
    });

    let request = client.build_request(
        "diagnose this laptop",
        &[RemoteToolName::ShellExec, RemoteToolName::BootstrapGuide],
    );
    let request_json = serde_json::to_value(request).expect("request should serialize");

    assert_eq!(request_json["model"], "deepseek-v4-flash");
    assert_eq!(
        request_json["messages"],
        serde_json::json!([
            {
                "role": "user",
                "content": "diagnose this laptop"
            }
        ])
    );
    assert_eq!(request_json["tools"][0]["type"], "function");
    assert_eq!(
        request_json["tools"][0]["function"]["name"],
        "remote.shell.exec"
    );
    assert_eq!(
        request_json["tools"][1]["function"]["name"],
        "remote.bootstrap.guide"
    );
}

#[test]
fn cloud_model_request_does_not_serialize_api_key_state() {
    let client = CloudModelClient::new(CloudModelConfig {
        provider: "deepseek".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-v4-pro".to_string(),
        api_key_present: true,
    });

    let request = client.build_request("hello", &[RemoteToolName::DiagnoseSystem]);
    let request_json = serde_json::to_value(request).expect("request should serialize");

    assert!(
        request_json.get("api_key").is_none(),
        "chat completion request must not expose an API key"
    );
    assert!(
        request_json.get("api_key_present").is_none(),
        "chat completion request must not expose API key presence"
    );
}
