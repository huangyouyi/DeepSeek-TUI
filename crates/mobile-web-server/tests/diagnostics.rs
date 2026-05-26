use std::sync::{Arc, Mutex};
use std::time::Duration;

use deepseek_mobile_web_server::{
    AppState, DiagnosticRequest, SshTarget,
    diagnostics::{DiagnosticError, DiagnosticService, preset_command, preset_diagnostics},
    ssh_exec::{CommandRunner, SshCommandOutput},
};

#[derive(Clone, Debug)]
struct FakeRunner {
    calls: Arc<Mutex<Vec<String>>>,
    output: SshCommandOutput,
}

impl FakeRunner {
    fn new(output: SshCommandOutput) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            output,
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
        command: &deepseek_mobile_agent_core::ssh::SshCommandRequest,
    ) -> Result<SshCommandOutput, deepseek_mobile_web_server::ssh_exec::CommandRunError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(command.command.clone());
        Ok(self.output.clone())
    }
}

fn target() -> SshTarget {
    SshTarget {
        host: "127.0.0.1".to_string(),
        user: "tester".to_string(),
        port: 2222,
        key_present: false,
    }
}

#[test]
fn diagnostics_preset_mapping_is_exact() {
    assert_eq!(preset_command("system_info"), Some("uname -a"));
    assert_eq!(preset_command("current_user"), Some("id"));
    assert_eq!(preset_command("disk_usage"), Some("df -h"));
    assert_eq!(
        preset_command("memory"),
        Some("free -m || cat /proc/meminfo")
    );
    assert_eq!(preset_command("network"), Some("ip addr || ifconfig"));
    assert_eq!(preset_command("working_directory"), Some("pwd"));
    assert_eq!(
        preset_command("dns"),
        Some("getent hosts deepseek.com || nslookup deepseek.com || cat /etc/resolv.conf")
    );
    assert_eq!(
        preset_command("cpu_memory"),
        Some(
            "uptime; free -m || cat /proc/meminfo; ps -eo pid,ppid,comm,%cpu,%mem --sort=-%cpu | head -20"
        )
    );
    assert_eq!(
        preset_command("services"),
        Some(
            "systemctl list-units --type=service --state=running --no-pager || ps -eo pid,comm,args | head -50"
        )
    );
    assert_eq!(
        preset_command("docker"),
        Some(
            "docker ps --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' || docker info"
        )
    );
    assert_eq!(
        preset_command("openwrt_network"),
        Some("ubus call system board; ip route; ip addr; cat /etc/resolv.conf")
    );
    assert_eq!(
        preset_command("logs"),
        Some("journalctl -n 80 --no-pager || logread -l 80 || dmesg | tail -80")
    );
    assert_eq!(preset_command("other"), None);
}

#[test]
fn diagnostics_presets_all_map_to_preset_commands() {
    let presets = preset_diagnostics();

    assert!(!presets.is_empty());
    for preset in presets {
        assert_eq!(preset_command(&preset.key), Some(preset.command.as_str()));
        assert!(!preset.label.trim().is_empty());
        assert!(!preset.requires_approval);
    }
}

#[test]
fn diagnostics_include_common_read_only_presets() {
    let presets = preset_diagnostics();
    let keys = presets
        .iter()
        .map(|preset| preset.key.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        keys,
        vec![
            "system_info",
            "current_user",
            "disk_usage",
            "memory",
            "network",
            "working_directory",
            "dns",
            "cpu_memory",
            "services",
            "docker",
            "openwrt_network",
            "logs",
        ]
    );

    for preset in presets {
        let command = preset.command.to_ascii_lowercase();
        for forbidden in [
            " install ",
            " update ",
            " upgrade ",
            " restart",
            " reboot",
            " delete",
            " rm ",
            " rm -",
            " rmdir",
            " mkdir",
            " touch ",
            " tee ",
            " >",
            ">>",
        ] {
            assert!(
                !command.contains(forbidden),
                "{forbidden:?} should not appear in read-only preset {}: {}",
                preset.key,
                preset.command
            );
        }
    }
}

