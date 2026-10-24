//! Test-only third adapter. No executable, network call, or production registration.
use crate::{
    agents::{AgentAdapter, AgentCapabilities, AgentDescriptor, AgentFuture, AgentRole, ConnectionKind, LeadAdapter, LeadSession, ReviewAdapter},
    cli::validate_arch_result,
    models::ModelSettings,
    store::{Session, Store},
};
use serde_json::{json, Value};
use std::{collections::{HashMap, VecDeque}, sync::{Arc, Mutex}};
use tokio::sync::watch;

#[derive(Clone, Copy)]
enum ReviewOutcome { Success, Failure, Wait, WrongId }

struct MockAgent {
    outcome: ReviewOutcome,
    turns: Arc<Mutex<HashMap<String, usize>>>,
}

impl MockAgent {
    fn new(outcome: ReviewOutcome) -> Self {
        Self { outcome, turns: Arc::new(Mutex::new(HashMap::new())) }
    }
}

impl AgentAdapter for MockAgent {
    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor {
            id: "mock_agent", display_name: "Mock Agent", connection: ConnectionKind::MockInProcess,
            roles: &[AgentRole::Lead, AgentRole::Reviewer],
            capabilities: AgentCapabilities {
                lead_conversation: true, delegated_review: true, persistent_session: true,
                streaming: false, cancellation: true, model_catalog: false,
                context_compaction: false, tool_call_bridge: true,
            },
        }
    }
}

impl ReviewAdapter for MockAgent {
    fn review<'a>(&'a self, _store: Arc<Store>, question: &'a str, version: i64, mut cancel: watch::Receiver<bool>) -> AgentFuture<'a, Value> {
        Box::pin(async move {
            if *cancel.borrow() { return Err("mock review cancelled".into()); }
            let expected_id = format!("mock-review-{version}");
            match self.outcome {
                ReviewOutcome::Failure => Err("mock review failed".into()),
                ReviewOutcome::Wait => {
                    loop {
                        if cancel.changed().await.is_err() || *cancel.borrow() {
                            return Err("mock review cancelled".into());
                        }
                    }
                }
                ReviewOutcome::Success | ReviewOutcome::WrongId => {
                    let review_id = if matches!(self.outcome, ReviewOutcome::WrongId) { "wrong-review-id".to_string() } else { expected_id.clone() };
                    let value = json!({"reviewId":review_id,"conversationVersion":version,"answer":question,"status":"success"});
                    validate_arch_result(&value, &expected_id, version)?;
                    Ok(value)
                }
            }
        })
    }
}

struct MockLeadSession {
    turns: Arc<Mutex<HashMap<String, usize>>>,
    session_id: Option<String>,
    events: VecDeque<Value>,
    open_call: Option<Value>,
    cancelled: bool,
    closed: bool,
}

impl LeadAdapter for MockAgent {
    type Session = MockLeadSession;
    fn connect(&self, _store: Arc<Store>) -> AgentFuture<'_, Self::Session> {
        let turns = self.turns.clone();
        Box::pin(async move { Ok(MockLeadSession { turns, session_id: None, events: VecDeque::new(), open_call: None, cancelled: false, closed: false }) })
    }
}

