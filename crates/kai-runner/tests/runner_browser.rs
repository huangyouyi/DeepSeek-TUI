use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName};
use kai_runner::{
    BrowserClickApproval, BrowserClickRequest, BrowserEngine, BrowserExtractTextRequest,
    BrowserExtractTextResult, BrowserOpenRequest, BrowserOpenResult, BrowserSessionMetadata,
    BrowserSessionRegistry, KaiRunner, RunnerToolError,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn call(name: RemoteToolName, arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: format!("call-{name}"),
        name,
        arguments,
    }
}

#[derive(Debug, Default)]
struct FakeBrowserEngine {
    open_requests: Mutex<Vec<BrowserOpenRequest>>,
    requests: Mutex<Vec<BrowserExtractTextRequest>>,
    click_requests: Mutex<Vec<BrowserClickRequest>>,
}

impl BrowserEngine for FakeBrowserEngine {
    fn open(&self, request: BrowserOpenRequest) -> Result<BrowserOpenResult, RunnerToolError> {
        self.open_requests.lock().unwrap().push(request.clone());
        Ok(BrowserOpenResult::planned(
            request.url,
            request.profile,
            "fake browser engine opened an in-memory page",
        ))
    }

    fn extract_text(
        &self,
        request: BrowserExtractTextRequest,
    ) -> Result<BrowserExtractTextResult, RunnerToolError> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(BrowserExtractTextResult {
            text: "Documentation body text".to_string(),
        })
    }

    fn click(&self, request: BrowserClickRequest) -> Result<BrowserClickApproval, RunnerToolError> {
        self.click_requests.lock().unwrap().push(request.clone());
        Ok(BrowserClickApproval::approval_required(request))
    }
}

#[test]
fn browser_session_registry_creates_lists_and_closes_sessions() {
    let registry = BrowserSessionRegistry::new();

    let first = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/docs".to_string()),
        profile: Some("work".to_string()),
    });
    let second = registry.create_session(BrowserSessionMetadata {
        url: None,
        profile: Some("personal".to_string()),
    });

    assert_eq!(first.id, "browser-session-1");
    assert_eq!(second.id, "browser-session-2");
    assert_eq!(
        registry.list_sessions(),
        vec![first.clone(), second.clone()]
    );

    assert_eq!(registry.close_session(&first.id), Some(first));
    assert_eq!(registry.list_sessions(), vec![second.clone()]);
    assert_eq!(registry.close_session("browser-session-missing"), None);
    assert_eq!(registry.close_session(&second.id), Some(second));
    assert_eq!(registry.list_sessions(), vec![]);
}

#[test]
fn browser_open_returns_planned_scaffold_response() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::BrowserOpen,
        json!({
            "url": "https://example.com/docs",
            "profile": "work"
        }),
    ));

    assert!(output.success);
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.browser.open",
            "status": "ok",
            "data": {
                "status": "planned",
                "url": "https://example.com/docs",
                "profile": "work",
                "events": [
                    {
                        "type": "planned",
                        "message": "browser.open accepted but no browser was launched"
                    }
                ],
                "next_approval_required": false
            }
        })
    );
}

#[test]
fn browser_open_allocates_session_in_registry_enabled_runner() {
    let registry = BrowserSessionRegistry::new();
    let runner = KaiRunner::with_browser_registry(registry.clone());

    let output = runner.execute(call(
        RemoteToolName::BrowserOpen,
        json!({
            "url": "https://example.com/docs",
            "profile": "work"
        }),
    ));

    assert!(output.success);
    assert_eq!(
        output.result["data"]["session_id"],
        json!("browser-session-1")
    );
    assert_eq!(
        registry.list_sessions(),
        vec![kai_runner::BrowserSession {
            id: "browser-session-1".to_string(),
            url: Some("https://example.com/docs".to_string()),
            profile: Some("work".to_string()),
        }]
    );
}

#[test]
fn browser_open_rejects_missing_or_empty_url() {
    let runner = KaiRunner;

    for arguments in [json!({}), json!({ "url": "" }), json!({ "url": "   " })] {
        let output = runner.execute(call(RemoteToolName::BrowserOpen, arguments));

        assert!(!output.success);
        assert_eq!(output.result["tool"], json!("remote.browser.open"));
        assert_eq!(output.result["status"], json!("error"));
        assert_eq!(output.result["error"]["code"], json!("invalid_arguments"));
    }
}