#[test]
fn preset_diagnostics_execute_without_approval() {
    let runner = FakeRunner::new(SshCommandOutput {
        stdout: "Linux test-host\n".to_string(),
        stderr: String::new(),
        exit_code: Some(0),
        duration: Duration::from_millis(15),
        timed_out: false,
    });
    let state = AppState::new(target());
    let service = DiagnosticService::new(runner.clone());

    let run = service
        .run(
            &state,
            DiagnosticRequest {
                session_id: "session-1".to_string(),
                diagnostic: "system_info".to_string(),
            },
        )
        .expect("preset diagnostic should run");

    assert_eq!(runner.calls(), vec!["uname -a"]);
    assert_eq!(state.pending_approvals(), Vec::new());
    assert_eq!(run.response.status, "completed");
    assert_eq!(run.response.result["requires_approval"], false);
}

#[test]
fn diagnostics_capture_response_events_and_audit_data() {
    let runner = FakeRunner::new(SshCommandOutput {
        stdout: "uid=0(root)\n".to_string(),
        stderr: "warning on stderr\n".to_string(),
        exit_code: Some(7),
        duration: Duration::from_millis(42),
        timed_out: false,
    });
    let state = AppState::new(target());
    let mut events = state.subscribe();
    let service = DiagnosticService::new(runner);

    let run = service
        .run(
            &state,
            DiagnosticRequest {
                session_id: "session-1".to_string(),
                diagnostic: "current_user".to_string(),
            },
        )
        .expect("diagnostic should run");

    assert_eq!(run.response.result["stdout"], "uid=0(root)\n");
    assert_eq!(run.response.result["stderr"], "warning on stderr\n");
    assert_eq!(run.response.result["exit_code"], 7);
    assert_eq!(run.response.result["duration_ms"], 42);

    let emitted: Vec<_> = (0..4)
        .map(|_| events.try_recv().expect("expected diagnostic event"))
        .collect();
    assert_eq!(
        emitted
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        vec![
            "tool.started",
            "tool.stdout",
            "tool.stderr",
            "tool.completed"
        ]
    );
    assert_eq!(emitted[1].payload["stdout"], "uid=0(root)\n");
    assert_eq!(emitted[2].payload["stderr"], "warning on stderr\n");
    assert_eq!(emitted[3].payload["exit_code"], 7);
    assert_eq!(emitted[3].payload["duration_ms"], 42);

    let audit = state.audit_recent();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].kind, "diagnostic.completed");
    assert_eq!(audit[0].session_id.as_deref(), Some("session-1"));
    assert_eq!(audit[0].metadata["stdout"], "uid=0(root)\n");
    assert_eq!(audit[0].metadata["stderr"], "warning on stderr\n");
    assert_eq!(audit[0].metadata["exit_code"], 7);
    assert_eq!(audit[0].metadata["duration_ms"], 42);
}

#[test]
fn diagnostics_audit_output_redacts_token_like_sentinel_fields() {
    let runner = FakeRunner::new(SshCommandOutput {
        stdout: "API_TOKEN=secret-token\nnonce=secret-nonce\nsafe=value\n".to_string(),
        stderr: "command_lease_secret=secret-lease\nbearer_token=secret-bearer\n".to_string(),
        exit_code: Some(0),
        duration: Duration::from_millis(3),
        timed_out: false,
    });
    let state = AppState::new(target());
    let service = DiagnosticService::new(runner);

    service
        .run(
            &state,
            DiagnosticRequest {
                session_id: "session-1".to_string(),
                diagnostic: "working_directory".to_string(),
            },
        )
        .expect("diagnostic should run");

    let audit_json = serde_json::to_string(&state.audit_recent()).expect("audit serializes");
    assert!(!audit_json.contains("secret-token"));
    assert!(!audit_json.contains("secret-nonce"));
    assert!(!audit_json.contains("secret-lease"));
    assert!(!audit_json.contains("secret-bearer"));
    assert!(audit_json.contains("[REDACTED]"));
    assert!(audit_json.contains("safe=value"));
}

#[test]
fn diagnostics_unknown_preset_is_rejected_before_running_ssh() {
    let runner = FakeRunner::new(SshCommandOutput {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
        duration: Duration::ZERO,
        timed_out: false,
    });
    let service = DiagnosticService::new(runner.clone());

    let err = service
        .run(
            &AppState::new(target()),
            DiagnosticRequest {
                session_id: "session-1".to_string(),
                diagnostic: "rm_everything".to_string(),
            },
        )
        .expect_err("unknown diagnostic should be rejected");

    assert!(matches!(err, DiagnosticError::UnknownDiagnostic { .. }));
    assert!(runner.calls().is_empty());
}
