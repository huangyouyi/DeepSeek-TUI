use deepseek_mobile_agent_core::MobileEventKind;
use deepseek_mobile_agent_core::event::ToolEventStreamBuilder;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn tool_event_stream_orders_started_stdout_stderr_completed_payloads() {
    let mut stream = ToolEventStreamBuilder::new("call-1", "shell_exec");

    let events = [
        stream.started(),
        stream.stdout("one\n"),
        stream.stderr("warning\n"),
        stream.completed(0, 125),
    ];

    assert_eq!(
        events
            .iter()
            .map(|event| (event.seq, event.kind.clone()))
            .collect::<Vec<_>>(),
        vec![
            (1, MobileEventKind::ToolCallStarted),
            (2, MobileEventKind::ToolStdout),
            (3, MobileEventKind::ToolStderr),
            (4, MobileEventKind::ToolCallCompleted),
        ]
    );
    assert_eq!(
        events[0].payload,
        json!({ "call_id": "call-1", "tool": "shell_exec" })
    );
    assert_eq!(
        events[1].payload,
        json!({ "call_id": "call-1", "tool": "shell_exec", "chunk": "one\n" })
    );
    assert_eq!(
        events[2].payload,
        json!({ "call_id": "call-1", "tool": "shell_exec", "chunk": "warning\n" })
    );
    assert_eq!(
        events[3].payload,
        json!({
            "call_id": "call-1",
            "tool": "shell_exec",
            "exit_code": 0,
            "duration_ms": 125,
        })
    );
}

#[test]
fn tool_event_stream_builds_timeout_payload() {
    let mut stream = ToolEventStreamBuilder::new("call-timeout", "shell_exec");

    let event = stream.timed_out(5000, 5012, "deadline exceeded");

    assert_eq!(event.seq, 1);
    assert_eq!(event.kind, MobileEventKind::ToolTimedOut);
    assert_eq!(
        event.payload,
        json!({
            "call_id": "call-timeout",
            "tool": "shell_exec",
            "timeout_ms": 5000,
            "duration_ms": 5012,
            "reason": "deadline exceeded",
        })
    );
}

#[test]
fn tool_event_stream_builds_cancelled_payload() {
    let mut stream = ToolEventStreamBuilder::new("call-cancelled", "shell_exec");

    let event = stream.cancelled(321, "user requested cancellation");

    assert_eq!(event.seq, 1);
    assert_eq!(event.kind, MobileEventKind::ToolCancelled);
    assert_eq!(
        event.payload,
        json!({
            "call_id": "call-cancelled",
            "tool": "shell_exec",
            "duration_ms": 321,
            "reason": "user requested cancellation",
        })
    );
}
