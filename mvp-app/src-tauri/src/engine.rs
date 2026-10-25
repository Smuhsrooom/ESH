use crate::{
    agents::{registered, AgentRole, CodexLeadSession, LeadAdapter, LeadSession, ReviewAdapter, ANTIGRAVITY, CODEX},
    store::{err, ActiveRunSnapshot, Message, Result, Route, Snapshot, Store},
};
use serde_json::{json, Value};
use crate::models::{CodexModel, ModelSettings};
use std::{
    future::Future,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

// The worker has already taken the next input from its queue when this runs.
// Dropping that input after closing the previous connection loses the turn.
async fn switch_connection<C, F, Fut>(
    connection: &mut Option<C>,
    owner: &mut Option<String>,
    next_session: &str,
    close: F,
) -> Result<()>
where
    F: FnOnce(C) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    if owner.as_deref() == Some(next_session) && connection.is_some() {
        return Ok(());
    }
    let previous = connection.take();
    *owner = None;
    if let Some(previous) = previous {
        close(previous).await?;
    }
    Ok(())
}
/// Admission control for work that outlives the current call: worker jobs,
/// SQLite writes that shutdown later checkpoints, and short-lived CLI processes.
/// `shutdown` closes this before it stops the worker, so every command that
/// starts new work has to ask first instead of relying on `submit` alone.
pub(crate) fn ensure_accepting(accepting: &AtomicBool) -> Result<()> {
    if accepting.load(Ordering::SeqCst) {
        Ok(())
    } else {
        Err("앱을 종료 중이어서 새 작업을 받을 수 없습니다".into())
    }
}
/// Builds the Blocking Delegation payload that goes back on the original open
/// tool call. `archReview` and `latestUserRequirements` stay separate keys so
/// Codex can apply the user's newest requirements first, and `reviewIsStale`
/// compares the version captured when the review started with the version the
/// delivered requirements belong to. Both versions come from one store read, so
/// the flag can never disagree with the `messages` list next to it.
fn review_envelope(
    review: Value,
    review_version: i64,
    requirements_version: i64,
    pending: Vec<Message>,
) -> Value {
    json!({"kind":"arch_review_delivery","archReview":review,
        "latestUserRequirements":{"conversationVersion":requirements_version,"messages":pending},
        "reviewIsStale":review_version!=requirements_version,
        "processingOrder":["latestUserRequirements","archReview"]})
}
fn interrupt_consumed_input(store: &Store, id: &str) -> Result<bool> {
    store.interrupt_queued(id)
}
/// Commits a finished OpenCode turn. The local turn binding is confirmed first:
/// when the user cancels while the CLI is answering, the message is no longer
/// `queued`, and the turn must not move `chat_sessions.thread_id` to an external
/// session whose answer was never stored locally. Otherwise the next turn would
/// resume a conversation that local history cannot show. The successful path
/// writes the same rows as before.
fn commit_opencode_answer(
    store: &Store,
    session_id: &str,
    user_id: &str,
    answer: &crate::opencode::Answer,
) -> Result<()> {
    let turn = uuid::Uuid::new_v4().to_string();
    store.confirm_turn_start(user_id, &answer.session_id, &turn)?;
    store.bind_codex_session(session_id, &answer.session_id, &answer.session_id)?;
    store.routed_message(
        "opencode",
        &answer.text,
        false,
        Route {
            channel: "main",
            kind: "answer",
            thread_id: Some(answer.session_id.clone()),
            turn_id: Some(turn),
            chat_session_id: Some(session_id.to_string()),
            ..Route::default()
        },
    )?;
    Ok(())
}
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch};

struct Input {
    message: Message,
    target: String,
    settings: ModelSettings,
}
/// The worker owns the only Codex connection, so compaction is queued like a message.
enum Job {
    Message(Input),
    Compact(String),
}
enum CompactOutcome {
    Completed,
    CompletedUnconfirmed,
}
fn matches_compaction_item(event: &serde_json::Value, thread: &str, turn: &str, item: Option<&str>, method: &str) -> bool {
    let id = event["params"]["item"]["id"].as_str();
    event["method"] == method
        && event["params"]["threadId"] == thread
        && event["params"]["turnId"] == turn
        && event["params"]["item"]["type"] == "contextCompaction"
        && id.is_some()
        && (item.is_none() && method == "item/started" || item == id)
}
/// Applies one Codex event to the public-answer drafts and reports whether the
/// stored conversation changed, so the caller emits exactly once when it did.
/// Thread and turn have to match the running turn: a late event from a finished
/// turn is dropped instead of being written into the current answer, and only
/// `agentMessage` items reach the drafts (tool calls and compaction stay out).
pub(crate) fn apply_stream_event(
    streams: &mut crate::stream::Streams,
    store: &Store,
    event: &Value,
    thread: &str,
    turn: &str,
    session: &str,
) -> Result<bool> {
    let p = &event["params"];
    if p["threadId"] != thread || p["turnId"] != turn {
        return Ok(false);
    }
    match event["method"].as_str().unwrap_or_default() {
        "item/started" if p["item"]["type"] == "agentMessage" => {
            if let Some(id) = p["item"]["id"].as_str() {
                streams.start(id, p["item"]["phase"].as_str(), thread, turn, session);
            }
            Ok(false)
        }
        "item/agentMessage/delta" => match (p["itemId"].as_str(), p["delta"].as_str()) {
            (Some(id), Some(delta)) => streams.delta(store, id, delta),
            _ => Ok(false),
        },
        "item/completed" if p["item"]["type"] == "agentMessage" => match p["item"]["id"].as_str() {
            Some(id) => streams.complete(
                store,
                id,
                p["item"]["text"].as_str().unwrap_or_default(),
                p["item"]["phase"].as_str(),
                thread,
                turn,
                session,
            ),
            None => Ok(false),
        },
        _ => Ok(false),
    }
}
struct ActiveRun {
    snapshot: ActiveRunSnapshot,
    stop: watch::Sender<bool>,
}
pub struct Engine {
    pub store: Arc<Store>,
    input: mpsc::UnboundedSender<Input>,
    compact: mpsc::UnboundedSender<String>,
    stop: watch::Sender<bool>,
    done: watch::Receiver<bool>,
    accepting: AtomicBool,
    app: AppHandle,
    pub data_dir: PathBuf,
    models: Mutex<Vec<CodexModel>>,
    active: Mutex<Option<ActiveRun>>,
    streams: Mutex<crate::stream::Streams>,
}

