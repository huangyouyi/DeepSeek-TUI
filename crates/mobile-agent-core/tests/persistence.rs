use deepseek_mobile_agent_core::persistence::InMemorySnapshotStore;
use deepseek_mobile_agent_core::{MobileAgentCore, SessionSnapshot, SnapshotStore};

fn session_with_bootstrap_output() -> (MobileAgentCore, String) {
    let mut core = MobileAgentCore::new();
    let session_id = core.create_session("phone rescue");
    core.homebrew_bootstrap_step(&session_id)
        .expect("bootstrap step should be created");
    core.submit_bootstrap_output(
        &session_id,
        "homebrew-xcode-clt-diagnostics",
        "Darwin example\n/usr/local/bin/brew",
    )
    .expect("bootstrap output should be recorded");
    (core, session_id)
}

#[test]
fn session_snapshot_summarizes_session_and_bootstrap_steps() {
    let (core, session_id) = session_with_bootstrap_output();
    let session = core.session(&session_id).expect("session should exist");

    let snapshot = SessionSnapshot::from_session(session);

    assert_eq!(snapshot.session_id, session_id);
    assert_eq!(snapshot.title, "phone rescue");
    assert_eq!(snapshot.event_count, 3);
    assert_eq!(snapshot.bootstrap_step_count, 1);
    assert_eq!(snapshot.bootstrap_steps.len(), 1);

    let step = &snapshot.bootstrap_steps[0];
    assert_eq!(step.step_id, "homebrew-xcode-clt-diagnostics");
    assert!(step.command.contains("command -v brew"));
    assert_eq!(
        step.submitted_output.as_deref(),
        Some("Darwin example\n/usr/local/bin/brew")
    );
}

#[test]
fn session_snapshot_round_trips_through_json() {
    let (core, session_id) = session_with_bootstrap_output();
    let session = core.session(&session_id).expect("session should exist");
    let snapshot = SessionSnapshot::from_session(session);

    let encoded = serde_json::to_string(&snapshot).expect("snapshot should serialize");
    let decoded: SessionSnapshot =
        serde_json::from_str(&encoded).expect("snapshot should deserialize");

    assert_eq!(decoded, snapshot);
}

#[test]
fn in_memory_snapshot_store_saves_and_loads_by_session_id() {
    let (core, session_id) = session_with_bootstrap_output();
    let session = core.session(&session_id).expect("session should exist");
    let snapshot = SessionSnapshot::from_session(session);
    let mut store = InMemorySnapshotStore::default();

    store.save(snapshot.clone());

    assert_eq!(store.load(&session_id), Some(snapshot));
    assert_eq!(store.load("missing-session"), None);
}
