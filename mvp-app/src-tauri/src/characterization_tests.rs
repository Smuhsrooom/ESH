//! Stage 1 storage characteristics. These tests use isolated SQLite files and no CLI.
use crate::store::{Route, Store};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn local_sessions_keep_turns_versions_and_resume_bindings_separate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chat.sqlite");
    let store = Store::open(&path).unwrap();
    let other = store.create_session("default", "second").unwrap();
    let first = store.routed_message("user", "first request", false, Route {
        channel: "main", kind: "user_message", chat_session_id: Some("default-session".into()), ..Route::default()
    }).unwrap();
    let second = store.routed_message("user", "second request", false, Route {
        channel: "main", kind: "user_message", chat_session_id: Some(other.id.clone()), ..Route::default()
    }).unwrap();
    store.bind_codex_session("default-session", "thread-first", "rpc-first").unwrap();
    store.bind_codex_session(&other.id, "thread-second", "rpc-second").unwrap();
    store.bind_messages(&[first.id.clone()], "thread-first", "turn-first").unwrap();
    store.delivery(&[first.id.clone()], "delivered").unwrap();
    store.set_busy_session(Some("default-session".into())).unwrap();
    assert_eq!(store.snapshot().unwrap().busy_session_id.as_deref(), Some("default-session"));
    assert_eq!(store.session_version("default-session").unwrap(), 1);
    assert_eq!(store.session_version(&other.id).unwrap(), 1);
    assert!(store.context().unwrap().contains("first request"));
    assert!(!store.context().unwrap().contains("second request"));
    drop(store);

    let reopened = Store::open(&path).unwrap();
    let snapshot = reopened.snapshot().unwrap();
    assert!(snapshot.busy_session_id.is_none()); // Running state is memory only.
    assert_eq!(reopened.session("default-session").unwrap().thread_id.as_deref(), Some("thread-first"));
    assert_eq!(reopened.session(&other.id).unwrap().thread_id.as_deref(), Some("thread-second"));
    let saved_first = snapshot.messages.iter().find(|m| m.id == first.id).unwrap();
    let saved_second = snapshot.messages.iter().find(|m| m.id == second.id).unwrap();
    assert_eq!((saved_first.delivery.as_str(), saved_first.turn_id.as_deref()), ("delivered", Some("turn-first")));
    assert_eq!((saved_second.delivery.as_str(), saved_second.turn_id.as_deref()), ("interrupted", None));
    assert_eq!(saved_first.content, "first request");
    assert_eq!(saved_second.content, "second request");
}

#[test]
fn review_records_and_tool_delivery_survive_restart_without_replaying_open_call() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chat.sqlite");
    let store = Store::open(&path).unwrap();
    let user = store.message("user", "review this", false).unwrap();
    let request = store.routed_message("codex", "design question", false, Route {
        channel: "collaboration", kind: "review_request", thread_id: Some("thread".into()),
        turn_id: Some("turn".into()), collaboration_id: Some("call".into()), ..Route::default()
    }).unwrap();
    let reply = store.routed_message("antigravity", "review answer", false, Route {
        channel: "collaboration", kind: "review_reply", thread_id: Some("thread".into()),
        turn_id: Some("turn".into()), collaboration_id: Some("call".into()), ..Route::default()
    }).unwrap();
    store.bind_messages(&[user.id.clone()], "thread", "turn").unwrap();
    store.delivery(&[user.id.clone()], "delivered").unwrap();
    assert!(store.reserve_tool("connection:thread:turn:call", &json!("rpc-1")).unwrap());
    assert!(!store.reserve_tool("connection:thread:turn:call", &json!("rpc-1")).unwrap());
    store.tool_state("connection:thread:turn:call", "sent").unwrap();
    store.status("codex", "resuming", "waiting").unwrap();
    drop(store);

    let reopened = Store::open(&path).unwrap();
    let snapshot = reopened.snapshot().unwrap();
    assert_eq!(snapshot.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), [user.id.as_str(), request.id.as_str(), reply.id.as_str()]);
    assert_eq!(snapshot.messages[1].collaboration_id.as_deref(), Some("call"));
    assert_eq!(snapshot.messages[2].kind, "review_reply");
    assert_eq!(snapshot.agents.iter().find(|a| a.id == "codex").unwrap().status, "unknown");
    assert!(!reopened.reserve_tool("connection:thread:turn:call", &json!("rpc-1")).unwrap());
    let db = Connection::open(&path).unwrap();
    let state: String = db.query_row("SELECT state FROM tool_deliveries WHERE call_key=?1", ["connection:thread:turn:call"], |r| r.get(0)).unwrap();
    assert_eq!(state, "unknown");
    let schema: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(schema, 6);
}