impl Engine {
    pub fn start(app: AppHandle, data_dir: PathBuf) -> Result<Arc<Self>> {
        let store = Arc::new(Store::open(&data_dir.join("chat.sqlite"))?);
        let (input, rx) = mpsc::unbounded_channel();
        let (compact, compact_rx) = mpsc::unbounded_channel();
        let (stop, stopped) = watch::channel(false);
        let (done_tx, done) = watch::channel(false);
        let engine = Arc::new(Self {
            store,
            input,
            compact,
            stop,
            done,
            accepting: AtomicBool::new(true),
            app,
            data_dir,
            models: Mutex::new(Vec::new()),
            active: Mutex::new(None),
            streams: Mutex::new(crate::stream::Streams::default()),
        });
        let e = engine.clone();
        tauri::async_runtime::spawn(async move {
            e.clone().worker(rx, compact_rx, stopped).await;
            let _ = done_tx.send(true);
        });
        Ok(engine)
    }
    pub fn emit(&self) -> Result<()> {
        let snap = self.snapshot()?;
        self.app.emit("chat-updated", snap).map_err(err)
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let mut snapshot = self.store.snapshot()?;
        snapshot.active_run = self.active.lock().map_err(err)?.as_ref().map(|r| r.snapshot.clone());
        Ok(snapshot)
    }
    /// Shared by every Tauri command that starts new work (worker turn,
    /// compaction, session/room creation, external list/import, CLI process).
    pub fn ensure_accepting(&self) -> Result<()> {
        ensure_accepting(&self.accepting)
    }
    /// Lets command wrappers hand the admission flag to helpers that only need
    /// the flag and the store, so the guard order stays testable.
    pub fn admission(&self) -> &AtomicBool {
        &self.accepting
    }
    pub fn cancel(&self, session_id: &str, run_id: &str) -> Result<Snapshot> {
        let mut active = self.active.lock().map_err(err)?;
        let run = active.as_mut().ok_or("이미 실행이 끝났습니다")?;
        if run.snapshot.id != run_id || run.snapshot.session_id != session_id {
            return Err("현재 실행과 다른 중단 요청입니다. 상태를 다시 확인해 주세요.".into());
        }
        if !run.snapshot.cancelling {
            self.store.cancel_queued(session_id)?;
            self.store.audit("cancel_requested", json!({"runId":run_id,"sessionId":session_id}))?;
            run.snapshot.cancelling = true;
            let agent = self.store.session(session_id)?.agent_instance_id.unwrap_or("codex".into());
            if !self.store.status(&agent, "cancelling", "현재 실행을 중단하는 중")? {
                let _ = self.store.audit("agent_status_unmatched", json!({"agent":agent,"status":"cancelling"}));
            }
            let _ = run.stop.send(true);
        }
        drop(active);
        self.emit()?;
        self.snapshot()
    }
    pub fn cache_models(&self, models: Vec<CodexModel>) -> Result<()> {
        *self.models.lock().map_err(err)? = models;
        Ok(())
    }
    pub fn set_model_settings(&self, id: &str, settings: &ModelSettings) -> Result<Snapshot> {
        if !self.accepting.load(Ordering::SeqCst) { return Err("앱을 종료 중입니다".into()); }
        if self.store.session(id)?.provider != "codex" { return Err("Codex 세션이 아닙니다".into()); }
        crate::models::validate(&self.models.lock().map_err(err)?, settings)?;
        self.store.set_model_settings(id, settings)?;
        self.store.invalidate_context(id)?;
        self.emit()?;
        self.snapshot()
    }
    pub async fn list_opencode_models(&self, id: &str) -> Result<Vec<crate::opencode::OpenCodeModel>> {
        let saved = self.store.session(id)?;
        if saved.provider != "opencode" { return Err("OpenCode 세션이 아닙니다".into()); }
        let instance_id = saved.agent_instance_id.as_deref().ok_or("OpenCode 등록 ID가 없습니다")?;
        let instance = self.store.list_agent_instances()?.into_iter().find(|i| i.id == instance_id)
            .ok_or("OpenCode 등록을 찾을 수 없습니다")?;
        if instance.active_role != Some("lead") || instance.installation != "installed" {
            return Err("등록된 OpenCode CLI가 실행 가능하지 않습니다".into());
        }
        let path = instance.path.as_deref().ok_or("OpenCode 실행 파일 경로가 없습니다")?;
        crate::opencode::list_models(std::path::Path::new(path), &self.data_dir).await
    }
    pub async fn set_opencode_model(&self, id: &str, model: Option<String>) -> Result<Snapshot> {
        if !self.accepting.load(Ordering::SeqCst) { return Err("앱을 종료 중입니다".into()); }
        if let Some(ref selected) = model {
            let models = self.list_opencode_models(id).await?;
            if !models.iter().any(|item| &item.model == selected) {
                return Err("선택한 OpenCode 모델을 현재 목록에서 찾을 수 없습니다".into());
            }
        } else if self.store.session(id)?.provider != "opencode" {
            return Err("OpenCode 세션이 아닙니다".into());
        }
        self.store.set_model_settings(id, &ModelSettings { model, reasoning_effort: None })?;
        self.emit()?;
        self.snapshot()
    }
    fn state(&self, agent: &str, status: &str, detail: &str) -> Result<()> {
        if !self.store.status(agent, status, detail)? {
            // A session can still point at an instance that was unregistered
            // while it was running. Keep the turn alive but leave a trace
            // instead of reporting a status that was never stored.
            self.store
                .audit("agent_status_unmatched", json!({"agent":agent,"status":status}))?;
        }
        self.emit()
    }
    fn error(&self, agent: &str, e: &str) -> Result<()> {
        self.store
            .message("system", &format!("{agent}: {e}"), true)?;
        self.state(
            agent,
            if e.contains("unknown") {
                "unknown"
            } else {
                "failed"
            },
            e,
        )
    }
    pub fn submit(
        &self,
        text: String,
        target: String,
        session_id: Option<String>,
    ) -> Result<Snapshot> {
        if !self.accepting.load(Ordering::SeqCst) {
            return Err("앱을 종료 중이어서 새 메시지를 받을 수 없습니다".into());
        }
        if text.trim().is_empty() {
            return Err("메시지를 입력해 주세요".into());
        }
        if text.len() > 16000 {
            return Err("메시지는 UTF-8 기준 16KB 이하로 입력해 주세요".into());
        }
        let supported = registered().iter().any(|adapter| {
            let agent = adapter.descriptor();
            agent.id == target &&
                ((agent.roles.contains(&AgentRole::Lead) && agent.capabilities.lead_conversation) ||
                 (agent.roles.contains(&AgentRole::Reviewer) && agent.capabilities.delegated_review))
        });
        if !supported {
            return Err("지원하지 않는 수신 대상입니다".into());
        }
        let active = self.active.lock().map_err(err)?;
        if active.is_none() && self.store.snapshot()?.contexts.iter().any(|c| c.compaction == "compacting") {
            return Err("컨텍스트 압축 중입니다. 끝난 뒤에 메시지를 보내 주세요.".into());
        }
        if active.as_ref().is_some_and(|r| r.snapshot.cancelling && Some(r.snapshot.session_id.as_str()) == session_id.as_deref().or(Some("default-session"))) {
            return Err("이 세션을 중단 중입니다. 정리가 끝나면 다시 보내 주세요.".into());
        }
        let content = if target == "antigravity" {
            format!("@arch {}", text.trim())
        } else {
            text.trim().to_string()
        };
        let local_id = session_id.unwrap_or("default-session".into());
        let saved = self.store.session(&local_id)?;
        if (saved.provider == "opencode" && target != "opencode") || (saved.provider == "codex" && target == "opencode") {
            return Err("이 세션의 에이전트와 메시지 대상이 다릅니다".into());
        }
        let settings = ModelSettings { model: saved.model, reasoning_effort: saved.reasoning_effort };
        let message = self.store.routed_message(
            "user",
            &content,
            false,
            Route {
                channel: if target == "antigravity" {
                    "legacy"
                } else {
                    "main"
                },
                kind: "user_message",
                chat_session_id: Some(local_id),
                ..Route::default()
            },
        )?;
        self.input
            .send(Input { message, target, settings })
            .map_err(|_| "실행 작업자가 종료됐습니다. 앱을 다시 열어 주세요".to_string())?;
        drop(active);
        self.emit()?;
        self.snapshot()
    }
    /// Queues a user-requested context compaction for the selected local session.
    pub fn request_compact(&self, session_id: &str) -> Result<Snapshot> {
        if !self.accepting.load(Ordering::SeqCst) {
            return Err("앱을 종료 중이어서 압축을 시작할 수 없습니다".into());
        }
        let active = self.active.lock().map_err(err)?;
        if active.is_some() {
            return Err("Codex가 실행 중입니다. 끝난 뒤에 압축해 주세요.".into());
        }
        let snapshot = self.store.snapshot()?;
        if snapshot.busy_session_id.is_some() || snapshot.messages.iter().any(|m| m.delivery == "queued") {
            return Err("대기 중이거나 실행 중인 작업이 있습니다. 끝난 뒤에 압축해 주세요.".into());
        }
        let session = snapshot.sessions.iter().find(|s| s.id == session_id)
            .ok_or("압축할 세션을 찾을 수 없습니다")?;
        let thread = session.thread_id.as_deref().ok_or("이 세션은 아직 Codex 스레드가 없습니다. 먼저 메시지를 한 번 보내 주세요.")?;
        if snapshot.contexts.iter().any(|c| c.compaction == "compacting") {
            return Err("이미 컨텍스트를 압축하는 중입니다".into());
        }
        if !self.store.begin_compact(session_id, thread)? {
            return Err("이미 컨텍스트를 압축하는 중입니다".into());
        }
        if self.compact.send(session_id.to_string()).is_err() {
            self.store.finish_context(session_id, "failed")?;
            return Err("실행 작업자가 종료됐습니다. 앱을 다시 열어 주세요".into());
        }
        drop(active);
        self.emit()?;
        self.snapshot()
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.accepting.store(false, Ordering::SeqCst);
        self.store
            .audit("shutdown", json!({"phase":"block_new_work"}))?;
        let _ = self.stop.send(true);
        if let Some(run) = self.active.lock().map_err(err)?.as_ref() { let _ = run.stop.send(true); }
        let mut done = self.done.clone();
        if !*done.borrow() {
            tokio::time::timeout(Duration::from_secs(25), async {
                while !*done.borrow() {
                    done.changed().await.map_err(err)?;
                }
                Ok::<(), String>(())
            })
            .await
            .map_err(|_| "종료 확인 시간 초과: 실행 상태를 unknown으로 기록했습니다".to_string())??;
        }
        Ok(())
    }
    async fn worker(
        self: Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<Input>,
        mut compact_rx: mpsc::UnboundedReceiver<String>,
        mut stop: watch::Receiver<bool>,
    ) {
        let mut rpc: Option<CodexLeadSession> = None;
        let mut rpc_owner: Option<String> = None;
        let mut connection_uncertain = false;
        loop {
            let job = tokio::select! { biased; _=stop.changed()=>break, value=rx.recv()=>match value{Some(v)=>Job::Message(v),None=>break}, value=compact_rx.recv()=>match value{Some(v)=>Job::Compact(v),None=>break} };
            if *stop.borrow() {
                break;
            }
            let input = match job {
                Job::Compact(session) => {
                    if connection_uncertain {
                        let _ = self.store.finish_context(&session, "unknown");
                        let _ = self.store.audit("compact_failed", json!({"sessionId":session,"error":"이전 Codex 연결 종료 확인 불가 (unknown)"}));
                        let _ = self.emit();
                        continue;
                    }
                    self.run_compact(&session, &mut rpc, &mut rpc_owner, stop.clone()).await;
                    continue;
                }
                Job::Message(input) => input,
            };
            // User messages consumed during an Arch wait must not start another turn.
            let queued = self
                .store
                .snapshot()
                .map(|s| {
                    s.messages
                        .iter()
                        .any(|m| m.id == input.message.id && m.delivery == "queued")
                })
                .unwrap_or(false);
            if !queued {
                continue;
            }
            if connection_uncertain {
                let _ = interrupt_consumed_input(&self.store, &input.message.id);
                let _ = self.emit();
                continue;
            }
            let agent_key = self.store.session(&input.message.chat_session_id).ok()
                .and_then(|s| s.agent_instance_id).unwrap_or("codex".into());
            let (run_stop, run_stopped) = watch::channel(false);
            let run_id = uuid::Uuid::new_v4().to_string();
            *self.active.lock().unwrap() = Some(ActiveRun {
                snapshot: ActiveRunSnapshot { id: run_id.clone(), session_id: input.message.chat_session_id.clone(), cancelling: false },
                stop: run_stop,
            });
            let _ = self
                .store
                .set_busy_session(Some(input.message.chat_session_id.clone()));
            let _ = self.emit();
            if *stop.borrow() {
                if let Some(run) = self.active.lock().unwrap().as_ref() { let _ = run.stop.send(true); }
            }
            let transition = switch_connection(
                &mut rpc,
                &mut rpc_owner,
                &input.message.chat_session_id,
                |mut old| async move { LeadSession::close(&mut old).await },
            ).await.map_err(|e| format!("연결 전환 종료 확인 불가 (unknown): {e}"));
            if transition.is_err() { connection_uncertain = true; }
            let _ = self.store.set_execution_session(&input.message.chat_session_id);
            let result = if let Err(e) = transition {
                Err(e)
            } else if *run_stopped.borrow() {
                Err("실행 중단".into())
            } else if input.target == "antigravity" {
                let _ = self.state("antigravity", "arch_running", "질문에 답하는 중");
                let _ = self
                    .store
                    .delivery(&[input.message.id.clone()], "delivered");
                let _ = self.emit();
                match ANTIGRAVITY.review(
                    self.store.clone(),
                    &input.message.content,
                    input.message.version,
                    run_stopped.clone(),
                )
                .await
                {
                    Ok(review) => {
                        let answer = review["answer"].as_str().unwrap_or_default();
                        self.store
                            .message("antigravity", answer, false)
                            .and_then(|_| self.state("antigravity", "completed", "답변 완료"))
                    }
                    Err(e) => Err(e),
                }
            } else if input.target == "opencode" {
                self.opencode_turn(&input.message, run_stopped.clone()).await
            } else {
                let result = self.codex_turn(&mut rpc, &input.message, &input.settings, run_stopped.clone()).await;
                if rpc.is_some() { rpc_owner = Some(input.message.chat_session_id.clone()); }
                result
            };
            if self.streams.lock().unwrap().interrupt(&self.store).unwrap_or(0) > 0 { let _ = self.emit(); }
            if *stop.borrow() { break; }
            if *run_stopped.borrow() {
                let cancelled = match rpc.as_mut() {
                    Some(connection) => LeadSession::cancel_turn(connection).await,
                    None => Ok("cancelled".to_string()),
                };
                let arch_unknown = result.as_ref().err().is_some_and(|e| e.contains("unknown"));
                let (status, detail) = match cancelled {
                    Ok(status) if !arch_unknown => (status, "사용자가 실행을 중단했습니다".to_string()),
                    other => {
                        let detail = format!("중단 결과 확인 불가 (unknown): {:?}", other);
                        if let Some(mut connection) = rpc.take() { let _ = LeadSession::close(&mut connection).await; }
                        let _ = self.store.message("system", &detail, true);
                        ("unknown".into(), detail)
                    }
                };
                let _ = self.store.status(&agent_key, &status, &detail);
                if let Ok(s) = self.store.snapshot() {
                    if s.agents.iter().any(|a| a.id == "antigravity" && a.status == "arch_running") {
                        let _ = self.store.status("antigravity", "cancelled", "사용자가 Arch 검토를 중단했습니다");
                    }
                }
                let _ = self.store.audit("cancel_finished", json!({"runId":run_id,"sessionId":input.message.chat_session_id,"status":status}));
            } else {
            if let Err(e) = result {
                if *stop.borrow() {
                    break;
                }
                // A failed close, reconnect, or start has consumed this queue entry.
                // Keep it in SQLite as a recoverable, non-replayed input.
                let _ = interrupt_consumed_input(&self.store, &input.message.id);
                if e.contains("연결 정리 확인 불가 (unknown)") { connection_uncertain = true; }
                let _ = self.error(if connection_uncertain { "codex" } else if input.target == "opencode" { &agent_key } else { &input.target }, &e);
                if input.target == "codex" {
                    if let Some(mut old) = rpc.take() {
                        if let Err(close_error) = LeadSession::close(&mut old).await {
                            connection_uncertain = true;
                            let _ = self.error("codex", &format!("연결 정리 확인 불가 (unknown): {close_error}"));
                        }
                    }
                    rpc_owner = None;
                }
            }
            }
            *self.active.lock().unwrap() = None;
            let status = self.store.snapshot().ok().and_then(|s| s.agents.into_iter().find(|a| a.id == "codex")).map(|a| a.status).unwrap_or("unknown".into());
            let _ = self.store.finish_context(&input.message.chat_session_id, &status);
            let _ = self.store.set_busy_session(None);
            let _ = self.emit();
        }
        let _ = self.store.audit(
            "shutdown",
            json!({"phase":"arch_cancel_confirmed_or_recorded"}),
        );
        let _ = self
            .store
            .audit("shutdown", json!({"phase":"resolve_open_codex_tools"}));
        let exit = if let Some(mut rpc) = rpc {
            LeadSession::close(&mut rpc).await
        } else {
            Ok(())
        };
        let _ = self.store.audit(
            "shutdown",
            json!({"phase":"codex_exit","confirmed":exit.is_ok()}),
        );
        for agent in ["codex", "antigravity"] {
            let status = self
                .store
                .snapshot()
                .ok()
                .and_then(|s| s.agents.into_iter().find(|a| a.id == agent))
                .map(|a| a.status)
                .unwrap_or_default();
            if !["idle", "completed", "failed", "cancelled", "unknown"].contains(&status.as_str()) {
                let _ = self.store.status(
                    agent,
                    if exit.is_ok() { "cancelled" } else { "unknown" },
                    "앱 종료로 실행을 정리했습니다",
                );
            }
        }
        if let Ok(snapshot) = self.store.snapshot() {
            for session in snapshot.sessions { let _ = self.store.finish_context(&session.id, "unknown"); }
        }
        let _ = self
            .store
            .audit("shutdown", json!({"phase":"sqlite_finalize"}));
        let _ = self.store.checkpoint();
        let _ = self.store.set_busy_session(None);
        *self.active.lock().unwrap() = None;
        let _ = self.emit();
    }
    async fn run_compact(
        &self,
        session: &str,
        rpc: &mut Option<CodexLeadSession>,
        rpc_owner: &mut Option<String>,
        stop: watch::Receiver<bool>,
    ) {
        let _ = self.store.set_execution_session(session);
        let result = self.compact_session(session, rpc, rpc_owner, stop).await;
        match result {
            Ok(outcome) => {
                if matches!(outcome, CompactOutcome::CompletedUnconfirmed) {
                    if let Some(mut connection) = rpc.take() { let _ = LeadSession::close(&mut connection).await; }
                    *rpc_owner = None;
                }
                let _ = self.store.audit("compact_completed", json!({"sessionId":session}));
                let _ = self.state("codex", "idle", "컨텍스트 압축 완료");
            }
            Err(e) => {
                let uncertain = e.contains("unknown") || e.contains("앱 종료") || e.contains("연결이 종료") || e.contains("응답 시간 초과");
                let _ = self.store.finish_context(session, if uncertain { "unknown" } else { "failed" });
                let _ = self.store.audit("compact_failed", json!({"sessionId":session,"error":e}));
                if uncertain {
                    let _ = self.store.message("system", &format!("codex: {e}"), true);
                    let _ = self.state("codex", "unknown", &e);
                } else {
                    let _ = self.error("codex", &e);
                }
                if let Some(mut connection) = rpc.take() { let _ = LeadSession::close(&mut connection).await; }
                *rpc_owner = None;
            }
        }
        let _ = self.store.set_busy_session(None);
        let _ = self.emit();
    }
    /// Sends thread/compact/start and waits for the matching compaction item to complete.
    async fn compact_session(
        &self,
        session: &str,
        rpc: &mut Option<CodexLeadSession>,
        rpc_owner: &mut Option<String>,
        mut stop: watch::Receiver<bool>,
    ) -> Result<CompactOutcome> {
        if *stop.borrow() { return Err("앱 종료".into()); }
        let saved = self.store.session(session)?;
        let thread = saved.thread_id.clone().ok_or("이 세션은 아직 Codex 스레드가 없습니다. 먼저 메시지를 한 번 보내 주세요.")?;
        self.state("codex", "compacting", "컨텍스트를 압축하는 중")?;
        if rpc_owner.as_deref() != Some(session) {
            if let Some(mut old) = rpc.take() { LeadSession::close(&mut old).await?; }
            *rpc_owner = None;
            let mut connection = tokio::select! { _ = stop.changed() => return Err("앱 종료".into()), result = CODEX.connect(self.store.clone()) => result? };
            if let Err(e) = tokio::select! { _ = stop.changed() => Err("앱 종료".to_string()), result = connection.open_or_resume(&saved) => result } {
                let _ = LeadSession::close(&mut connection).await;
                return Err(e);
            }
            *rpc = Some(connection);
            *rpc_owner = Some(session.to_string());
        }
        let connection = rpc.as_mut().ok_or("Codex 연결이 없습니다")?;
        if connection.thread_id != thread {
            return Err("세션의 Codex 스레드와 현재 연결이 달라 압축을 시작할 수 없습니다".into());
        }
        self.store.set_busy_session(Some(session.into()))?;
        self.store.audit("compact_requested", json!({"sessionId":session,"threadId":thread}))?;
        self.emit()?;
        if let Err(e) = tokio::select! { _ = stop.changed() => Err("앱 종료".into()), result = connection.start_compact() => result } {
            return Err(if e.contains("Codex thread/compact/start 오류") { e } else { format!("압축 시작 여부 확인 불가 (unknown): {e}") });
        }
        let mut item_id: Option<String> = None;
        let mut compact_turn: Option<String> = None;
        let mut compacted = false;
        let mut completion_event: Option<serde_json::Value> = None;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
        let mut close_deadline: Option<tokio::time::Instant> = None;
        loop {
            let now = tokio::time::Instant::now();
            let until = close_deadline.unwrap_or(deadline);
            let remaining = until.saturating_duration_since(now);
            if remaining.is_zero() {
                if compacted {
                    self.store.observe_context(session, &thread, completion_event.as_ref().ok_or("압축 완료 이벤트가 없습니다")?)?;
                    self.store.audit("compact_turn_close_unconfirmed", json!({"sessionId":session,"turnId":compact_turn}))?;
                    return Ok(CompactOutcome::CompletedUnconfirmed);
                }
                return Err("컨텍스트 압축 완료를 확인하지 못했습니다 (unknown)".into());
            }
            let waited = tokio::select! {
                _ = stop.changed() => return Err("앱 종료".into()),
                result = tokio::time::timeout(remaining, connection.next_unobserved()) => result,
            };
            let event = match waited {
                Ok(Ok(value)) => value,
                Ok(Err(e)) => {
                    if compacted {
                        self.store.observe_context(session, &thread, completion_event.as_ref().ok_or("압축 완료 이벤트가 없습니다")?)?;
                        self.store.audit("compact_turn_close_unconfirmed", json!({"sessionId":session,"turnId":compact_turn,"error":e}))?;
                        return Ok(CompactOutcome::CompletedUnconfirmed);
                    }
                    return Err(format!("압축 연결 상태 확인 불가 (unknown): {e}"));
                }
                Err(_) => {
                    if compacted {
                        self.store.observe_context(session, &thread, completion_event.as_ref().ok_or("압축 완료 이벤트가 없습니다")?)?;
                        self.store.audit("compact_turn_close_unconfirmed", json!({"sessionId":session,"turnId":compact_turn}))?;
                        return Ok(CompactOutcome::CompletedUnconfirmed);
                    }
                    return Err("컨텍스트 압축 완료를 확인하지 못했습니다 (unknown)".into());
                }
            };
            let p = &event["params"];
            if p["threadId"] != json!(connection.thread_id) { continue; }
            if event["method"] == "error" && p["willRetry"].as_bool() != Some(true) {
                return Err(format!("Codex 압축 오류: {p}"));
            }
            if event["method"] == "turn/started" && compact_turn.is_none() {
                compact_turn = p["turn"]["id"].as_str().map(str::to_string);
                continue;
            }
            if event["method"] == "turn/completed" && compact_turn.as_deref() == p["turn"]["id"].as_str() {
                let status = p["turn"]["status"].as_str().unwrap_or("unknown");
                self.store.audit("compact_turn_completed", json!({"sessionId":session,"turnId":compact_turn,"status":status}))?;
                if !compacted {
                    return Err(format!("Codex 압축 턴이 완료되지 않았습니다: {status}"));
                }
                if status != "completed" {
                    return Err(format!("Codex 압축 턴이 실패했습니다: {status}"));
                }
                self.store.observe_context(session, &thread, completion_event.as_ref().ok_or("압축 완료 이벤트가 없습니다")?)?;
                // The compaction item already decided the outcome; this only releases the turn slot.
                return Ok(CompactOutcome::Completed);
            }
            let Some(turn) = compact_turn.as_deref() else { continue; };
            if event["method"] == "thread/tokenUsage/updated" && p["turnId"] == turn {
                self.store.observe_context(session, &thread, &event)?;
            } else if matches_compaction_item(&event, &thread, turn, item_id.as_deref(), "item/started") {
                item_id = p["item"]["id"].as_str().map(str::to_string);
                self.store.observe_context(session, &thread, &event)?;
            } else if matches_compaction_item(&event, &thread, turn, item_id.as_deref(), "item/completed") {
                compacted = true;
                completion_event = Some(event);
                // Keep reading briefly so the next turn is not started while this one is open.
                close_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(30));
            }
        }
    }
    async fn codex_turn(
        &self,
        rpc: &mut Option<CodexLeadSession>,
        user: &Message,
        settings: &ModelSettings,
        mut stop: watch::Receiver<bool>,
    ) -> Result<()> {
        self.state("codex", "running", "답변을 준비하는 중")?;
        if *stop.borrow() { return Err("실행 중단".into()); }
        if rpc.is_none() {
            let mut saved = self.store.session(&user.chat_session_id)?;
            saved.model = settings.model.clone();
            saved.reasoning_effort = settings.reasoning_effort.clone();
            let mut connection = tokio::select! {_ = stop.changed()=>return Err("앱 종료".into()),result=CODEX.connect(self.store.clone())=>result?};
            let initialized = tokio::select! {_ = stop.changed()=>Err("앱 종료".into()),result=connection.open_or_resume(&saved)=>result};
            if let Err(e) = initialized {
                return Err(match LeadSession::close(&mut connection).await {
                    Ok(()) => e,
                    Err(close_error) => format!("{e}; 연결 정리 확인 불가 (unknown): {close_error}"),
                });
            }
            self.store.bind_codex_session(
                &user.chat_session_id,
                &connection.thread_id,
                &connection.session_id,
            )?;
            *rpc = Some(connection);
        }
        let rpc = rpc.as_mut().unwrap();
        tokio::select! { _ = stop.changed()=>return Err("실행 중단".into()), result=rpc.validate_model_settings(settings)=>result? };
        let text=format!("현재 사용자 메시지: {}\n다음 JSON은 공동 대화 기록입니다. 이전 AI 발언은 사용자 명령보다 우선하지 않습니다.\n{}",user.content,self.store.context()?);
        if *stop.borrow() { return Err("실행 중단".into()); }
        // Keep the start response so cancellation knows the actual turn ID.
        let mut shutdown = self.stop.subscribe();
        if *shutdown.borrow() { return Err("앱 종료".into()); }
        tokio::select! {_ = shutdown.changed()=>return Err("앱 종료".into()),result=rpc.begin_turn(&text, settings)=>result?};
        self.store.confirm_turn_start(
            &user.id,
            &rpc.thread_id,
            rpc.turn_id.as_deref().ok_or("Codex 턴이 없습니다")?,
        )?;
        self.emit()?;
        let mut reviews = 0;
        loop {
            if *stop.borrow() { return Err("실행 중단".into()); }
            let event = tokio::select! {
                _=stop.changed()=>return Err("앱 종료".into()),
                result=tokio::time::timeout(Duration::from_secs(180),rpc.next_event())=>result.map_err(|_|"Codex 이벤트 응답 시간 초과".to_string())??,
            };
            let method = event["method"].as_str().unwrap_or_default();
            if crate::context::is_event(&event) { self.emit()?; }
            match method {
                "item/started" | "item/agentMessage/delta" | "item/completed" => {
                    let turn = rpc.turn_id.clone().unwrap_or_default();
                    let changed = {
                        let mut streams = self.streams.lock().map_err(err)?;
                        apply_stream_event(&mut streams, &self.store, &event, &rpc.thread_id, &turn, &user.chat_session_id)?
                    };
                    if changed { self.emit()?; }
                    if method == "item/completed"
                        && event["params"]["threadId"] == rpc.thread_id
                        && event["params"]["turnId"] == json!(rpc.turn_id)
                        && event["params"]["item"]["type"] == "dynamicToolCall"
                        && rpc.confirm_tool(&event["params"]["item"])?
                    {
                        self.state("codex", "running", "검토 결과를 반영하는 중")?;
                    }
                }
                "item/tool/call" => {
                    if event["params"]["threadId"] != rpc.thread_id
                        || event["params"]["turnId"] != json!(rpc.turn_id)
                    {
                        return Err("현재 턴과 다른 도구 호출을 수신했습니다".into());
                    }
                    rpc.open_call = Some(event.clone());
                    if event["params"]["tool"] != "request_arch_review" {
                        return Err("지원하지 않는 동적 도구 요청입니다".into());
                    }
                    reviews += 1;
                    let review_version = self.store.session_version(&user.chat_session_id)?;
                    self.state("codex", "waiting_for_arch", "Arch 검토를 기다리는 중")?;
                    let question = event["params"]["arguments"]["question"]
                        .as_str()
                        .ok_or("리뷰 질문이 없습니다")?;
                    let call_id = event["params"]["callId"]
                        .as_str()
                        .ok_or("리뷰 callId가 없습니다")?
                        .to_string();
                    self.store.routed_message(
                        "codex",
                        question,
                        false,
                        Route {
                            channel: "collaboration",
                            kind: if reviews == 1 {
                                "review_request"
                            } else {
                                "review_followup"
                            },
                            thread_id: Some(rpc.thread_id.clone()),
                            turn_id: rpc.turn_id.clone(),
                            collaboration_id: Some(call_id.clone()),
                            chat_session_id: None,
                        },
                    )?;
                    self.emit()?;
                    let review = if reviews > 3 {
                        Err("이번 턴의 자동 리뷰 한도 3회에 도달했습니다".into())
                    } else {
                        self.state("antigravity", "arch_running", "Codex 요청을 검토하는 중")?;
                        ANTIGRAVITY.review(self.store.clone(), question, review_version, stop.clone()).await
                    };
                    if *stop.borrow() {
                        return Err(review.err().filter(|e| e.contains("unknown")).unwrap_or("실행 중단".into()));
                    }
                    let (review, success) = match review {
                        Ok(value) => {
                            self.store.routed_message(
                                "antigravity",
                                value["answer"].as_str().unwrap_or_default(),
                                false,
                                Route {
                                    channel: "collaboration",
                                    kind: "review_reply",
                                    thread_id: Some(rpc.thread_id.clone()),
                                    turn_id: rpc.turn_id.clone(),
                                    collaboration_id: Some(call_id),
                                    chat_session_id: None,
                                },
                            )?;
                            self.state("antigravity", "completed", "검토 완료")?;
                            (value, true)
                        }
                        Err(e) => {
                            self.error("antigravity", &e)?;
                            (
                                json!({"status":if e.contains("unknown"){"unknown"}else if e.contains("시간 초과"){"timed_out"}else{"failed"},"conversationVersion":review_version,"error":e}),
                                false,
                            )
                        }
                    };
                    let (requirements_version, pending) = self.store.pending_requirements(&user.chat_session_id)?;
                    let ids = pending.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
                    let envelope = review_envelope(review, review_version, requirements_version, pending);
                    self.state("codex", "resuming", "최신 요구사항과 리뷰를 전달하는 중")?;
                    rpc.reply_to_tool_call(&event, envelope, success).await?;
                    self.store.deliver_requirements(
                        &ids,
                        &rpc.thread_id,
                        rpc.turn_id.as_deref().ok_or("Codex 턴이 없습니다")?,
                    )?;
                    self.emit()?;
                }
                "turn/completed" => {
                    if event["params"]["turn"]["id"] != json!(rpc.turn_id) {
                        continue;
                    }
                    let turn = &event["params"]["turn"];
                    rpc.turn_id = None;
                    if turn["status"] != "completed" {
                        return Err(format!(
                            "Codex 턴이 완료되지 않았습니다: {} {}",
                            turn["status"], turn["error"]
                        ));
                    }
                    self.store.audit(
                        "codex_turn_completed",
                        json!({"threadId":rpc.thread_id,"turnId":turn["id"]}),
                    )?;
                    self.state("codex", "completed", "답변 완료")?;
                    return Ok(());
                }
                "error" => {
                    if event["params"]["willRetry"].as_bool() != Some(true) {
                        return Err(format!("Codex 실행 오류: {}", event["params"]));
                    }
                }
                _ => {
                    if event.get("id").is_some() && event.get("method").is_some() {
                        rpc.send(json!({"id":event["id"],"error":{"code":-32601,"message":"대화 전용 MVP에서는 이 도구를 허용하지 않습니다"}})).await?;
                    }
                }
            }
        }
    }

    async fn opencode_turn(&self, user: &Message, stop: watch::Receiver<bool>) -> Result<()> {
        let saved = self.store.session(&user.chat_session_id)?;
        let id = saved.agent_instance_id.as_deref().ok_or("OpenCode 등록 ID가 없습니다")?;
        let instance = self.store.list_agent_instances()?.into_iter().find(|i| i.id == id)
            .ok_or("OpenCode 등록을 찾을 수 없습니다")?;
        if instance.active_role != Some("lead") || instance.installation != "installed" {
            return Err("등록된 OpenCode CLI가 실행 가능하지 않습니다".into());
        }
        let path = instance.path.as_deref().ok_or("OpenCode 실행 파일 경로가 없습니다")?;
        self.state(id, "running", "OpenCode 답변을 준비하는 중")?;
        let answer = crate::opencode::run(
            std::path::Path::new(path), &self.data_dir, saved.thread_id.as_deref(), saved.model.as_deref(), &user.content, stop.clone(),
        ).await?;
        if *stop.borrow() { return Err("OpenCode 실행 중단".into()); }
        commit_opencode_answer(&self.store, &saved.id, &user.id, &answer)?;
        self.state(id, "completed", "답변 완료")?;
        Ok(())
    }
}

