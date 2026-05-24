use deepseek_mobile_agent_core::model::{
    ModelStreamEvent, aggregate_model_stream_chunks, parse_model_stream_chunks,
};
use pretty_assertions::assert_eq;

#[test]
fn parses_openai_compatible_content_deltas_across_chunks() {
    let events = parse_model_stream_chunks(&[
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n",
        "\ndata: [DONE]\n\n",
    ])
    .expect("valid SSE chunks should parse");

    assert_eq!(
        events,
        vec![
            ModelStreamEvent::ContentDelta("Hel".to_string()),
            ModelStreamEvent::ContentDelta("lo".to_string()),
            ModelStreamEvent::Done,
        ]
    );
}

#[test]
fn aggregates_streamed_content_into_model_response() {
    let response = aggregate_model_stream_chunks(&[
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\" from stream\"}}]}\n\n",
        "data: [DONE]\n\n",
    ])
    .expect("valid SSE chunks should aggregate");

    assert_eq!(response.text, "Hello from stream");
    assert!(response.tool_calls.is_empty());
}

#[test]
fn malformed_stream_chunk_returns_diagnostic_error() {
    let error = parse_model_stream_chunks(&["data: {not json}\n\n"])
        .expect_err("malformed stream data should fail");

    let diagnostic = error.to_string();
    assert!(
        diagnostic.contains("malformed SSE data"),
        "error should identify malformed SSE data, got: {diagnostic}"
    );
    assert!(
        diagnostic.contains("{not json}"),
        "error should include the bad payload, got: {diagnostic}"
    );
}
