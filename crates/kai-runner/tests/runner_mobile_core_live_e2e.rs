use deepseek_mobile_agent_core::capabilities::{CapabilitySet, ExecutionMode};
use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName};
use deepseek_mobile_agent_core::transport::{
    RunnerBearerToken, RunnerHttpBackend, RunnerHttpClient, RunnerHttpRequestAuth,
    RunnerHttpRequestSpec, RunnerHttpTransport, RunnerMaintenanceApproval,
    RunnerMaintenancePlanRequest, RunnerTcpHttpBackend,
};
use kai_runner::server::{build_router_with_shell_runner_and_token, build_router_with_token};
use kai_runner::{
    KaiRunner, ShellApprovalNonceManager, ShellExecutionRequest, ShellExecutionResult,
    ShellExecutor,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

const RUNNER_TOKEN: &str = "local-mobile-core-runner-token";
const MAINTENANCE_RUNNER_TOKEN: &str = "local-mobile-core-maintenance-token";

#[derive(Debug, Clone)]
struct NoopShellExecutor;

impl ShellExecutor for NoopShellExecutor {
    fn execute(&self, _request: ShellExecutionRequest) -> ShellExecutionResult {
        ShellExecutionResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
            timed_out: false,
        }
    }
}

struct LiveRunnerServer {
    addr: SocketAddr,
    task: JoinHandle<()>,
}

impl LiveRunnerServer {
    async fn start_with_token(token: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router_with_token(kai_runner::KaiRunner, token);
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        tokio::task::yield_now().await;

        Self { addr, task }
    }

    async fn start_shell_with_managed_approvals(token: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let runner = KaiRunner::with_shell_executor(NoopShellExecutor)
            .with_approval_nonce_manager(ShellApprovalNonceManager::new());
        let router = build_router_with_shell_runner_and_token(runner, token);
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        tokio::task::yield_now().await;

        Self { addr, task }
    }

    fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }
}

fn approval_nonce_request(endpoint: &str, token: &str) -> RunnerHttpRequestSpec {
    RunnerHttpRequestSpec {
        method: "POST".to_string(),
        url: format!("{endpoint}/approval/nonce"),
        token_present: true,
        auth: Some(RunnerHttpRequestAuth::Bearer {
            token: RunnerBearerToken::new(token),
        }),
        body: Value::Null,
    }
}

impl Drop for LiveRunnerServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn mobile_core_client_exercises_live_runner_socket_with_bearer_auth() {
    let server = LiveRunnerServer::start_with_token(RUNNER_TOKEN).await;
    let transport = RunnerHttpTransport::with_bearer_token(server.endpoint(), RUNNER_TOKEN);
    let mut client = RunnerHttpClient::new(
        transport.clone(),
        RunnerTcpHttpBackend::with_timeout(Duration::from_secs(5)),
    );

    let capabilities = client.discover_capabilities().unwrap();
    assert_eq!(capabilities, CapabilitySet::runner());
    assert_eq!(capabilities.mode, ExecutionMode::Runner);
    assert!(capabilities.allows(RemoteToolName::DiagnoseSystem));
    assert!(!format!("{transport:?}").contains(RUNNER_TOKEN));

    let output = client
        .execute_tool_call(RemoteToolCall {
            call_id: "mobile-core-live-diagnose".to_string(),
            name: RemoteToolName::DiagnoseSystem,
            arguments: json!({}),
        })
        .unwrap();

    assert_eq!(output.call_id, "mobile-core-live-diagnose");
    assert_eq!(output.success, true);
    assert_eq!(output.result["tool"], json!("remote.diagnose.system"));
    assert_eq!(output.result["status"], json!("ok"));
    assert_eq!(output.result["system"]["os"], json!(std::env::consts::OS));
    assert_eq!(
        output.result["system"]["arch"],
        json!(std::env::consts::ARCH)
    );
    assert_eq!(output.result["capabilities"]["mode"], json!("runner"));

