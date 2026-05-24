use deepseek_mobile_agent_core::remote_schema::{RemoteToolCall, RemoteToolName, RemoteToolOutput};
use deepseek_mobile_agent_core::transport::{
    ManualBootstrapTransportConfig, RemoteToolTransport, RunnerTransportConfig,
    RunnerWebSocketTransport, SshTransport, SshTransportConfig, TransportCapability,
    TransportError, TransportKind, TransportProfile,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn shell_call(call_id: &str) -> RemoteToolCall {
    RemoteToolCall {
        call_id: call_id.to_string(),
        name: RemoteToolName::ShellExec,
        arguments: json!({ "command": "echo hello" }),
    }
}

#[test]
fn transport_profiles_describe_manual_bootstrap_ssh_and_runner_modes() {
    let bootstrap = TransportProfile::manual_bootstrap(ManualBootstrapTransportConfig {
        instructions: "Install the runner and paste the pairing code.".to_string(),
    });
    let ssh = TransportProfile::ssh(SshTransportConfig {
        host: "linux-builder.local".to_string(),
        port: 2222,
        username: "mobile".to_string(),
    });
    let runner = TransportProfile::runner(RunnerTransportConfig {
        endpoint: "wss://runner.example/ws".to_string(),
        token_present: true,
    });

    assert_eq!(bootstrap.kind(), TransportKind::ManualBootstrap);
    assert_eq!(ssh.kind(), TransportKind::Ssh);
    assert_eq!(runner.kind(), TransportKind::RunnerWebSocket);

    assert_eq!(bootstrap.execution_mode(), "manual-bootstrap");
    assert_eq!(ssh.execution_mode(), "ssh");
    assert_eq!(runner.execution_mode(), "runner-websocket");

    assert_eq!(
        bootstrap.capabilities(),
        &[TransportCapability::BootstrapInstructions]
    );
    assert_eq!(
        ssh.capabilities(),
        &[
            TransportCapability::RemoteShell,
            TransportCapability::RemoteFileSystem
        ]
    );
    assert_eq!(
        runner.capabilities(),
        &[
            TransportCapability::RemoteShell,
            TransportCapability::RemoteFileSystem,
            TransportCapability::Mcp
        ]
    );
}

#[test]
fn runner_websocket_transport_holds_endpoint_and_token_state_without_connecting() {
    let config = RunnerTransportConfig {
        endpoint: "wss://runner.example/ws".to_string(),
        token_present: true,
    };
    let mut transport = RunnerWebSocketTransport::new(config.clone());

    assert_eq!(transport.endpoint(), config.endpoint);
    assert!(transport.token_present());

    let err = transport.execute(shell_call("call-runner")).unwrap_err();
    assert_eq!(
        err,
        TransportError::Failed("runner websocket transport is not connected".to_string())
    );
}

#[test]
fn runner_transport_config_from_pairing_keeps_only_redacted_token_state() {
    let pairing_token = "pair-secret-transport";

    let config =
        RunnerTransportConfig::from_pairing(" https://runner.example/mobile/ ", pairing_token);

    assert_eq!(config.endpoint, "https://runner.example/mobile");
    assert!(config.token_present);
    assert!(!format!("{config:?}").contains(pairing_token));
}

#[test]
fn ssh_transport_is_an_explicit_non_executing_scaffold() {
    let config = SshTransportConfig {
        host: "linux-builder.local".to_string(),
        port: 22,
        username: "mobile".to_string(),
    };
    let mut transport = SshTransport::new(config.clone());

    assert_eq!(transport.config(), &config);

    let err = transport.execute(shell_call("call-ssh")).unwrap_err();
    assert_eq!(
        err,
        TransportError::Failed("ssh transport is not connected".to_string())
    );
}

#[test]
fn scaffolds_do_not_fabricate_remote_outputs() {
    let mut runner = RunnerWebSocketTransport::new(RunnerTransportConfig {
        endpoint: "wss://runner.example/ws".to_string(),
        token_present: false,
    });
    let mut ssh = SshTransport::new(SshTransportConfig {
        host: "linux-builder.local".to_string(),
        port: 22,
        username: "mobile".to_string(),
    });

    let runner_result: Result<RemoteToolOutput, TransportError> =
        runner.execute(shell_call("call-runner"));
    let ssh_result: Result<RemoteToolOutput, TransportError> = ssh.execute(shell_call("call-ssh"));

    assert!(runner_result.is_err());
    assert!(ssh_result.is_err());
}