#[cfg(test)]
mod compact_event_tests {
    use super::matches_compaction_item;
    use serde_json::json;

    #[test]
    fn manual_compaction_accepts_only_its_started_item_and_completion() {
        let started = json!({"method":"item/started","params":{"threadId":"thread-a","turnId":"turn-a","item":{"id":"item-a","type":"contextCompaction"}}});
        let completed = json!({"method":"item/completed","params":{"threadId":"thread-a","turnId":"turn-a","item":{"id":"item-a","type":"contextCompaction"}}});
        assert!(matches_compaction_item(&started, "thread-a", "turn-a", None, "item/started"));
        assert!(matches_compaction_item(&completed, "thread-a", "turn-a", Some("item-a"), "item/completed"));
        assert!(!matches_compaction_item(&completed, "thread-a", "turn-a", None, "item/completed"));
        assert!(!matches_compaction_item(&completed, "thread-b", "turn-a", Some("item-a"), "item/completed"));
        assert!(!matches_compaction_item(&completed, "thread-a", "turn-b", Some("item-a"), "item/completed"));
        assert!(!matches_compaction_item(&completed, "thread-a", "turn-a", Some("item-b"), "item/completed"));
    }
}

#[cfg(test)]
mod envelope_tests {
    use super::review_envelope;
    use serde_json::json;