#[test]
fn browser_extract_text_reports_browser_unavailable() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::BrowserExtractText,
        json!({
            "url": "https://example.com/docs",
            "selector": "main"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.extract_text"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("browser_unavailable"));
    assert!(
        output.result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no browser engine yet")
    );
}

#[test]
fn browser_enabled_extract_text_with_known_session_reports_engine_missing_with_session_id() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/docs".to_string()),
        profile: Some("work".to_string()),
    });
    let runner = KaiRunner::with_browser_registry(registry);

    let output = runner.execute(call(
        RemoteToolName::BrowserExtractText,
        json!({
            "session_id": session.id,
            "selector": "main"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.extract_text"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(
        output.result["data"]["session_id"],
        json!("browser-session-1")
    );
    assert_ne!(
        output.result["error"]["code"],
        json!("unknown_browser_session")
    );
    assert!(matches!(
        output.result["error"]["code"].as_str(),
        Some("browser_unavailable" | "engine_missing")
    ));
}

#[test]
fn browser_enabled_extract_text_with_fake_engine_returns_text_and_metadata() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/docs".to_string()),
        profile: Some("work".to_string()),
    });
    let engine = Arc::new(FakeBrowserEngine::default());
    let runner = KaiRunner::with_browser_engine(registry, engine.clone());

    let output = runner.execute(call(
        RemoteToolName::BrowserExtractText,
        json!({
            "session_id": session.id,
            "selector": "main"
        }),
    ));

    assert!(output.success);
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.browser.extract_text",
            "status": "ok",
            "data": {
                "session_id": "browser-session-1",
                "text": "Documentation body text",
                "selector": "main",
                "url": "https://example.com/docs"
            }
        })
    );
    assert_eq!(
        *engine.requests.lock().unwrap(),
        vec![BrowserExtractTextRequest {
            session_id: "browser-session-1".to_string(),
            url: Some("https://example.com/docs".to_string()),
            selector: Some("main".to_string()),
            handle: None,
        }]
    );
}

#[test]
fn browser_enabled_open_uses_injected_engine_before_registering_session() {
    let registry = BrowserSessionRegistry::new();
    let engine = Arc::new(FakeBrowserEngine::default());
    let runner = KaiRunner::with_browser_engine(registry.clone(), engine.clone());

    let output = runner.execute(call(
        RemoteToolName::BrowserOpen,
        json!({
            "url": "https://example.com/docs",
            "profile": "work"
        }),
    ));

    assert!(output.success);
    assert_eq!(output.result["data"]["status"], json!("planned"));
    assert_eq!(
        output.result["data"]["events"][0]["message"],
        json!("fake browser engine opened an in-memory page")
    );
    assert_eq!(
        output.result["data"]["session_id"],
        json!("browser-session-1")
    );
    assert_eq!(
        *engine.open_requests.lock().unwrap(),
        vec![BrowserOpenRequest {
            url: "https://example.com/docs".to_string(),
            profile: Some("work".to_string()),
        }]
    );
    assert_eq!(registry.list_sessions()[0].id, "browser-session-1");
}