impl LeadSession for MockLeadSession {
    fn open_or_resume<'a>(&'a mut self, saved: &'a Session) -> AgentFuture<'a, ()> {
        Box::pin(async move {
            if self.closed { return Err("mock connection closed".into()); }
            self.turns.lock().unwrap().entry(saved.id.clone()).or_default();
            self.session_id = Some(saved.id.clone());
            Ok(())
        })
    }
    fn begin_turn<'a>(&'a mut self, input: &'a str, _settings: &'a ModelSettings) -> AgentFuture<'a, ()> {
        Box::pin(async move {
            if self.closed || self.session_id.is_none() || self.open_call.is_some() { return Err("mock turn cannot start".into()); }
            let session = self.session_id.as_ref().unwrap();
            let mut turns = self.turns.lock().unwrap();
            let turn = turns.entry(session.clone()).or_default();
            *turn += 1;
            let call = json!({"id":format!("mock-rpc-{session}-{turn}"),"method":"item/tool/call","params":{"tool":"request_arch_review","callId":format!("mock-call-{session}-{turn}"),"arguments":{"question":input},"threadId":session,"turnId":format!("turn-{turn}")}});
            self.open_call = Some(call.clone());
            self.events.push_back(call);
            self.cancelled = false;
            Ok(())
        })
    }
    fn next_event(&mut self) -> AgentFuture<'_, Value> {
        Box::pin(async move { self.events.pop_front().ok_or_else(|| "no mock event".into()) })
    }
    fn reply_to_tool_call<'a>(&'a mut self, call: &'a Value, result: Value, _success: bool) -> AgentFuture<'a, ()> {
        Box::pin(async move {
            if self.closed || self.cancelled || self.open_call.as_ref() != Some(call) { return Err("wrong or duplicate mock call".into()); }
            if result["kind"] != "arch_review_delivery" { return Err("wrong mock delivery".into()); }
            self.open_call = None;
            self.events.push_back(json!({"method":"turn/completed","params":{"turn":{"id":call["params"]["turnId"],"status":"completed"}}}));
            Ok(())
        })
    }
    fn cancel_turn(&mut self) -> AgentFuture<'_, String> {
        Box::pin(async move {
            self.cancelled = true;
            self.open_call = None;
            self.events.clear();
            Ok("mock turn cancelled".into())
        })
    }
    fn close(&mut self) -> AgentFuture<'_, ()> {
        Box::pin(async move { self.closed = true; self.events.clear(); self.open_call = None; Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{registered, registered_with_mock};
    use std::time::Duration;

    fn store() -> (tempfile::TempDir, Arc<Store>) {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&dir.path().join("mock.sqlite")).unwrap());
        (dir, store)
    }

    #[test]
    fn mock_is_a_distinct_test_only_registry_entry_and_is_hidden_by_default() {
        let (_dir, store) = store();
        let mock = MockAgent::new(ReviewOutcome::Success);
        let production = registered();
        assert_eq!(production.len(), 2);
        assert!(!production.iter().any(|adapter| adapter.descriptor().id == "mock_agent"));
        assert!(!store.list_agent_instances().unwrap().iter().any(|i| i.provider == "mock_agent"));
        let test_registry = registered_with_mock(&mock);
        assert_eq!(test_registry[2].descriptor().connection, ConnectionKind::MockInProcess);
        assert_eq!(test_registry[2].descriptor().roles, [AgentRole::Lead, AgentRole::Reviewer]);
        store.register_mock_for_test("mock-reviewer", "reviewer").unwrap();
        store.register_mock_for_test("mock-lead", "lead").unwrap();
        let instances = store.list_agent_instances().unwrap();
        assert_eq!(instances.iter().filter(|i| i.provider == "mock_agent").count(), 2);
        assert!(instances.iter().filter(|i| i.provider == "mock_agent").all(|i| i.connection == "mock_in_process" && i.support == "supported"));
        assert!(store.register_mock_for_test("mock-lead", "lead").is_err());
        assert!(store.register_mock_for_test("mock-invalid", "other").is_err());
    }

    #[tokio::test]
    async fn reviewer_success_failure_timeout_cancel_and_wrong_id() {
        let (_dir, store) = store();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let success = MockAgent::new(ReviewOutcome::Success);
        let result = success.review(store.clone(), "review question", 7, cancel_rx.clone()).await.unwrap();
        assert_eq!(result["reviewId"], "mock-review-7");
        assert_eq!(result["answer"], "review question");
        let failure = MockAgent::new(ReviewOutcome::Failure);
        assert!(failure.review(store.clone(), "question", 7, cancel_rx.clone()).await.unwrap_err().contains("failed"));
        let wrong = MockAgent::new(ReviewOutcome::WrongId);
        assert!(wrong.review(store.clone(), "question", 7, cancel_rx.clone()).await.unwrap_err().contains("리뷰 ID"));
        let waiting = Arc::new(MockAgent::new(ReviewOutcome::Wait));
        assert!(tokio::time::timeout(Duration::from_millis(20), waiting.review(store.clone(), "question", 7, cancel_rx.clone())).await.is_err());
        let in_flight = waiting.clone();
        let pending_store = store.clone();
        let pending_cancel = cancel_rx.clone();
        let task = tokio::spawn(async move { in_flight.review(pending_store, "question", 7, pending_cancel).await });
        tokio::task::yield_now().await;
        cancel_tx.send(true).unwrap();
        assert!(tokio::time::timeout(Duration::from_secs(1), task).await.unwrap().unwrap().unwrap_err().contains("cancelled"));
        assert!(waiting.review(store, "question", 7, cancel_rx).await.unwrap_err().contains("cancelled"));
    }

    #[tokio::test]
    async fn lead_create_turn_cancel_resume_and_original_tool_call() {
        let (_dir, store) = store();
        let mock = MockAgent::new(ReviewOutcome::Success);
        let saved = store.session("default-session").unwrap();
        let settings = ModelSettings { model: None, reasoning_effort: None };
        let mut lead = mock.connect(store.clone()).await.unwrap();
        assert!(lead.begin_turn("too early", &settings).await.is_err());
        lead.open_or_resume(&saved).await.unwrap();
        lead.begin_turn("first", &settings).await.unwrap();
        let first = lead.next_event().await.unwrap();
        assert_eq!(first["params"]["tool"], "request_arch_review");
        assert!(lead.reply_to_tool_call(&json!({"id":"wrong"}), json!({"kind":"arch_review_delivery"}), true).await.is_err());
        lead.reply_to_tool_call(&first, json!({"kind":"arch_review_delivery","archReview":{"answer":"ok"}}), true).await.unwrap();
        assert!(lead.reply_to_tool_call(&first, json!({"kind":"arch_review_delivery"}), true).await.is_err());
        assert_eq!(lead.next_event().await.unwrap()["method"], "turn/completed");
        lead.begin_turn("second", &settings).await.unwrap();
        let second = lead.next_event().await.unwrap();
        assert_ne!(first["id"], second["id"]);
        assert_eq!(lead.cancel_turn().await.unwrap(), "mock turn cancelled");
        assert!(lead.reply_to_tool_call(&second, json!({"kind":"arch_review_delivery"}), true).await.is_err());
        lead.close().await.unwrap();
        assert!(lead.begin_turn("closed", &settings).await.is_err());
        let mut resumed = mock.connect(store).await.unwrap();
        resumed.open_or_resume(&saved).await.unwrap();
        resumed.begin_turn("third", &settings).await.unwrap();
        assert_eq!(resumed.next_event().await.unwrap()["params"]["turnId"], "turn-3");
    }

    #[tokio::test]
    async fn mock_blocking_delegation_replies_to_the_original_open_call() {
        let (_dir, store) = store();
        let mock = MockAgent::new(ReviewOutcome::Success);
        let mut lead = mock.connect(store.clone()).await.unwrap();
        lead.open_or_resume(&store.session("default-session").unwrap()).await.unwrap();
        let settings = ModelSettings { model: None, reasoning_effort: None };
        lead.begin_turn("review this", &settings).await.unwrap();
        let call = lead.next_event().await.unwrap();
        assert!(lead.begin_turn("parallel turn", &settings).await.is_err());
        let (_, cancel) = watch::channel(false);
        let review = mock.review(store, call["params"]["arguments"]["question"].as_str().unwrap(), 1, cancel).await.unwrap();
        let envelope = json!({"kind":"arch_review_delivery","archReview":review,"latestUserRequirements":{"messages":[]},"reviewIsStale":false});
        lead.reply_to_tool_call(&call, envelope, true).await.unwrap();
        assert_eq!(lead.next_event().await.unwrap()["method"], "turn/completed");
        assert!(lead.reply_to_tool_call(&call, json!({"kind":"arch_review_delivery"}), true).await.is_err());
    }
}