    #[test]
    fn review_and_latest_requirements_stay_separate_with_an_explicit_staleness_flag() {
        // Nothing arrived while Arch was reviewing: the review is current and the
        // requirements field is still present, empty, with its own version.
        let fresh = review_envelope(json!({"status":"success","answer":"review"}), 3, 3, vec![]);
        assert_eq!(fresh["kind"], "arch_review_delivery");
        assert_eq!(fresh["archReview"]["answer"], "review");
        assert_eq!(fresh["latestUserRequirements"]["conversationVersion"], 3);
        assert_eq!(fresh["latestUserRequirements"]["messages"], json!([]));
        assert_eq!(fresh["reviewIsStale"], false);
        assert_eq!(fresh["processingOrder"], json!(["latestUserRequirements", "archReview"]));

        // A message that arrived during the review makes it stale and travels in
        // its own field: it must not be merged into archReview, and the review
        // result itself must not be reported as a fresh verification.
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::Store::open(&dir.path().join("envelope.sqlite")).unwrap();
        let late = store.message("user", "new requirement", false).unwrap();
        let stale = review_envelope(json!({"status":"success","answer":"old review"}), 2, 3, vec![late]);
        assert_eq!(stale["reviewIsStale"], true);
        assert_eq!(stale["latestUserRequirements"]["conversationVersion"], 3);
        assert_eq!(stale["latestUserRequirements"]["messages"][0]["content"], "new requirement");
        assert_eq!(stale["latestUserRequirements"]["messages"][0]["delivery"], "queued");
        assert_eq!(stale["archReview"]["answer"], "old review");
        assert!(stale["archReview"].get("messages").is_none());
        assert!(stale["archReview"].get("latestUserRequirements").is_none());
    }
}

