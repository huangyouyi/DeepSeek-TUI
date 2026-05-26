use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use deepseek_mobile_agent_core::ssh::SshCommandRequest;
use deepseek_mobile_web_server::{
    AppState, MobileWebServerConfig, SshTarget,
    agent_model::{
        AgentModel, AgentModelError, AgentModelRequest, AgentModelResponse, AgentModelStatus,
        AgentToolCall,
    },
    app_router_with_runner,
    routes::app_router_with_config_runner_and_model,
    ssh_exec::{CommandRunError, CommandRunner, SshCommandOutput},
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Clone, Debug)]
struct FakeRunner {
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeRunner {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls mutex").clone()
    }
}

impl CommandRunner for FakeRunner {
    fn run(
        &self,
        _target: &SshTarget,
        command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(command.command.clone());
        Ok(SshCommandOutput {
            stdout: format!("ran {}\n", command.command),
            stderr: String::new(),
            exit_code: Some(0),
            duration: Duration::from_millis(12),
            timed_out: false,
        })
    }
}

#[derive(Debug)]
struct HighRiskCommandModel;

impl AgentModel for HighRiskCommandModel {
    fn complete(&self, request: &AgentModelRequest) -> Result<AgentModelResponse, AgentModelError> {
        let command = if request.message.contains("restart") {
            "systemctl restart ssh"
        } else {
            "opkg update"
        };
        Ok(AgentModelResponse {
            assistant_text: "I'll run the requested command.".to_string(),
            tool_calls: vec![AgentToolCall {
                tool: "remote.shell.exec".to_string(),
                command: command.to_string(),
            }],
        })
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "test".to_string(),
            model: "test".to_string(),
            base_url: "test".to_string(),
            api_key_present: false,
        }
    }
}

#[derive(Debug)]
struct ApprovalFinalAnswerModel {
    requests: Arc<Mutex<Vec<AgentModelRequest>>>,
}

#[derive(Debug)]
struct LowRiskFinalAnswerModel {
    requests: Arc<Mutex<Vec<AgentModelRequest>>>,
}

impl LowRiskFinalAnswerModel {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<AgentModelRequest> {
        self.requests.lock().expect("requests mutex").clone()
    }
}

impl AgentModel for LowRiskFinalAnswerModel {
    fn complete(&self, request: &AgentModelRequest) -> Result<AgentModelResponse, AgentModelError> {
        let mut requests = self.requests.lock().expect("requests mutex");
        requests.push(request.clone());
        let request_count = requests.len();
        drop(requests);

        Ok(if request_count == 1 {
            AgentModelResponse {
                assistant_text: "I'll inspect the system.".to_string(),
                tool_calls: vec![AgentToolCall {
                    tool: "remote.shell.exec".to_string(),
                    command: "uname -a".to_string(),
                }],
            }
        } else {
            AgentModelResponse {
                assistant_text: "最终结论：远程系统信息已读取，内核信息可用于后续判断。"
                    .to_string(),
                tool_calls: Vec::new(),
            }
        })
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "test".to_string(),
            model: "test".to_string(),
            base_url: "test".to_string(),
            api_key_present: false,
        }
    }
}

impl ApprovalFinalAnswerModel {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<AgentModelRequest> {
        self.requests.lock().expect("requests mutex").clone()
    }
}

impl AgentModel for ApprovalFinalAnswerModel {
    fn complete(&self, request: &AgentModelRequest) -> Result<AgentModelResponse, AgentModelError> {
        let mut requests = self.requests.lock().expect("requests mutex");
        requests.push(request.clone());
        let request_count = requests.len();
        drop(requests);

        Ok(if request_count == 1 {
            AgentModelResponse {
                assistant_text: "I'll run the requested command.".to_string(),
                tool_calls: vec![AgentToolCall {
                    tool: "remote.shell.exec".to_string(),
                    command: "opkg update".to_string(),
                }],
            }
        } else {
            AgentModelResponse {
                assistant_text:
                    "最终结论：软件源更新命令已执行成功，远程输出显示 opkg update 已完成。"
                        .to_string(),
                tool_calls: Vec::new(),
            }
        })
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "test".to_string(),
            model: "test".to_string(),
            base_url: "test".to_string(),
            api_key_present: false,
        }
    }
}

