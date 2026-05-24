use deepseek_mobile_agent_core::uniffi_api::IosMobileAgentCore;
use serde_json::{Value, json};

#[test]
fn session_snapshot_json_returns_swift_friendly_snapshot() {
    let mut core = IosMobileAgentCore::new();
    let session_id = core.create_session("phone rescue".to_string());
    let step = core
        .homebrew_bootstrap_step(session_id.clone())
        .expect("bootstrap step should be returned");
    core.submit_bootstrap_output(
        session_id.clone(),
        step.step_id,
        "Darwin example\n/usr/local/bin/brew".to_string(),
    )
    .expect("output should be recorded");

    let snapshot_json = core
        .session_snapshot_json(session_id.clone())
        .expect("snapshot should serialize");
    let snapshot: Value = serde_json::from_str(&snapshot_json).expect("snapshot should be JSON");

    assert_eq!(snapshot["session_id"], session_id);
    assert_eq!(snapshot["title"], "phone rescue");
    assert_eq!(snapshot["event_count"], 3);
    assert_eq!(snapshot["bootstrap_step_count"], 1);
    assert_eq!(
        snapshot["bootstrap_steps"][0]["submitted_output"],
        "Darwin example\n/usr/local/bin/brew"
    );
}

#[test]
fn connection_bootstrap_returns_manual_bootstrap_profile() {
    let core = IosMobileAgentCore::new();

    let profile = core.connection_bootstrap("mac-mini".to_string(), "Mac mini".to_string());

    assert_eq!(profile.id, "mac-mini");
    assert_eq!(profile.label, "Mac mini");
    assert_eq!(profile.status, "manual_bootstrap");
    assert_eq!(profile.mode, "bootstrap");
    assert_eq!(profile.tool_names, vec!["remote.bootstrap.guide"]);
}

#[test]
fn connection_runner_from_tools_filters_and_deduplicates_tool_names() {
    let core = IosMobileAgentCore::new();

    let profile = core.connection_runner_from_tools(
        "runner-1".to_string(),
        "Runner".to_string(),
        vec![
            "remote.shell.exec".to_string(),
            "remote.file.delete".to_string(),
            "remote.shell.exec".to_string(),
            "remote.mcp.call".to_string(),
        ],
    );

    assert_eq!(profile.id, "runner-1");
    assert_eq!(profile.label, "Runner");
    assert_eq!(profile.status, "connected");
    assert_eq!(profile.mode, "runner");
    assert_eq!(
        profile.tool_names,
        vec!["remote.shell.exec", "remote.mcp.call"]
    );
}

#[test]
fn connection_next_step_returns_json_for_profile_state() {
    let core = IosMobileAgentCore::new();
    let bootstrap = core.connection_bootstrap("mac-mini".to_string(), "Mac mini".to_string());
    let runner = core.connection_runner_from_tools(
        "runner-1".to_string(),
        "Runner".to_string(),
        vec!["remote.shell.exec".to_string()],
    );

    let bootstrap_step: Value = serde_json::from_str(&core.connection_next_step(bootstrap))
        .expect("next step should be JSON");
    let runner_step: Value =
        serde_json::from_str(&core.connection_next_step(runner)).expect("next step should be JSON");

    assert_eq!(bootstrap_step["action"], "show_bootstrap_instructions");
    assert_eq!(bootstrap_step["connection_id"], "mac-mini");
    assert_eq!(runner_step["action"], "start_remote_agent_loop");
    assert_eq!(runner_step["connection_id"], "runner-1");
}

#[test]
fn bootstrap_install_steps_json_returns_swift_friendly_install_steps() {
    let core = IosMobileAgentCore::new();

    let steps_json = core
        .bootstrap_install_steps_json()
        .expect("install steps should serialize");
    let steps: Value = serde_json::from_str(&steps_json).expect("steps should be JSON");

    assert_eq!(steps["steps"].as_array().unwrap().len(), 2);
    assert_eq!(steps["steps"][0]["platform"], "macos");
    assert_eq!(
        steps["steps"][0]["step_id"],
        "macos-runner-install-bootstrap"
    );
    assert_eq!(steps["steps"][0]["approval_required"], true);
    assert!(
        steps["steps"][0]["commands"][0]
            .as_str()
            .unwrap()
            .contains("kai-runner pair")
    );
    assert_eq!(steps["steps"][1]["platform"], "windows");
    assert_eq!(
        steps["steps"][1]["step_id"],
        "windows-runner-install-bootstrap"
    );
    assert!(
        steps["steps"][1]["commands"][0]
            .as_str()
            .unwrap()
            .contains("Invoke-WebRequest")
    );
}