#[cfg(test)]
mod switch_tests {
    use super::{interrupt_consumed_input, switch_connection};
    use crate::store::{Route, Store};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn a_to_b_to_a_closes_each_previous_connection_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("queue.sqlite")).unwrap();
        let b = store.create_session("default", "B").unwrap();
        let first_b = store.routed_message("user", "B first", false, Route {
            channel: "main", kind: "user_message", chat_session_id: Some(b.id.clone()), ..Route::default()
        }).unwrap();
        let second_b = store.routed_message("user", "B second", false, Route {
            channel: "main", kind: "user_message", chat_session_id: Some(b.id.clone()), ..Route::default()
        }).unwrap();
        let next_a = store.message("user", "A again", false).unwrap();
        let closed = Arc::new(Mutex::new(Vec::new()));
        let mut connection = Some("connection-a".to_string());
        let mut owner = Some("default-session".to_string());
        let events = closed.clone();
        switch_connection(&mut connection, &mut owner, &b.id, move |old| async move {
            events.lock().unwrap().push(old);
            Ok(())
        }).await.unwrap();
        assert!(connection.is_none());
        assert!(owner.is_none());
        connection = Some("connection-b".into()); owner = Some(b.id.clone());
        store.confirm_turn_start(&first_b.id, "thread-b", "turn-b1").unwrap();
        switch_connection(&mut connection, &mut owner, &b.id, |_old| async {
            panic!("same-session input must not close the connection")
        }).await.unwrap();
        store.confirm_turn_start(&second_b.id, "thread-b", "turn-b2").unwrap();
        let events = closed.clone();
        switch_connection(&mut connection, &mut owner, "default-session", move |old| async move {
            events.lock().unwrap().push(old);
            Ok(())
        }).await.unwrap();
        store.confirm_turn_start(&next_a.id, "thread-a", "turn-a2").unwrap();
        assert_eq!(*closed.lock().unwrap(), ["connection-a", "connection-b"]);
        assert!(connection.is_none());
        assert!(owner.is_none());
        assert_eq!(store.snapshot().unwrap().messages.iter().map(|m| (m.content.as_str(), m.delivery.as_str())).collect::<Vec<_>>(), [
            ("B first", "delivered"), ("B second", "delivered"), ("A again", "delivered")
        ]);
    }

    #[tokio::test]
    async fn close_failure_marks_consumed_input_interrupted_without_replay() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("switch.sqlite")).unwrap();
        let b = store.create_session("default", "B").unwrap();
        let input = store.routed_message("user", "send to B", false, Route {
            channel: "main", kind: "user_message", chat_session_id: Some(b.id), ..Route::default()
        }).unwrap();
        let mut connection = Some("connection-a");
        let mut owner = Some("A".into());
        assert!(switch_connection(&mut connection, &mut owner, "B", |_old| async {
            Err("close failed".into())
        }).await.is_err());
        assert!(connection.is_none() && owner.is_none());
        assert!(interrupt_consumed_input(&store, &input.id).unwrap());
        assert!(!interrupt_consumed_input(&store, &input.id).unwrap());
        assert!(store.confirm_turn_start(&input.id, "thread-b", "turn-b").is_err());
        let saved = store.snapshot().unwrap().messages.remove(0);
        assert_eq!((saved.delivery.as_str(), saved.turn_id), ("interrupted", None));
    }

    #[tokio::test]
    async fn cancellation_during_close_keeps_new_turn_unstarted() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("cancel-switch.sqlite")).unwrap();
        let b = store.create_session("default", "B").unwrap();
        let input = store.routed_message("user", "cancel B", false, Route {
            channel: "main", kind: "user_message", chat_session_id: Some(b.id.clone()), ..Route::default()
        }).unwrap();
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let mut connection = Some("connection-a");
        let mut owner = Some("A".into());
        switch_connection(&mut connection, &mut owner, &b.id, |_old| async {
            store.cancel_queued(&b.id)?;
            stop.send(true).unwrap();
            Ok(())
        }).await.unwrap();
        assert!(*stopped.borrow());
        assert!(!interrupt_consumed_input(&store, &input.id).unwrap());
        assert!(store.confirm_turn_start(&input.id, "thread-b", "turn-b").is_err());
        let saved = store.snapshot().unwrap().messages.remove(0);
        assert_eq!((saved.delivery.as_str(), saved.turn_id), ("cancelled", None));
    }

    #[tokio::test]
    async fn reconnect_failure_leaves_consumed_message_recoverable() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("reconnect.sqlite")).unwrap();
        let b = store.create_session("default", "B").unwrap();
        let input = store.routed_message("user", "retry manually", false, Route {
            channel: "main", kind: "user_message", chat_session_id: Some(b.id.clone()), ..Route::default()
        }).unwrap();
        let mut connection = Some("connection-a");
        let mut owner = Some("default-session".into());
        switch_connection(&mut connection, &mut owner, &b.id, |_old| async { Ok(()) }).await.unwrap();
        let reopen: crate::store::Result<()> = Err("mock reconnect failure".into());
        assert!(reopen.is_err());
        assert!(interrupt_consumed_input(&store, &input.id).unwrap());
        assert!(store.confirm_turn_start(&input.id, "thread-b", "turn-b").is_err());
        assert_eq!(store.snapshot().unwrap().messages[0].delivery, "interrupted");
    }
}