#[derive(Debug)]
struct MetaQuestionToolModel;

impl AgentModel for MetaQuestionToolModel {
    fn complete(
        &self,
        _request: &AgentModelRequest,
    ) -> Result<AgentModelResponse, AgentModelError> {
        Ok(AgentModelResponse {
            assistant_text: "I'll inspect disk usage.".to_string(),
            tool_calls: vec![AgentToolCall {
                tool: "remote.shell.exec".to_string(),
                command: "df -h".to_string(),
            }],
        })
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "test".to_string(),
            model: "test".to_string(),
            base_url: "test".to_string(),
            api_key_present: false,
        }
    }
}

#[derive(Debug)]
struct ContextRecordingModel {
    requests: Arc<Mutex<Vec<AgentModelRequest>>>,
}

impl ContextRecordingModel {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<AgentModelRequest> {
        self.requests.lock().expect("requests mutex").clone()
    }
}

impl AgentModel for ContextRecordingModel {
    fn complete(&self, request: &AgentModelRequest) -> Result<AgentModelResponse, AgentModelError> {
        let mut requests = self.requests.lock().expect("requests mutex");
        requests.push(request.clone());
        let is_first_turn = requests.len() == 1;
        drop(requests);

        Ok(if is_first_turn {
            AgentModelResponse {
                assistant_text: "I'll inspect network state.".to_string(),
                tool_calls: vec![AgentToolCall {
                    tool: "remote.shell.exec".to_string(),
                    command: "ip addr || ifconfig".to_string(),
                }],
            }
        } else {
            AgentModelResponse {
                assistant_text: "I can continue from the previous network result.".to_string(),
                tool_calls: Vec::new(),
            }
        })
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "test".to_string(),
            model: "test".to_string(),
            base_url: "test".to_string(),
            api_key_present: false,
        }
    }
}

#[derive(Clone, Debug)]
struct LongOutputRunner;

impl CommandRunner for LongOutputRunner {
    fn run(
        &self,
        _target: &SshTarget,
        _command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        Ok(SshCommandOutput {
            stdout: format!(
                "network head\n{}\nMIDDLE_SENTINEL_SHOULD_NOT_ENTER_CONTEXT\n{}\nnetwork tail\n",
                "alpha\n".repeat(160),
                "omega\n".repeat(160)
            ),
            stderr: String::new(),
            exit_code: Some(0),
            duration: Duration::from_millis(12),
            timed_out: false,
        })
    }
}

#[derive(Clone, Debug)]
struct OpenWrtDiskRunner;

impl CommandRunner for OpenWrtDiskRunner {
    fn run(
        &self,
        _target: &SshTarget,
        command: &SshCommandRequest,
    ) -> Result<SshCommandOutput, CommandRunError> {
        assert_eq!(command.command, "df -h");
        Ok(SshCommandOutput {
            stdout: r#"Filesystem                Size      Used Available Use% Mounted on
/dev/root               212.5M    212.5M         0 100% /rom
tmpfs                     1.9G    178.4M      1.7G   9% /tmp
/dev/sda3                 1.9G    705.5M      1.2G  37% /overlay
overlayfs:/overlay        1.9G    705.5M      1.2G  37% /
/dev/sdb1                62.7G      9.0G     50.5G  15% /mnt/vio3-1
"#
            .to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            duration: Duration::from_millis(12),
            timed_out: false,
        })
    }
}

#[derive(Debug)]
struct FailingModel;

impl AgentModel for FailingModel {
    fn complete(
        &self,
        _request: &AgentModelRequest,
    ) -> Result<AgentModelResponse, AgentModelError> {
        Err(AgentModelError::transport(
            "upstream model unavailable: token=secret-value",
        ))
    }