#[test]
fn browser_enabled_click_uses_injected_engine_but_stays_approval_bound() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/docs".to_string()),
        profile: Some("work".to_string()),
    });
    let engine = Arc::new(FakeBrowserEngine::default());
    let runner = KaiRunner::with_browser_engine(registry, engine.clone());

    let output = runner.execute(call(
        RemoteToolName::BrowserClick,
        json!({
            "session_id": session.id,
            "selector": "button.next",
            "label": "Next",
            "action": "click",
            "metadata": {
                "source": "test"
            }
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.click"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
    assert_eq!(output.result["data"]["risk"], json!("state_changing"));
    assert_eq!(
        *engine.click_requests.lock().unwrap(),
        vec![BrowserClickRequest {
            session_id: Some("browser-session-1".to_string()),
            selector: Some("button.next".to_string()),
            label: Some("Next".to_string()),
            action: "click".to_string(),
            metadata: Some(json!({ "source": "test" })),
            idempotency_key: None,
            approval_nonce: None,
        }]
    );
}

#[test]
fn browser_enabled_install_page_smoke_extracts_text_and_requires_approval_for_download_click() {
    let registry = BrowserSessionRegistry::new();
    let engine = Arc::new(FakeBrowserEngine::default());
    let runner = KaiRunner::with_browser_engine(registry, engine.clone());

    let open = runner.execute(call(
        RemoteToolName::BrowserOpen,
        json!({
            "url": "https://example.com/downloads/pro-editor",
            "profile": "installer"
        }),
    ));

    assert!(open.success);
    assert_eq!(open.result["data"]["status"], json!("planned"));
    assert_eq!(
        open.result["data"]["session_id"],
        json!("browser-session-1")
    );
    assert_eq!(open.result["data"]["next_approval_required"], json!(false));

    let extract = runner.execute(call(
        RemoteToolName::BrowserExtractText,
        json!({
            "session_id": "browser-session-1",
            "selector": "main"
        }),
    ));

    assert!(extract.success);
    assert_eq!(
        extract.result["data"]["text"],
        json!("Documentation body text")
    );
    assert_eq!(
        extract.result["data"]["url"],
        json!("https://example.com/downloads/pro-editor")
    );
    assert_eq!(
        *engine.requests.lock().unwrap(),
        vec![BrowserExtractTextRequest {
            session_id: "browser-session-1".to_string(),
            url: Some("https://example.com/downloads/pro-editor".to_string()),
            selector: Some("main".to_string()),
            handle: None,
        }]
    );

    let click = runner.execute(call(
        RemoteToolName::BrowserClick,
        json!({
            "session_id": "browser-session-1",
            "selector": "a[data-action=download]",
            "label": "Download for macOS",
            "action": "download",
            "metadata": {
                "scenario": "install-pro-editor",
                "source": "browser-assisted-install-smoke"
            }
        }),
    ));

    assert!(!click.success);
    assert_eq!(click.result["tool"], json!("remote.browser.click"));
    assert_eq!(click.result["status"], json!("error"));
    assert_eq!(click.result["error"]["code"], json!("approval_required"));
    assert_eq!(
        click.result["data"],
        json!({
            "session_id": "browser-session-1",
            "selector": "a[data-action=download]",
            "label": "Download for macOS",
            "action": "download",
            "approval_required": true,
            "risk": "state_changing",
            "metadata": {
                "scenario": "install-pro-editor",
                "source": "browser-assisted-install-smoke"
            },
            "audit": [
                {
                    "type": "browser_action_blocked",
                    "tool": "remote.browser.click",
                    "action": "download",
                    "session_id": "browser-session-1",
                    "selector": "a[data-action=download]",
                    "reason": "approval_required"
                }
            ]
        })
    );
}

#[test]
fn browser_enabled_extract_text_rejects_unknown_session() {
    let runner = KaiRunner::with_browser_registry(BrowserSessionRegistry::new());

    let output = runner.execute(call(
        RemoteToolName::BrowserExtractText,
        json!({
            "session_id": "browser-session-missing",
            "selector": "main"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.extract_text"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(
        output.result["error"]["code"],
        json!("unknown_browser_session")
    );
}

#[test]
fn browser_enabled_extract_text_rejects_missing_session_id() {
    let runner = KaiRunner::with_browser_registry(BrowserSessionRegistry::new());

    let output = runner.execute(call(
        RemoteToolName::BrowserExtractText,
        json!({
            "selector": "main"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.extract_text"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("invalid_arguments"));
}

#[test]
fn browser_click_requires_approval_without_action() {
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::BrowserClick,
        json!({
            "handle": "page-1",
            "selector": "button[type=submit]"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.click"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
    let message = output.result["error"]["message"].as_str().unwrap();
    assert!(message.contains("click"));
    assert!(message.contains("submit"));
}

#[test]
fn browser_enabled_click_with_known_session_requires_approval_with_session_id() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/docs".to_string()),
        profile: Some("work".to_string()),
    });
    let runner = KaiRunner::with_browser_registry(registry);

    let output = runner.execute(call(
        RemoteToolName::BrowserClick,
        json!({
            "session_id": session.id,
            "selector": "button[type=submit]"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.click"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(
        output.result["data"]["session_id"],
        json!("browser-session-1")
    );
    assert_eq!(output.result["error"]["code"], json!("approval_required"));
}

#[test]
fn browser_enabled_click_requires_approval_for_sensitive_browser_actions() {
    let registry = BrowserSessionRegistry::new();
    let session = registry.create_session(BrowserSessionMetadata {
        url: Some("https://example.com/account".to_string()),
        profile: Some("work".to_string()),
    });
    let runner = KaiRunner::with_browser_registry(registry);

    for action in ["download", "login", "submit", "upload", "payment"] {
        let output = runner.execute(call(
            RemoteToolName::BrowserClick,
            json!({
                "session_id": session.id,
                "selector": format!("[data-action={action}]"),
                "action": action
            }),
        ));

        assert!(!output.success, "{action} should require approval");
        assert_eq!(output.result["tool"], json!("remote.browser.click"));
        assert_eq!(output.result["status"], json!("error"));
        assert_eq!(output.result["error"]["code"], json!("approval_required"));
        assert_eq!(output.result["data"]["approval_required"], json!(true));
        assert_eq!(output.result["data"]["risk"], json!("state_changing"));
        assert_eq!(output.result["data"]["action"], json!(action));
        assert_eq!(
            output.result["data"]["audit"][0]["type"],
            json!("browser_action_blocked")
        );
        assert_eq!(
            output.result["data"]["audit"][0]["reason"],
            json!("approval_required")
        );
    }
}

#[test]
fn browser_enabled_click_rejects_unknown_session_before_approval() {
    let runner = KaiRunner::with_browser_registry(BrowserSessionRegistry::new());

    let output = runner.execute(call(
        RemoteToolName::BrowserClick,
        json!({
            "session_id": "browser-session-missing",
            "selector": "button[type=submit]"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.click"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(
        output.result["error"]["code"],
        json!("unknown_browser_session")
    );
}

#[test]
fn browser_enabled_click_rejects_missing_session_id_before_approval() {
    let runner = KaiRunner::with_browser_registry(BrowserSessionRegistry::new());

    let output = runner.execute(call(
        RemoteToolName::BrowserClick,
        json!({
            "selector": "button[type=submit]"
        }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.browser.click"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("invalid_arguments"));
}