#[test]
fn runner_maintenance_audit_json_parses_response_without_leaking_secrets() {
    let core = IosMobileAgentCore::new();
    let raw_nonce = "approval-nonce-secret";
    let bearer_token = "runner-bearer-secret";

    let audit_json = core
        .runner_maintenance_audit_json(
            "session-1".to_string(),
            json!({
                "action": "self_update",
                "token": bearer_token,
                "audit": [
                    {
                        "event": "maintenance.approval_nonce",
                        "status": "consumed",
                        "nonce": {
                            "label": "approval_nonce",
                            "status": "consumed",
                            "raw": raw_nonce
                        },
                        "authorization": format!("Bearer {bearer_token}")
                    },
                    {
                        "event": "maintenance.execute",
                        "status": "no_op",
                        "reason": "dry_run"
                    }
                ]
            })
            .to_string(),
        )
        .expect("maintenance audit should serialize");

    let audit: Value = serde_json::from_str(&audit_json).expect("audit should be JSON");
    assert_eq!(audit["body"]["action"], "self_update");
    assert!(audit["body"].get("audit").is_none());
    assert_eq!(audit["audit_entries"].as_array().unwrap().len(), 2);
    assert_eq!(audit["audit_entries"][0]["kind"], "runner_maintenance");
    assert_eq!(audit["audit_entries"][0]["action"], "approval_nonce");
    assert_eq!(audit["audit_records"][1]["reason"], "dry_run");
    assert!(!audit_json.contains(raw_nonce));
    assert!(!audit_json.contains(bearer_token));
}