    fn redacted_status(&self) -> AgentModelStatus {
        AgentModelStatus {
            model_mode: "test".to_string(),
            model: "test".to_string(),
            base_url: "test".to_string(),
            api_key_present: false,
        }
    }
}

fn test_state() -> AppState {
    AppState::new(SshTarget {
        host: "127.0.0.1".to_string(),
        user: "tester".to_string(),
        port: 2222,
        key_present: false,
    })
}

async fn json_response(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body must be readable");
    serde_json::from_slice(&bytes).expect("response body must be json")
}

async fn create_session(app: axum::Router) -> String {
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"title": "Agent chat"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_response(response).await;
    body["id"].as_str().expect("session id").to_string()
}

fn tool_parts(messages: &[deepseek_mobile_web_server::types::Message]) -> Vec<&Value> {
    messages
        .iter()
        .flat_map(|message| &message.parts)
        .filter(|part| part.kind == "tool")
        .map(|part| &part.data)
        .collect()
}

#[tokio::test]
async fn agent_turn_routes_chinese_system_question_to_uname_not_raw_shell_text() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请问当前运行在什么系统？"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"][0]["command"], "uname -a");
    assert_eq!(body["executed_tools"][0]["requires_approval"], false);
    assert!(
        body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("结论：远程命令已执行成功")
    );
    assert_eq!(runner.calls(), vec!["uname -a"]);
    assert!(
        !runner
            .calls()
            .contains(&"请问当前运行在什么系统？".to_string())
    );

    let messages = state.messages(&session_id);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    let tools = tool_parts(&messages);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["tool"], "remote.shell.exec");
    assert_eq!(tools[0]["status"], "completed");
    assert_eq!(tools[0]["requires_approval"], false);
    assert_eq!(tools[0]["command"], "uname -a");
    assert_eq!(tools[0]["output"], "ran uname -a\n");
    assert_eq!(tools[0]["stdout"], "ran uname -a\n");
    assert_eq!(tools[0]["stderr"], "");
    assert_eq!(tools[0]["exit_code"], 0);
    assert_eq!(tools[0]["duration_ms"], 12);
    assert_eq!(tools[0]["timed_out"], false);
    assert_eq!(tools[0]["turn_id"].is_string(), true);
    assert_eq!(tools[0]["agent_turn_id"], tools[0]["turn_id"]);
}

#[tokio::test]
async fn agent_turn_low_risk_tool_result_returns_to_model_for_final_conclusion() {
    let state = test_state();
    let runner = FakeRunner::new();
    let model = Arc::new(LowRiskFinalAnswerModel::new());
    let app = app_router_with_config_runner_and_model(
        state.clone(),
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: None,
        },
        runner.clone(),
        model.clone(),
    );
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请问当前运行在什么系统？"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"][0]["command"], "uname -a");
    assert!(
        body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("最终结论：远程系统信息已读取")
    );
    assert_eq!(runner.calls(), vec!["uname -a"]);

    let requests = model.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].message, "请问当前运行在什么系统？");
    assert!(requests[1].message.contains("根据刚才远程命令结果"));
    let final_context = requests[1]
        .context
        .iter()
        .map(|item| item.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(final_context.contains("Tool remote.shell.exec `uname -a` status completed"));
    assert!(final_context.contains("stdout"));

    let messages = state.messages(&session_id);
    let last_text = messages
        .last()
        .and_then(|message| {
            message
                .parts
                .iter()
                .filter(|part| part.kind == "text")
                .filter_map(|part| part.text.as_deref())
                .next()
        })
        .unwrap_or("");
    assert!(last_text.contains("最终结论：远程系统信息已读取"));
}

#[tokio::test]
async fn agent_turn_routes_disk_question_to_df() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "请查看磁盘空间"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"][0]["command"], "df -h");
    assert_eq!(runner.calls(), vec!["df -h"]);
}