#[cfg(test)]
mod opencode_commit_tests {
    use super::commit_opencode_answer;
    use crate::{opencode::Answer, store::Store};

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("opencode.sqlite")).unwrap();
        (dir, store)
    }

    fn answer() -> Answer {
        Answer { session_id: "ses_external".into(), text: "OpenCode 답변".into() }
    }

    #[test]
    fn a_finished_turn_binds_the_external_session_and_stores_the_answer() {
        let (_dir, store) = store();
        let user = store.message("user", "질문", false).unwrap();
        commit_opencode_answer(&store, "default-session", &user.id, &answer()).unwrap();
        let session = store.session("default-session").unwrap();
        assert_eq!(session.thread_id.as_deref(), Some("ses_external"));
        assert_eq!(session.session_id, "ses_external");
        let messages = store.snapshot().unwrap().messages;
        assert_eq!(messages.len(), 2);
        assert_eq!((messages[0].delivery.as_str(), messages[0].turn_id.is_some()), ("delivered", true));
        assert_eq!((messages[1].speaker.as_str(), messages[1].channel.as_str()), ("opencode", "main"));
        assert_eq!(
            (messages[1].content.as_str(), messages[1].delivery.as_str()),
            ("OpenCode 답변", "delivered")
        );
    }

    #[test]
    fn a_cancelled_turn_neither_moves_the_external_session_nor_stores_an_answer() {
        let (_dir, store) = store();
        let user = store.message("user", "질문", false).unwrap();
        store.cancel_queued("default-session").unwrap();
        assert!(commit_opencode_answer(&store, "default-session", &user.id, &answer()).is_err());
        let session = store.session("default-session").unwrap();
        assert_eq!(session.thread_id, None);
        assert_eq!(session.session_id, "");
        let messages = store.snapshot().unwrap().messages;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].delivery, "cancelled");
    }
}
