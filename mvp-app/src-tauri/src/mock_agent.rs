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
    public_answer: bool,
}

impl MockAgent {
    fn new(outcome: ReviewOutcome) -> Self {
        Self { outcome, turns: Arc::new(Mutex::new(HashMap::new())), public_answer: false }
    }
    /// Scripts the public-answer events the real App Server sends for a turn
    /// (`item/started` → `item/agentMessage/delta` → `item/completed`), so the
    /// answer path can be driven without an executable or a network call.
    fn with_public_answer(outcome: ReviewOutcome) -> Self {
        Self { public_answer: true, ..Self::new(outcome) }
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
    public_answer: bool,
}

impl LeadAdapter for MockAgent {
    type Session = MockLeadSession;
    fn connect(&self, _store: Arc<Store>) -> AgentFuture<'_, Self::Session> {
        let turns = self.turns.clone();
        let public_answer = self.public_answer;
        Box::pin(async move { Ok(MockLeadSession { turns, session_id: None, events: VecDeque::new(), open_call: None, cancelled: false, closed: false, public_answer }) })
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
            let turn_id = format!("turn-{turn}");
            let call = json!({"id":format!("mock-rpc-{session}-{turn}"),"method":"item/tool/call","params":{"tool":"request_arch_review","callId":format!("mock-call-{session}-{turn}"),"arguments":{"question":input},"threadId":session,"turnId":turn_id}});
            if self.public_answer {
                self.events.push_back(json!({"method":"item/started","params":{"threadId":session,"turnId":turn_id,"item":{"id":format!("mock-item-{turn_id}"),"type":"agentMessage","phase":"final_answer"}}}));
                self.events.push_back(json!({"method":"item/agentMessage/delta","params":{"threadId":session,"turnId":turn_id,"itemId":format!("mock-item-{turn_id}"),"delta":"mock public "}}));
            }
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
            let session = self.session_id.clone().unwrap_or_default();
            let turn = call["params"]["turnId"].as_str().unwrap_or_default().to_string();
            if self.public_answer {
                self.events.push_back(json!({"method":"item/agentMessage/delta","params":{"threadId":session,"turnId":turn,"itemId":format!("mock-item-{turn}"),"delta":"answer"}}));
                self.events.push_back(json!({"method":"item/completed","params":{"threadId":session,"turnId":turn,"item":{"id":format!("mock-item-{turn}"),"type":"agentMessage","phase":"final_answer","text":"mock public answer"}}}));
            }
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
        assert_eq!(production.len(), 3);
        assert!(!production.iter().any(|adapter| adapter.descriptor().id == "mock_agent"));
        assert!(!store.list_agent_instances().unwrap().iter().any(|i| i.provider == "mock_agent"));
        let test_registry = registered_with_mock(&mock);
        assert_eq!(test_registry[3].descriptor().connection, ConnectionKind::MockInProcess);
        assert_eq!(test_registry[3].descriptor().roles, [AgentRole::Lead, AgentRole::Reviewer]);
        store.register_mock_for_test("mock-reviewer", "reviewer").unwrap();
        store.register_mock_for_test("mock-lead", "lead").unwrap();
        let instances = store.list_agent_instances().unwrap();
        assert_eq!(instances.iter().filter(|i| i.provider == "mock_agent").count(), 2);
        assert!(instances.iter().filter(|i| i.provider == "mock_agent").all(|i| i.connection == "mock_in_process" && i.support == "supported"));
        assert!(instances.iter().filter(|i| i.provider == "mock_agent").all(|i| i.active_role.is_none()));
        assert!(store.register_mock_for_test("mock-lead", "lead").is_err());
        assert!(store.register_mock_for_test("mock-invalid", "other").is_err());
    }

    #[tokio::test]
    async fn reviewer_success_preserves_question_version_and_id() {
        let (_dir, store) = store();
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let success = MockAgent::new(ReviewOutcome::Success);
        let result = success.review(store, "review question", 7, cancel_rx).await.unwrap();
        assert_eq!(result["reviewId"], "mock-review-7");
        assert_eq!(result["answer"], "review question");
        assert_eq!(result["conversationVersion"], 7);
    }

    #[tokio::test]
    async fn reviewer_failure_is_not_a_successful_review() {
        let (_dir, store) = store();
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let failure = MockAgent::new(ReviewOutcome::Failure);
        assert!(failure.review(store, "question", 7, cancel_rx).await.unwrap_err().contains("failed"));
    }