#[tokio::test]
async fn agent_turn_common_diagnostic_answers_with_conclusion_not_stdout_dump() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "DNS 是否正常？"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(
        body["executed_tools"][0]["command"],
        "getent hosts deepseek.com || nslookup deepseek.com || cat /etc/resolv.conf"
    );
    let assistant_text = body["assistant_text"].as_str().expect("assistant text");
    assert!(assistant_text.contains("结论："));
    assert!(assistant_text.contains("工具结果"));
    assert!(!assistant_text.contains("\nstdout:\n"));
    let messages = state.messages(&session_id);
    let tools = tool_parts(&messages);
    assert!(
        tools[0]["stdout"]
            .as_str()
            .unwrap_or("")
            .starts_with("ran getent hosts")
    );
}

#[tokio::test]
async fn agent_turn_text_only_answer_executes_no_ssh() {
    let runner = FakeRunner::new();
    let app = app_router_with_runner(test_state(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "你好"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"], json!([]));
    assert!(
        body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("remote Linux")
    );
    assert!(runner.calls().is_empty());
}

#[tokio::test]
async fn agent_turn_high_risk_tool_request_creates_pending_approval_without_execution() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "waiting_for_approval");
    assert_eq!(body["executed_tools"], json!([]));
    assert_eq!(body["pending_approvals"][0]["command"], "opkg update");
    assert_eq!(
        body["pending_approvals"][0]["agent_turn_id"].is_string(),
        true
    );
    assert!(runner.calls().is_empty());

    let messages = state.messages(&session_id);
    let tools = tool_parts(&messages);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["tool"], "remote.shell.exec");
    assert_eq!(tools[0]["status"], "pending_approval");
    assert_eq!(tools[0]["requires_approval"], true);
    assert_eq!(tools[0]["command"], "opkg update");
    assert_eq!(tools[0]["approval_id"], body["pending_approvals"][0]["id"]);
    assert_eq!(tools[0]["agent_turn_id"], body["turn_id"]);
    assert_eq!(tools[0]["turn_id"], body["turn_id"]);
}

#[tokio::test]
async fn agent_turn_high_risk_approval_returns_final_assistant_summary() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let turn_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(turn_response.status(), StatusCode::OK);
    let turn_body = json_response(turn_response).await;
    let approval_id = turn_body["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");
    let pending_tool_part_id = state
        .messages(&session_id)
        .into_iter()
        .flat_map(|message| message.parts)
        .find(|part| part.kind == "tool")
        .expect("pending tool part")
        .id;

    let approved = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "approve_once"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = json_response(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert!(
        approved_body["result"]["summary"]
            .as_str()
            .unwrap_or("")
            .contains("结论：远程命令已执行成功")
    );
    assert!(
        state
            .messages(&session_id)
            .last()
            .and_then(|message| message.parts[0].text.as_deref())
            .unwrap_or("")
            .contains("结论：远程命令已执行成功")
    );
    assert_eq!(runner.calls(), vec!["opkg update"]);
    let updated_tool_part = state
        .messages(&session_id)
        .into_iter()
        .flat_map(|message| message.parts)
        .find(|part| part.id == pending_tool_part_id)
        .expect("original pending tool part should still exist");
    assert_eq!(updated_tool_part.data["status"], "completed");
    assert_eq!(updated_tool_part.data["approval_id"], approval_id);
    assert_eq!(updated_tool_part.data["exit_code"], 0);
}