#[test]
fn late_user_requirement_is_queued_only_in_its_session() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("chat.sqlite")).unwrap();
    let other = store.create_session("default", "other").unwrap();
    let review_version = store.session_version("default-session").unwrap();
    let current = store.routed_message("user", "new requirement", false, Route {
        channel: "main", kind: "user_message", chat_session_id: Some("default-session".into()), ..Route::default()
    }).unwrap();
    let foreign = store.routed_message("user", "other session work", false, Route {
        channel: "main", kind: "user_message", chat_session_id: Some(other.id), ..Route::default()
    }).unwrap();
    let pending: Vec<_> = store.snapshot().unwrap().messages.into_iter().filter(|m|
        m.speaker == "user" && m.chat_session_id == "default-session" && m.delivery == "queued"
    ).collect();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, current.id);
    assert_ne!(pending[0].id, foreign.id);
    assert_ne!(review_version, store.session_version("default-session").unwrap());
    store.bind_messages(&[current.id.clone()], "thread", "turn").unwrap();
    store.delivery(&[current.id.clone()], "delivered").unwrap();
    assert_eq!(store.snapshot().unwrap().messages.iter().find(|m| m.id == current.id).unwrap().delivery, "delivered");
    assert_eq!(store.snapshot().unwrap().messages.iter().find(|m| m.id == foreign.id).unwrap().delivery, "queued");
}

#[test]
fn closing_admission_refuses_every_new_work_command() {
    // Regression for the shutdown gap: only `submit` used to check admission,
    // so room/session creation and external list/import could still start a
    // worker job, a SQLite write, or a short-lived Codex process while the
    // worker was finalizing state and checkpointing WAL.
    use crate::engine::ensure_accepting;
    use std::sync::atomic::{AtomicBool, Ordering};
    let accepting = AtomicBool::new(true);
    assert!(ensure_accepting(&accepting).is_ok());
    // `shutdown` closes admission first, before it signals the worker.
    accepting.store(false, Ordering::SeqCst);
    let first = ensure_accepting(&accepting).unwrap_err();
    assert!(first.contains("종료 중"), "{first}");
    // Nothing in the guard reopens admission, so every later command fails too.
    assert!(ensure_accepting(&accepting).is_err());
    assert!(!accepting.load(Ordering::SeqCst));
}

#[test]
fn confirmed_turn_is_atomic_and_cannot_run_the_same_message_twice() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("turn.sqlite")).unwrap();
    let a = store.message("user", "first", false).unwrap();
    let b = store.message("user", "second", false).unwrap();
    let c = store.message("user", "third", false).unwrap();
    assert!(store.snapshot().unwrap().messages.iter().all(|m| m.delivery == "queued" && m.turn_id.is_none()));
    store.confirm_turn_start(&a.id, "thread-a", "turn-1").unwrap();
    assert!(store.confirm_turn_start(&a.id, "thread-a", "turn-duplicate").is_err());
    store.confirm_turn_start(&b.id, "thread-a", "turn-2").unwrap();
    store.delivery(&[c.id.clone()], "cancelled").unwrap();
    assert!(store.confirm_turn_start(&c.id, "thread-a", "turn-3").is_err());
    let saved = store.snapshot().unwrap().messages;
    assert_eq!(saved.iter().map(|m| (m.delivery.as_str(), m.turn_id.as_deref())).collect::<Vec<_>>(), [
        ("delivered", Some("turn-1")), ("delivered", Some("turn-2")), ("cancelled", None)
    ]);
}
