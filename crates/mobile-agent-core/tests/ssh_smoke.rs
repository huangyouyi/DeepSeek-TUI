use deepseek_mobile_agent_core::ssh::{
    SshCommandRequest, SshConnectionConfig, SshInvocationSpec, SshRunner, SshRunnerOutput,
    SshToolTransport,
};
use deepseek_mobile_agent_core::{
    RemoteToolCall, RemoteToolName, RemoteToolTransport, TransportError,
};
use serde_json::json;
use std::process::{Command, Stdio};
use std::time::Instant;

// Manual smoke test for a real SSH target.
//
// This test is ignored so normal test runs never attempt network access:
//   cargo test -p deepseek-mobile-agent-core --test ssh_smoke
//
// To run it intentionally against a configured host:
//   DEEPSEEK_MOBILE_SSH_HOST=builder.local \
//   DEEPSEEK_MOBILE_SSH_USER=mobile \
//   DEEPSEEK_MOBILE_SSH_PORT=22 \
//   cargo test -p deepseek-mobile-agent-core --test ssh_smoke -- --ignored --nocapture
//
// The command is read-only and the test does not accept or print secret material.
#[test]
#[ignore = "requires DEEPSEEK_MOBILE_SSH_HOST and DEEPSEEK_MOBILE_SSH_USER"]
fn ssh_tool_transport_can_run_read_only_command_against_configured_host() {
    let Some(config) = smoke_config() else {
        eprintln!(
            "skipping real SSH smoke test; set DEEPSEEK_MOBILE_SSH_HOST and \
             DEEPSEEK_MOBILE_SSH_USER to run it"
        );
        return;
    };
    let mut transport = SshToolTransport::new(config.clone(), SystemSshRunner);

    let output = transport
        .execute(RemoteToolCall {
            call_id: "ssh-smoke".to_string(),
            name: RemoteToolName::ShellExec,
            arguments: json!({
                "command": "printf deepseek-mobile-ssh-smoke",
                "timeout_ms": 10_000
            }),
        })
        .expect("configured SSH host should execute the read-only smoke command");

    assert!(
        output.success,
        "SSH smoke command failed: {}",
        output.result
    );
    assert_eq!(output.call_id, "ssh-smoke");
    assert_eq!(output.result["runner"]["transport"], "ssh");
    assert_eq!(output.result["risk"]["level"], "low");
    assert_eq!(output.result["stdout"], "deepseek-mobile-ssh-smoke");
    assert_eq!(transport.config(), &config);
}

#[derive(Debug, Clone, Copy)]
struct SystemSshRunner;

impl SshRunner for SystemSshRunner {
    fn run(
        &mut self,
        spec: &SshInvocationSpec,
        command: &SshCommandRequest,
    ) -> Result<SshRunnerOutput, TransportError> {
        assert_eq!(spec.program, "ssh");
        assert_eq!(spec.args.first().map(String::as_str), Some("-p"));
        assert!(spec.args.len() >= 4, "unexpected SSH invocation: {spec:?}");
        assert_eq!(command.command, "printf deepseek-mobile-ssh-smoke");

        let started = Instant::now();
        let output = Command::new(&spec.program)
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
            .args(&spec.args)
            .stdin(Stdio::null())
            .output()
            .map_err(|err| TransportError::Failed(format!("failed to invoke ssh: {err}")))?;

        Ok(SshRunnerOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code(),
            duration: started.elapsed(),
            timed_out: false,
        })
    }
}

fn smoke_config() -> Option<SshConnectionConfig> {
    let host = env_nonempty("DEEPSEEK_MOBILE_SSH_HOST")?;
    let user = env_nonempty("DEEPSEEK_MOBILE_SSH_USER")?;
    let port = env_nonempty("DEEPSEEK_MOBILE_SSH_PORT")
        .as_deref()
        .map(str::parse)
        .transpose()
        .expect("DEEPSEEK_MOBILE_SSH_PORT must be a valid u16")
        .unwrap_or(22);

    Some(SshConnectionConfig {
        host,
        user,
        port,
        token_present: false,
        key_present: false,
    })
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