#[tokio::test]
async fn agent_turn_high_risk_approval_returns_model_final_conclusion_after_tool_result() {
    let state = test_state();
    let runner = FakeRunner::new();
    let model = Arc::new(ApprovalFinalAnswerModel::new());
    let app = app_router_with_config_runner_and_model(
        state.clone(),
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: None,
        },
        runner.clone(),
        model.clone(),
    );
    let session_id = create_session(app.clone()).await;

    let turn_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(turn_response.status(), StatusCode::OK);
    let turn_body = json_response(turn_response).await;
    let approval_id = turn_body["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");

    let approved = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"response": "approve_once"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = json_response(approved).await;
    assert_eq!(approved_body["status"], "approved");
    let assistant_text = approved_body["result"]["assistant_text"]
        .as_str()
        .expect("assistant text");
    assert!(assistant_text.contains("最终结论：软件源更新命令已执行成功"));
    assert!(!assistant_text.contains("Approved command"));
    assert_eq!(runner.calls(), vec!["opkg update"]);

    let requests = model.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].message, "请更新软件包索引");
    assert!(requests[1].message.contains("根据刚才远程命令结果"));
    let final_context = requests[1]
        .context
        .iter()
        .map(|item| item.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(final_context.contains("Tool remote.shell.exec `opkg update` status completed"));
    assert!(final_context.contains("stdout"));

    let messages = state.messages(&session_id);
    let last_text = messages
        .last()
        .and_then(|message| message.parts[0].text.as_deref())
        .unwrap_or("");
    assert!(last_text.contains("最终结论：软件源更新命令已执行成功"));
}

#[tokio::test]
async fn agent_turn_approve_session_allows_same_session_command_without_new_pending_approval() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let turn_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let turn_body = json_response(turn_response).await;
    let approval_id = turn_body["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");

    let approved = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"response": "approve_session"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(approved.status(), StatusCode::OK);

    let allowed_turn = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请再次更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(allowed_turn.status(), StatusCode::OK);
    let allowed_body = json_response(allowed_turn).await;
    assert_eq!(allowed_body["status"], "completed");
    assert_eq!(allowed_body["pending_approvals"], json!([]));
    assert_eq!(allowed_body["executed_tools"][0]["command"], "opkg update");
    assert_eq!(
        allowed_body["executed_tools"][0]["requires_approval"],
        false
    );
    assert_eq!(runner.calls(), vec!["opkg update", "opkg update"]);
    assert_eq!(state.pending_approvals(), Vec::new());
}

#[tokio::test]
async fn agent_turn_openwrt_readonly_rom_full_does_not_mean_writable_disk_pressure() {
    let app = app_router_with_runner(test_state(), false, OpenWrtDiskRunner);
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "检查磁盘空间"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    let assistant_text = body["assistant_text"].as_str().expect("assistant text");
    assert!(assistant_text.contains("未看到明显满盘迹象"));
    assert!(assistant_text.contains("/rom 是只读系统镜像"));
    assert!(!assistant_text.contains("Tool Activity"));
    assert!(!assistant_text.contains("磁盘空间需要关注"));
}

#[tokio::test]
async fn agent_turn_meta_question_does_not_execute_model_proposed_remote_tool() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_config_runner_and_model(
        state,
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: None,
        },
        runner.clone(),
        Arc::new(MetaQuestionToolModel),
    );
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "为何没有得到汇总结果？"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "completed");
    assert_eq!(body["executed_tools"], json!([]));
    assert!(runner.calls().is_empty());
    assert!(
        body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("没有得到汇总")
    );
}

#[tokio::test]
async fn agent_turn_session_allow_does_not_apply_to_other_session_or_other_command() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_config_runner_and_model(
        state.clone(),
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: None,
        },
        runner.clone(),
        Arc::new(HighRiskCommandModel),
    );
    let session_id = create_session(app.clone()).await;
    let other_session_id = create_session(app.clone()).await;

    let first_turn = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let first_body = json_response(first_turn).await;
    let approval_id = first_body["pending_approvals"][0]["id"]
        .as_str()
        .expect("approval id");
    let approved = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/approvals/{approval_id}/respond"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"response": "approve_session"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(approved.status(), StatusCode::OK);

    let other_session_turn = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{other_session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let other_session_body = json_response(other_session_turn).await;
    assert_eq!(other_session_body["status"], "waiting_for_approval");
    assert_eq!(
        other_session_body["pending_approvals"][0]["command"],
        "opkg update"
    );

    let other_command_turn = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "restart ssh"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let other_command_body = json_response(other_command_turn).await;
    assert_eq!(other_command_body["status"], "waiting_for_approval");
    assert_eq!(
        other_command_body["pending_approvals"][0]["command"],
        "systemctl restart ssh"
    );
    assert_eq!(runner.calls(), vec!["opkg update"]);
}

