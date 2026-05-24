use deepseek_mobile_agent_core::persistence::{
    InMemorySnapshotStore, PendingTurnSnapshot, PendingTurnStore,
};

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

#[test]
fn pending_turn_snapshot_round_trips_through_json() {
    let snapshot = pending_turn_snapshot();

    let encoded = serde_json::to_string(&snapshot).expect("pending turn should serialize");
    let decoded: PendingTurnSnapshot =
        serde_json::from_str(&encoded).expect("pending turn should deserialize");

    assert_eq!(decoded, snapshot);
}

#[test]
fn in_memory_snapshot_store_saves_and_loads_pending_turn_by_session_and_turn() {
    let snapshot = pending_turn_snapshot();
    let mut store = InMemorySnapshotStore::default();

    store.save_pending_turn(snapshot.clone());

    assert_eq!(
        store.load_pending_turn(&snapshot.session_id, &snapshot.turn_id),
        Some(snapshot)
    );
    assert_eq!(store.load_pending_turn("missing-session", "turn-7"), None);
    assert_eq!(store.load_pending_turn("session-1", "missing-turn"), None);
}