#[test]
fn runner_recent_audit_json_matches_ios_command_lease_timeline_contract_without_leaking_secrets() {
    let core = IosMobileAgentCore::new();
    let audit_json = core
        .runner_recent_audit_json(
            "session-lease".to_string(),
            json!({
                "audit": [
                    {
                        "event": "shell.command_lease",
                        "status": "accepted",
                        "lease": {
                            "id": "lease-secret",
                            "label": "command_lease",
                            "status": "accepted",
                            "idempotency_key": "idem-secret"
                        },
                        "metadata": {
                            "call_id": "lease-call",
                            "tool": "remote.shell.exec",
                            "token": "runner-token-secret",
                            "command": "echo command-secret",
                            "env": {
                                "SAFE_ENV": "env-secret-value"
                            }
                        }
                    },
                    {
                        "event": "shell.command_lease",
                        "status": "consumed",
                        "lease": {
                            "label": "command_lease",
                            "status": "consumed",
                            "id": "lease-consumed-secret"
                        },
                        "metadata": {
                            "call_id": "lease-consumed",
                            "tool": "remote.shell.exec",
                            "command": "rm consumed-secret"
                        }
                    },
                    {
                        "event": "shell.command_lease",
                        "status": "replay_rejected",
                        "lease": {
                            "label": "command_lease",
                            "status": "replayed"
                        },
                        "metadata": {
                            "call_id": "lease-replay",
                            "tool": "remote.shell.exec",
                            "error_code": "approval_replayed"
                        }
                    },
                    {
                        "event": "shell.command_lease",
                        "status": "expired_rejected",
                        "lease": {
                            "label": "command_lease",
                            "status": "expired"
                        },
                        "metadata": {
                            "call_id": "lease-expired",
                            "tool": "remote.shell.exec",
                            "error_code": "approval_expired",
                            "authorization": "Bearer expired-token-secret"
                        }
                    },
                    {
                        "event": "shell.command_lease",
                        "status": "invalid_action_rejected",
                        "lease": {
                            "label": "command_lease",
                            "status": "invalid_action"
                        },
                        "metadata": {
                            "call_id": "lease-invalid",
                            "tool": "remote.shell.exec",
                            "error_code": "approval_required",
                            "env": {
                                "SECRET_ENV": "invalid-env-secret"
                            }
                        }
                    }
                ]
            })
            .to_string(),
        )
        .expect("recent audit should serialize");

    let audit: Value = serde_json::from_str(&audit_json).expect("audit should be JSON");
    let entries = audit["audit_entries"].as_array().unwrap();
    assert_eq!(entries.len(), 5);
    assert_eq!(
        entries
            .iter()
            .map(|entry| (
                entry["kind"].as_str().unwrap(),
                entry["action"].as_str().unwrap(),
                entry["summary"].as_str().unwrap(),
                entry["call_id"].as_str().unwrap(),
                entry["detail"].as_str().unwrap(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "runner_command_lease",
                "accepted",
                "command lease accepted",
                "lease-call",
                "lease accepted call_id=lease-call tool=remote.shell.exec",
            ),
            (
                "runner_command_lease",
                "consumed",
                "command lease consumed",
                "lease-consumed",
                "lease consumed call_id=lease-consumed tool=remote.shell.exec",
            ),
            (
                "runner_command_lease",
                "replay_rejected",
                "command lease replay_rejected",
                "lease-replay",
                "lease replay rejected call_id=lease-replay tool=remote.shell.exec error_code=approval_replayed",
            ),
            (
                "runner_command_lease",
                "expired_rejected",
                "command lease expired_rejected",
                "lease-expired",
                "lease expired rejected call_id=lease-expired tool=remote.shell.exec error_code=approval_expired",
            ),
            (
                "runner_command_lease",
                "invalid_action_rejected",
                "command lease invalid_action_rejected",
                "lease-invalid",
                "lease invalid rejected call_id=lease-invalid tool=remote.shell.exec error_code=approval_required",
            ),
        ]
    );
    assert!(!audit_json.contains("lease-secret"));
    assert!(!audit_json.contains("lease-consumed-secret"));
    assert!(!audit_json.contains("idem-secret"));
    assert!(!audit_json.contains("runner-token-secret"));
    assert!(!audit_json.contains("expired-token-secret"));
    assert!(!audit_json.contains("command-secret"));
    assert!(!audit_json.contains("consumed-secret"));
    assert!(!audit_json.contains("env-secret-value"));
    assert!(!audit_json.contains("invalid-env-secret"));
    assert!(!audit_json.contains("SAFE_ENV"));
    assert!(!audit_json.contains("SECRET_ENV"));
    assert!(!audit_json.contains("lease_label"));
    assert!(!audit_json.contains("idempotency_key"));
}

#[test]
fn runner_maintenance_plan_json_exposes_artifact_verification_without_leaking_secrets() {
    let core = IosMobileAgentCore::new();
    let bearer_token = "runner-plan-token-secret";
    let approval_nonce = "approval-nonce-secret";

    let plan_json = core
        .runner_maintenance_plan_json(
            "11111111-1111-1111-1111-111111111111".to_string(),
            json!({
                "type": "maintenance_plan",
                "action": "self_update",
                "dry_run": true,
                "planned": true,
                "title": "Runner self-update",
                "connection_id_label": "runner-core",
                "requested_by": "paired mobile session",
                "target": {
                    "summary": "deepseek runner 0.6.6 -> 0.6.7"
                },
                "risk": {
                    "level": "high",
                    "approval_required": true,
                    "reason": "self-update can replace the active runner binary"
                },
                "artifact": {
                    "name": "deepseek-tui-aarch64-apple-darwin.tar.gz",
                    "checksum": {
                        "source": "release manifest sha256"
                    },
                    "signature": {
                        "source": "minisign release signature"
                    },
                    "download_url": format!("https://runner.example/download?token={bearer_token}")
                },
                "verification": {
                    "step": "Verify sha256 checksum and minisign signature before staging."
                },
                "rollback": {
                    "guidance": "Keep current runner binary until a later execution approval succeeds."
                },
                "steps": [
                    "download signed runner artifact",
                    "verify artifact signature",
                    "replace runner after approval"
                ],
                "approval": {
                    "nonce": approval_nonce
                }
            })
            .to_string(),
        )
        .expect("maintenance plan should serialize");

    let plan: Value = serde_json::from_str(&plan_json).expect("plan should be JSON");
    let request = &plan["requests"][0];
    assert_eq!(
        request["app_session_id"],
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(request["operation"], "self_update");
    assert_eq!(request["approval_status"], "waiting_for_dry_run_approval");
    assert_eq!(request["dry_run_steps"].as_array().unwrap().len(), 3);
    assert_eq!(
        request["artifact_verification"]["artifact_name"],
        "deepseek-tui-aarch64-apple-darwin.tar.gz"
    );
    assert_eq!(
        request["artifact_verification"]["checksum_source"],
        "release manifest sha256"
    );
    assert_eq!(
        request["artifact_verification"]["signature_source"],
        "minisign release signature"
    );
    assert_eq!(
        request["artifact_verification"]["verify_step"],
        "Verify sha256 checksum and minisign signature before staging."
    );
    assert_eq!(
        request["artifact_verification"]["rollback_guidance"],
        "Keep current runner binary until a later execution approval succeeds."
    );
    assert!(!plan_json.contains(bearer_token));
    assert!(!plan_json.contains(approval_nonce));
    assert!(!plan_json.contains("download_url"));
    assert!(!plan_json.contains("nonce"));
}

#[test]
fn browser_audit_json_parses_response_without_leaking_secrets() {
    let core = IosMobileAgentCore::new();
    let raw_nonce = "raw-browser-approval-nonce";
    let bearer_token = "runner-browser-token";

    let audit_json = core
        .browser_audit_json(
            "mobile-session-1".to_string(),
            json!({
                "type": "tool_call",
                "call_id": "browser-click-download",
                "success": false,
                "result": {
                    "tool": "remote.browser.click",
                    "status": "error",
                    "error": { "code": "approval_required" },
                    "data": {
                        "session_id": "browser-session-1",
                        "selector": "a[data-action=download]",
                        "action": "download",
                        "approval_required": true,
                        "risk": "state_changing",
                        "metadata": {
                            "scenario": "install-pro-editor",
                            "nonce": raw_nonce,
                            "authorization": format!("Bearer {bearer_token}")
                        },
                        "audit": [{
                            "type": "browser_action_blocked",
                            "tool": "remote.browser.click",
                            "action": "download",
                            "session_id": "browser-session-1",
                            "selector": "a[data-action=download]",
                            "reason": "approval_required",
                            "raw_nonce": raw_nonce
                        }]
                    }
                }
            })
            .to_string(),
        )
        .expect("browser audit should serialize");

    let audit: Value = serde_json::from_str(&audit_json).expect("audit should be JSON");
    assert_eq!(audit["output"]["call_id"], "browser-click-download");
    assert_eq!(
        audit["audit_records"][0]["record_type"],
        "browser_action_blocked"
    );
    assert_eq!(audit["audit_entries"][0]["kind"], "runner_browser");
    assert_eq!(audit["audit_entries"][0]["action"], "download");
    assert_eq!(
        audit["audit_records"][0]["metadata"]["scenario"],
        "install-pro-editor"
    );
    assert!(!audit_json.contains(raw_nonce));
    assert!(!audit_json.contains(bearer_token));
}

#[test]
fn capability_report_upgrade_json_returns_runner_profile() {
    let core = IosMobileAgentCore::new();
    let report = r#"
{
  "capabilities": {
    "tools": [
      "remote.shell.exec",
      "remote.file.read",
      "remote.mcp.call",
      "unknown.tool"
    ]
  }
}
"#;

    let upgrade_json = core
        .capability_report_upgrade_json(
            "mac".to_string(),
            "User Mac".to_string(),
            report.to_string(),
        )
        .expect("capability report should upgrade");
    let upgrade: Value = serde_json::from_str(&upgrade_json).expect("upgrade should be JSON");

    assert_eq!(upgrade["profile"]["id"], "mac");
    assert_eq!(upgrade["profile"]["status"], "connected");
    assert_eq!(upgrade["profile"]["mode"], "runner");
    assert_eq!(
        upgrade["profile"]["tool_names"],
        json!(["remote.shell.exec", "remote.file.read", "remote.mcp.call"])
    );
    assert_eq!(upgrade["action"], "start_remote_agent_loop");
}