#[tokio::test]
async fn agent_turn_continue_uses_compact_previous_context_without_full_long_stdout() {
    let state = test_state();
    let model = Arc::new(ContextRecordingModel::new());
    let app = app_router_with_config_runner_and_model(
        state,
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: None,
        },
        LongOutputRunner,
        model.clone(),
    );
    let session_id = create_session(app.clone()).await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "检查网络状态"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "刚才那个网络问题继续帮我看", "mode": "continue"})
                        .to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(second.status(), StatusCode::OK);

    let requests = model.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].context, Vec::new());
    let context_text = requests[2]
        .context
        .iter()
        .map(|item| item.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(context_text.contains("检查网络状态"));
    assert!(context_text.contains("Tool remote.shell.exec `ip addr || ifconfig` status completed"));
    assert!(context_text.contains("stdout summarized"));
    assert!(!context_text.contains("MIDDLE_SENTINEL_SHOULD_NOT_ENTER_CONTEXT"));
}

#[tokio::test]
async fn agent_turn_model_failure_returns_deterministic_fallback_with_context() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_config_runner_and_model(
        state,
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: None,
        },
        runner,
        Arc::new(FailingModel),
    );
    let session_id = create_session(app.clone()).await;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "刚才那个网络问题继续帮我看"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    assert_eq!(body["status"], "model_fallback");
    assert!(
        body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("确定性总结")
    );
    assert!(
        !body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("secret-value")
    );
}

#[tokio::test]
async fn agent_turn_stop_removes_pending_approvals_for_turn_and_appends_summary() {
    let state = test_state();
    let runner = FakeRunner::new();
    let app = app_router_with_runner(state.clone(), false, runner.clone());
    let session_id = create_session(app.clone()).await;

    let turn = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "请更新软件包索引"}).to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let body = json_response(turn).await;
    let turn_id = body["turn_id"].as_str().expect("turn id");
    assert_eq!(state.pending_approvals().len(), 1);

    let stopped = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "/api/sessions/{session_id}/agent-turns/{turn_id}/stop"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("request must complete");

    assert_eq!(stopped.status(), StatusCode::OK);
    let stopped_body = json_response(stopped).await;
    assert_eq!(stopped_body["status"], "stopped");
    assert!(
        stopped_body["assistant_text"]
            .as_str()
            .unwrap_or("")
            .contains("已停止")
    );
    assert_eq!(state.pending_approvals(), Vec::new());
    assert!(runner.calls().is_empty());
}

#[tokio::test]
async fn agent_turn_retry_turn_id_replays_original_user_message() {
    let state = test_state();
    let model = Arc::new(ContextRecordingModel::new());
    let app = app_router_with_config_runner_and_model(
        state,
        MobileWebServerConfig {
            use_real_model: false,
            static_dir: None,
        },
        FakeRunner::new(),
        model.clone(),
    );
    let session_id = create_session(app.clone()).await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"message": "检查网络状态"}).to_string()))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    let first_body = json_response(first).await;
    let first_turn_id = first_body["turn_id"].as_str().expect("turn id");

    let retry = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/api/sessions/{session_id}/agent-turn"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"message": "", "mode": "retry", "retry_turn_id": first_turn_id})
                        .to_string(),
                ))
                .expect("request must build"),
        )
        .await
        .expect("request must complete");
    assert_eq!(retry.status(), StatusCode::OK);

    let requests = model.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].message, "检查网络状态");
}
