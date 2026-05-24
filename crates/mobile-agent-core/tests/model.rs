use deepseek_mobile_agent_core::{
    FakeModelClient, ModelClient, ModelResponse, RemoteToolCall, RemoteToolName,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn response(text: &str) -> ModelResponse {
    ModelResponse {
        text: text.to_string(),
        tool_calls: Vec::new(),
    }
}

#[test]
fn fake_model_returns_preloaded_responses_in_order_and_records_prompts() {
    let mut model = FakeModelClient::new(vec![response("first"), response("second")]);

    assert_eq!(model.complete("prompt one"), response("first"));
    assert_eq!(model.complete("prompt two"), response("second"));
    assert_eq!(
        model.prompts(),
        &["prompt one".to_string(), "prompt two".to_string()]
    );
}

#[test]
fn fake_model_returns_diagnostic_fallback_when_responses_are_exhausted() {
    let mut model = FakeModelClient::new(vec![response("only")]);

    assert_eq!(model.complete("first"), response("only"));
    let fallback = model.complete("unexpected prompt");

    assert!(fallback.tool_calls.is_empty());
    assert!(
        fallback.text.contains("FakeModelClient exhausted"),
        "fallback should diagnose missing fixture response, got: {}",
        fallback.text
    );
    assert!(
        fallback.text.contains("unexpected prompt"),
        "fallback should include the prompt that could not be answered, got: {}",
        fallback.text
    );
}

#[test]
fn model_response_can_carry_remote_tool_calls() {
    let tool_call = RemoteToolCall {
        call_id: "call-1".to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": "pwd" }),
    };
    let expected = ModelResponse {
        text: "run a command".to_string(),
        tool_calls: vec![tool_call],
    };
    let mut model = FakeModelClient::new(vec![expected.clone()]);

    assert_eq!(model.complete("use a tool"), expected);
    assert_eq!(model.prompts(), &["use a tool".to_string()]);
}