    #[tokio::test]
    async fn reviewer_rejects_wrong_review_id() {
        let (_dir, store) = store();
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let wrong = MockAgent::new(ReviewOutcome::WrongId);
        assert!(wrong.review(store, "question", 7, cancel_rx).await.unwrap_err().contains("리뷰 ID"));
    }

    #[tokio::test]
    async fn reviewer_timeout_remains_pending_until_caller_deadline() {
        let (_dir, store) = store();
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let waiting = MockAgent::new(ReviewOutcome::Wait);
        assert!(tokio::time::timeout(Duration::from_millis(20), waiting.review(store, "question", 7, cancel_rx)).await.is_err());
    }

    #[tokio::test]
    async fn reviewer_cancellation_interrupts_pending_and_rejects_pre_cancelled_request() {
        let (_dir, store) = store();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let waiting = Arc::new(MockAgent::new(ReviewOutcome::Wait));
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

    fn apply(streams: &mut crate::stream::Streams, store: &Store, event: &Value, session: &str, turn: &str) -> bool {
        crate::engine::apply_stream_event(streams, store, event, session, turn, session).unwrap()
    }

    #[tokio::test]
    async fn scripted_public_answer_reaches_the_stream_and_a_late_turn_is_dropped() {
        let (_dir, store) = store();
        let mock = MockAgent::with_public_answer(ReviewOutcome::Success);
        let saved = store.session("default-session").unwrap();
        let settings = ModelSettings { model: None, reasoning_effort: None };
        let mut lead = mock.connect(store.clone()).await.unwrap();
        lead.open_or_resume(&saved).await.unwrap();
        let mut streams = crate::stream::Streams::default();

        lead.begin_turn("review this", &settings).await.unwrap();
        let started = lead.next_event().await.unwrap();
        assert_eq!(started["method"], "item/started");
        assert!(!apply(&mut streams, &store, &started, &saved.id, "turn-1"));
        let delta = lead.next_event().await.unwrap();
        assert_eq!(delta["method"], "item/agentMessage/delta");
        assert!(apply(&mut streams, &store, &delta, &saved.id, "turn-1"));
        let call = lead.next_event().await.unwrap();
        assert_eq!(call["params"]["tool"], "request_arch_review");
        // A tool call is not public answer text and must not create a message.
        assert!(!apply(&mut streams, &store, &call, &saved.id, "turn-1"));
        lead.reply_to_tool_call(&call, json!({"kind":"arch_review_delivery"}), true).await.unwrap();
        let tail = lead.next_event().await.unwrap();
        assert_eq!(tail["method"], "item/agentMessage/delta");
        apply(&mut streams, &store, &tail, &saved.id, "turn-1");
        let done = lead.next_event().await.unwrap();
        assert_eq!(done["method"], "item/completed");
        assert!(apply(&mut streams, &store, &done, &saved.id, "turn-1"));
        assert_eq!(lead.next_event().await.unwrap()["method"], "turn/completed");

        let messages = store.snapshot().unwrap().messages;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "mock public answer");
        assert_eq!((messages[0].channel.as_str(), messages[0].delivery.as_str()), ("main", "delivered"));

        // The second turn is current now; a late event from the finished turn is
        // dropped instead of being appended to the new answer.
        lead.begin_turn("second", &settings).await.unwrap();
        let started2 = lead.next_event().await.unwrap();
        let delta2 = lead.next_event().await.unwrap();
        let call2 = lead.next_event().await.unwrap();
        assert_eq!(call2["params"]["turnId"], "turn-2");
        let late_delta = json!({"method":"item/agentMessage/delta","params":{"threadId":saved.id,"turnId":"turn-1","itemId":"mock-item-late","delta":"late text"}});
        let late_done = json!({"method":"item/completed","params":{"threadId":saved.id,"turnId":"turn-1","item":{"id":"mock-item-late","type":"agentMessage","phase":"final_answer","text":"late text"}}});
        assert!(!apply(&mut streams, &store, &late_delta, &saved.id, "turn-2"));
        assert!(!apply(&mut streams, &store, &late_done, &saved.id, "turn-2"));
        let messages = store.snapshot().unwrap().messages;
        assert_eq!(messages.len(), 1);
        assert!(messages.iter().all(|message| !message.content.contains("late")));

        assert!(!apply(&mut streams, &store, &started2, &saved.id, "turn-2"));
        assert!(apply(&mut streams, &store, &delta2, &saved.id, "turn-2"));
        let messages = store.snapshot().unwrap().messages;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "mock public ");
        assert_eq!((messages[1].channel.as_str(), messages[1].delivery.as_str()), ("main", "streaming"));
    }
}