    let audit = client.fetch_recent_audit().unwrap();
    assert_eq!(audit["type"], json!("audit_recent"));
    assert_eq!(audit["mode"], json!("runner"));
    assert_eq!(audit["persistence"], json!("memory_scaffold"));
    assert_eq!(audit["audit"][0]["event"], json!("capabilities.snapshot"));
    assert_eq!(audit["audit"][0]["status"], json!("available"));
    assert_eq!(audit["audit"][1]["event"], json!("tool_call"));
    assert_eq!(audit["audit"][1]["status"], json!("ok"));
    assert_eq!(
        audit["audit"][1]["metadata"]["call_id"],
        json!("mobile-core-live-diagnose")
    );
    assert_eq!(
        audit["audit"][1]["metadata"]["tool"],
        json!("remote.diagnose.system")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_runner_socket_rejects_mobile_core_client_without_bearer_auth() {
    let server = LiveRunnerServer::start_with_token(RUNNER_TOKEN).await;
    let transport = RunnerHttpTransport::new(
        deepseek_mobile_agent_core::transport::RunnerTransportConfig {
            endpoint: server.endpoint(),
            token_present: false,
        },
    );
    let request = transport.prepare_capabilities_request().unwrap();
    let mut backend = RunnerTcpHttpBackend::with_timeout(Duration::from_secs(5));

    let response = backend.execute(request).unwrap();

    assert_eq!(response.status, 401);
    assert_eq!(response.body["error"]["code"], json!("unauthorized"));
    assert!(!format!("{transport:?}").contains(RUNNER_TOKEN));
}

#[tokio::test(flavor = "multi_thread")]
async fn mobile_core_client_consumes_maintenance_approval_nonce_and_reads_redacted_audit() {
    let server =
        LiveRunnerServer::start_shell_with_managed_approvals(MAINTENANCE_RUNNER_TOKEN).await;
    let endpoint = server.endpoint();
    let backend = RunnerTcpHttpBackend::with_timeout(Duration::from_secs(5));
    let transport =
        RunnerHttpTransport::with_bearer_token(endpoint.clone(), MAINTENANCE_RUNNER_TOKEN);
    let mut client = RunnerHttpClient::new(transport.clone(), backend);

    let mut nonce_backend = RunnerTcpHttpBackend::with_timeout(Duration::from_secs(5));
    let nonce_response = nonce_backend
        .execute(approval_nonce_request(&endpoint, MAINTENANCE_RUNNER_TOKEN))
        .unwrap()
        .into_success_json()
        .unwrap();
    let nonce = nonce_response["approval_nonce"]
        .as_str()
        .expect("runner should issue an approval nonce");

    let plan = client
        .plan_maintenance_request(
            RunnerMaintenancePlanRequest::new("self_update").with_approval(
                RunnerMaintenanceApproval::new()
                    .with_nonce(nonce)
                    .with_metadata(json!({
                        "approval_id": "mobile-maintenance-e2e",
                        "approved_by": "ios-device",
                        "local_secret_marker": "audit-only-secret-must-not-leak"
                    })),
            ),
        )
        .unwrap();

    assert_eq!(plan["action"], json!("self_update"));
    assert_eq!(plan["dry_run"], json!(true));
    assert_eq!(plan["requires_approval"], json!(true));
    assert_eq!(plan["approval"]["status"], json!("consumed"));
    assert_eq!(
        plan["audit"][0]["event"],
        json!("maintenance.approval_nonce")
    );
    assert_eq!(plan["audit"][0]["status"], json!("consumed"));
    assert_eq!(plan["audit"][0]["nonce"]["label"], json!("approval_nonce"));
    assert_eq!(plan["audit"][0]["nonce"]["status"], json!("consumed"));

    let replay = client
        .plan_maintenance_request(
            RunnerMaintenancePlanRequest::new("self_update")
                .with_approval(RunnerMaintenanceApproval::new().with_nonce(nonce)),
        )
        .unwrap_err();
    assert_eq!(
        replay.to_string(),
        "remote tool failed: runner http backend returned unsuccessful status 400"
    );

    let recent = client.fetch_recent_audit().unwrap();
    let audit = recent["audit"]
        .as_array()
        .expect("recent audit should include an audit array");
    let audit_text = recent["audit"].to_string();

    assert_eq!(recent["type"], json!("audit_recent"));
    assert!(audit.iter().any(|event| {
        event["event"] == json!("maintenance.approval_nonce")
            && event["status"] == json!("consumed")
            && event["nonce"]["label"] == json!("approval_nonce")
            && event["nonce"]["status"] == json!("consumed")
    }));
    assert!(audit.iter().any(|event| {
        event["event"] == json!("maintenance.approval_nonce")
            && event["status"] == json!("replay")
            && event["nonce"]["status"] == json!("replayed")
    }));
    assert!(audit.iter().any(|event| {
        event["event"] == json!("maintenance.execute")
            && event["status"] == json!("no_op")
            && event["reason"] == json!("dry_run")
    }));
    assert!(!audit_text.contains(nonce));
    assert!(!audit_text.contains(MAINTENANCE_RUNNER_TOKEN));
    assert!(!audit_text.contains("Bearer"));
    assert!(!audit_text.contains("audit-only-secret-must-not-leak"));
    assert!(!format!("{transport:?}").contains(MAINTENANCE_RUNNER_TOKEN));
}
