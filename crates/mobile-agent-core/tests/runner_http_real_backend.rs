use deepseek_mobile_agent_core::transport::{
    RunnerHttpBackend, RunnerHttpClient, RunnerHttpClosureBackend, RunnerHttpRequestSpec,
    RunnerHttpResponseSpec, RunnerHttpTransport, RunnerTcpHttpBackend,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

fn serve_once(status: u16, body: &'static str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];

        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                let content_length = header_value(&request, "content-length")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let header_len = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|position| position + 4)
                    .unwrap();
                if request.len() >= header_len + content_length {
                    break;
                }
            }
        }

        tx.send(String::from_utf8(request).unwrap()).unwrap();
        write!(
            stream,
            "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });

    (url, rx)
}

fn header_value(request: &[u8], name: &str) -> Option<String> {
    let request = String::from_utf8_lossy(request);
    request.lines().find_map(|line| {
        let (header_name, value) = line.split_once(':')?;
        header_name
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

#[test]
fn real_http_backend_sends_get_with_bearer_and_parses_json_response() {
    let (endpoint, request_rx) = serve_once(200, r#"{"entries":[{"seq":1}]}"#);
    let transport = RunnerHttpTransport::with_bearer_token(endpoint, "runner-secret");
    let request = transport.prepare_recent_audit_request().unwrap();
    let mut backend = RunnerTcpHttpBackend::new();

    let response = backend.execute(request).unwrap();

    assert_eq!(
        response,
        RunnerHttpResponseSpec::ok_json(json!({ "entries": [{ "seq": 1 }] }))
    );
    let raw_request = request_rx.recv().unwrap();
    assert!(raw_request.starts_with("GET /audit/recent HTTP/1.1"));
    assert_eq!(
        header_value(raw_request.as_bytes(), "authorization"),
        Some("Bearer runner-secret".to_string())
    );
    assert!(!format!("{transport:?}").contains("runner-secret"));
}

#[test]
fn real_http_backend_sends_post_json_and_preserves_error_status_body() {
    let (endpoint, request_rx) = serve_once(418, r#"{"error":"teapot"}"#);
    let transport = RunnerHttpTransport::with_bearer_token(endpoint, "runner-secret");
    let request = transport
        .prepare_tool_call_request(&deepseek_mobile_agent_core::remote_schema::RemoteToolCall {
            call_id: "call-1".to_string(),
            name: deepseek_mobile_agent_core::remote_schema::RemoteToolName::ShellExec,
            arguments: json!({ "command": "pwd" }),
        })
        .unwrap();
    let mut backend = RunnerTcpHttpBackend::new();

    let response = backend.execute(request).unwrap();

    assert_eq!(
        response,
        RunnerHttpResponseSpec::json(418, json!({ "error": "teapot" }))
    );
    let raw_request = request_rx.recv().unwrap();
    assert!(raw_request.starts_with("POST /tool-call HTTP/1.1"));
    assert_eq!(
        header_value(raw_request.as_bytes(), "authorization"),
        Some("Bearer runner-secret".to_string())
    );
    assert_eq!(
        header_value(raw_request.as_bytes(), "content-type"),
        Some("application/json".to_string())
    );
    assert!(raw_request.contains(r#""call_id":"call-1""#));
    assert!(raw_request.contains(r#""command":"pwd""#));
}

#[test]
fn http_client_fetches_recent_audit_through_request_helper() {
    let transport = RunnerHttpTransport::with_bearer_token("http://runner.local/api/", "secret");
    let backend = RunnerHttpClosureBackend::new(|request: RunnerHttpRequestSpec| {
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "http://runner.local/api/audit/recent");
        assert_eq!(request.body, Value::Null);
        assert_eq!(
            request.authorization_header_value(),
            Some("Bearer secret".to_string())
        );
        Ok(RunnerHttpResponseSpec::ok_json(json!({
            "entries": [{ "seq": 7, "summary": "ok" }]
        })))
    });
    let mut client = RunnerHttpClient::new(transport, backend);

    let audit = client.fetch_recent_audit().unwrap();

    assert_eq!(audit, json!({ "entries": [{ "seq": 7, "summary": "ok" }] }));
}
