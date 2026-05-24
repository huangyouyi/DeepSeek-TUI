use deepseek_mobile_agent_core::persistence::{
    PendingTurnSnapshot, PendingTurnStore, SessionSnapshot, SnapshotStore, SqliteSnapshotStore,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn session_snapshot() -> SessionSnapshot {
    SessionSnapshot {
        session_id: "session-1".to_string(),
        title: "phone rescue".to_string(),
        event_count: 3,
        bootstrap_step_count: 1,
        bootstrap_steps: vec![],
    }
}

fn pending_turn_snapshot() -> PendingTurnSnapshot {
    PendingTurnSnapshot {
        session_id: "session-1".to_string(),
        turn_id: "turn-7".to_string(),
        prompt: "repair the mobile bootstrap".to_string(),
        created_at: "2026-05-23T12:34:56Z".to_string(),
        pending_approval_ids: vec!["approval-shell-danger".to_string()],
        pending_tool_call_ids: vec!["call-1".to_string(), "call-2".to_string()],
        pending_approvals: vec![],
    }
}

fn sqlite_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "deepseek-mobile-agent-core-sqlite-persistence-{nanos}.db"
    ))
}

#[test]
fn sqlite_snapshot_store_saves_and_loads_session_snapshot() {
    let path = sqlite_path();
    let snapshot = session_snapshot();
    let mut store = SqliteSnapshotStore::open(&path).expect("sqlite store should open");

    store.save(snapshot.clone());

    drop(store);
    let store = SqliteSnapshotStore::open(&path).expect("sqlite store should reopen");
    assert_eq!(store.load(&snapshot.session_id), Some(snapshot));

    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlite_snapshot_store_saves_and_loads_pending_turn() {
    let snapshot = pending_turn_snapshot();
    let mut store = SqliteSnapshotStore::in_memory().expect("sqlite store should open");

    store.save_pending_turn(snapshot.clone());

    assert_eq!(
        store.load_pending_turn(&snapshot.session_id, &snapshot.turn_id),
        Some(snapshot)
    );
}

#[test]
fn sqlite_snapshot_store_returns_none_for_missing_records() {
    let store = SqliteSnapshotStore::in_memory().expect("sqlite store should open");

    assert_eq!(store.load("missing-session"), None);
    assert_eq!(store.load_pending_turn("missing-session", "turn-7"), None);
    assert_eq!(store.load_pending_turn("session-1", "missing-turn"), None);
}
