//! Role-specific adapters around the existing conversation-only CLI implementations.
//! Capability metadata describes support; it does not claim installation or authentication.
use crate::{cli::{self, Rpc}, models::ModelSettings, store::{Result, Session, Store}};
use serde_json::Value;
use std::{future::Future, ops::{Deref, DerefMut}, pin::Pin, sync::Arc};
use tokio::sync::watch;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub type AgentFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionKind { AppServerStdioJsonRpc, AgyCliStreamJson, OpenCodeCliJson, #[cfg(test)] MockInProcess }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentRole { Lead, Reviewer }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentCapabilities {
    pub lead_conversation: bool,
    pub delegated_review: bool,
    pub persistent_session: bool,
    pub streaming: bool,
    pub cancellation: bool,
    pub model_catalog: bool,
    pub context_compaction: bool,
    pub tool_call_bridge: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub connection: ConnectionKind,
    pub roles: &'static [AgentRole],
    pub capabilities: AgentCapabilities,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Installation { Installed, Missing, Unknown, Error }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum Authentication { Authenticated, Required, Unknown, Error }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum Availability { Ready, Unavailable, Unknown, Error }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProbe {
    pub id: &'static str,
    pub connection: &'static str,
    pub installation: Installation,
    pub authentication: Authentication,
    pub availability: Availability,
    pub checked_at: String,
    pub failure_reason: Option<String>,
}

fn inspect_file(path: &Path) -> (Installation, Option<String>) {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => (Installation::Installed, None),
        Ok(_) => (Installation::Missing, Some("실행 파일 경로가 파일이 아닙니다".into())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Installation::Missing, Some("실행 파일을 찾을 수 없습니다".into())),
        Err(e) => (Installation::Error, Some(format!("실행 파일 확인 실패: {e}"))),
    }
}

fn inspect_codex_bin(root: &Path) -> (Installation, Option<String>) {
    match std::fs::read_dir(root) {
        Ok(entries) => {
            let mut failed = None;
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let (status, reason) = inspect_file(&entry.path().join("codex.exe"));
                        if status == Installation::Installed { return (status, None); }
                        if status == Installation::Error { failed = reason; }
                    }
                    Err(e) => failed = Some(format!("Codex 실행 경로 확인 실패: {e}")),
                }
            }
            failed.map_or((Installation::Missing, Some("Codex CLI 실행 파일이 없습니다".into())), |reason| (Installation::Error, Some(reason)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Installation::Missing, Some("Codex 실행 경로를 찾을 수 없습니다".into())),
        Err(e) => (Installation::Error, Some(format!("Codex 실행 경로 확인 실패: {e}"))),
    }
}

fn probe_install(id: &str) -> (Installation, Option<String>) {
    if id == "opencode" {
        let Some(appdata) = std::env::var_os("APPDATA") else {
            return (Installation::Unknown, Some("APPDATA를 확인할 수 없습니다".into()));
        };
        return inspect_file(&PathBuf::from(appdata).join("npm/node_modules/opencode-ai/bin/opencode.exe"));
    }
    let variable = if id == "codex" { "ESH_CODEX_BIN" } else { "ESH_AGY_BIN" };
    if let Some(path) = std::env::var_os(variable) {
        return inspect_file(&PathBuf::from(path));
    }
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return (Installation::Unknown, Some("USERPROFILE을 확인할 수 없습니다".into()));
    };
    let home = PathBuf::from(home);
    if id == "codex" { inspect_codex_bin(&home.join("AppData/Local/OpenAI/Codex/bin")) }
    else { inspect_file(&home.join(".gemini/bin/agy.exe")) }
}

fn probe_adapter(descriptor: AgentDescriptor) -> AgentProbe {
    let (installation, failure_reason) = probe_install(descriptor.id);
    // A file check cannot establish login or a usable model. No process is started.
    let availability = match installation {
        Installation::Missing => Availability::Unavailable,
        Installation::Error => Availability::Error,
        _ => Availability::Unknown,
    };
    AgentProbe {
        id: descriptor.id,
        connection: match descriptor.connection {
            ConnectionKind::AppServerStdioJsonRpc => "App Server · stdio JSON-RPC",
            ConnectionKind::AgyCliStreamJson => "agy CLI · stream-json",
            ConnectionKind::OpenCodeCliJson => "OpenCode CLI · JSON",
            #[cfg(test)] ConnectionKind::MockInProcess => "Mock · in-process",
        },
        installation,
        authentication: Authentication::Unknown,
        availability,
        checked_at: crate::store::timestamp(),
        failure_reason,
    }
}

pub trait AgentAdapter: Send + Sync {
    fn descriptor(&self) -> AgentDescriptor;
    // Read-only filesystem inspection. Never starts a CLI or touches a session.
    fn probe(&self) -> AgentProbe { probe_adapter(self.descriptor()) }
}

pub trait LeadAdapter: AgentAdapter {
    type Session: LeadSession;
    fn connect(&self, store: Arc<Store>) -> AgentFuture<'_, Self::Session>;
}

pub trait LeadSession: Send {
    fn open_or_resume<'a>(&'a mut self, saved: &'a Session) -> AgentFuture<'a, ()>;
    fn begin_turn<'a>(&'a mut self, input: &'a str, settings: &'a ModelSettings) -> AgentFuture<'a, ()>;
    fn next_event(&mut self) -> AgentFuture<'_, Value>;
    fn reply_to_tool_call<'a>(&'a mut self, call: &'a Value, result: Value, success: bool) -> AgentFuture<'a, ()>;
    fn cancel_turn(&mut self) -> AgentFuture<'_, String>;
    fn close(&mut self) -> AgentFuture<'_, ()>;
}

pub trait ReviewAdapter: AgentAdapter {
    fn review<'a>(&'a self, store: Arc<Store>, question: &'a str, version: i64, cancel: watch::Receiver<bool>) -> AgentFuture<'a, Value>;
}

pub struct CodexAdapter;
pub struct AntigravityAdapter;
pub struct OpenCodeAdapter;
pub const CODEX: CodexAdapter = CodexAdapter;
pub const ANTIGRAVITY: AntigravityAdapter = AntigravityAdapter;
pub const OPENCODE: OpenCodeAdapter = OpenCodeAdapter;

pub fn registered() -> [&'static dyn AgentAdapter; 3] { [&CODEX, &ANTIGRAVITY, &OPENCODE] }

impl AgentAdapter for OpenCodeAdapter {
    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor { id: "opencode", display_name: "OpenCode", connection: ConnectionKind::OpenCodeCliJson,
            roles: &[AgentRole::Lead],
            capabilities: AgentCapabilities { lead_conversation: true, delegated_review: false,
                persistent_session: true, streaming: false, cancellation: true, model_catalog: false,
                context_compaction: false, tool_call_bridge: false },
        }
    }
}

#[cfg(test)]
pub(crate) fn registered_with_mock(mock: &dyn AgentAdapter) -> Vec<&dyn AgentAdapter> {
    let mut adapters: Vec<&dyn AgentAdapter> = registered().into_iter().collect();
    adapters.push(mock);
    adapters
}

impl AgentAdapter for CodexAdapter {
    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor {
            id: "codex", display_name: "Codex", connection: ConnectionKind::AppServerStdioJsonRpc,
            roles: &[AgentRole::Lead],
            capabilities: AgentCapabilities {
                lead_conversation: true, delegated_review: false, persistent_session: true,
                streaming: true, cancellation: true, model_catalog: true,
                context_compaction: true, tool_call_bridge: true,
            },
        }
    }
}

impl LeadAdapter for CodexAdapter {
    type Session = CodexLeadSession;
    fn connect(&self, store: Arc<Store>) -> AgentFuture<'_, Self::Session> {
        Box::pin(async move { Ok(CodexLeadSession(Rpc::start(store).await?)) })
    }
}

pub struct CodexLeadSession(Rpc);

// Keep the existing event/compaction implementation in Rpc while the Engine owns
// turn ordering and the blocking delegation loop.
impl Deref for CodexLeadSession {
    type Target = Rpc;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl DerefMut for CodexLeadSession {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl LeadSession for CodexLeadSession {
    fn open_or_resume<'a>(&'a mut self, saved: &'a Session) -> AgentFuture<'a, ()> {
        Box::pin(self.0.initialize(saved))
    }
    fn begin_turn<'a>(&'a mut self, input: &'a str, settings: &'a ModelSettings) -> AgentFuture<'a, ()> {
        Box::pin(self.0.start_turn(input, settings))
    }
    fn next_event(&mut self) -> AgentFuture<'_, Value> { Box::pin(self.0.next()) }
    fn reply_to_tool_call<'a>(&'a mut self, call: &'a Value, result: Value, success: bool) -> AgentFuture<'a, ()> {
        Box::pin(self.0.resolve_once(call, result, success))
    }
    fn cancel_turn(&mut self) -> AgentFuture<'_, String> { Box::pin(self.0.cancel_turn()) }
    fn close(&mut self) -> AgentFuture<'_, ()> { Box::pin(self.0.close()) }
}

impl AgentAdapter for AntigravityAdapter {
    fn descriptor(&self) -> AgentDescriptor {
        AgentDescriptor {
            id: "antigravity", display_name: "Antigravity", connection: ConnectionKind::AgyCliStreamJson,
            roles: &[AgentRole::Reviewer],
            capabilities: AgentCapabilities {
                lead_conversation: false, delegated_review: true, persistent_session: false,
                streaming: false, cancellation: true, model_catalog: false,
                context_compaction: false, tool_call_bridge: false,
            },
        }
    }
}

impl ReviewAdapter for AntigravityAdapter {
    fn review<'a>(&'a self, store: Arc<Store>, question: &'a str, version: i64, cancel: watch::Receiver<bool>) -> AgentFuture<'a, Value> {
        Box::pin(cli::arch(store, question, version, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn installation_probe_distinguishes_present_missing_and_inspection_error() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("codex.exe");
        std::fs::write(&exe, b"test").unwrap();
        assert_eq!(inspect_file(&exe).0, Installation::Installed);
        assert_eq!(inspect_file(&dir.path().join("missing.exe")).0, Installation::Missing);
        assert_eq!(inspect_file(&dir.path().join("bad\0name.exe")).0, Installation::Error);
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(bin.join("version-a")).unwrap();
        std::fs::write(bin.join("version-a/codex.exe"), b"test").unwrap();
        assert_eq!(inspect_codex_bin(&bin).0, Installation::Installed);
    }

    #[test]
    fn probe_keeps_authentication_and_availability_independent() {
        let codex = CODEX.probe();
        assert_eq!(codex.authentication, Authentication::Unknown);
        assert_eq!(codex.availability, match codex.installation {
            Installation::Missing => Availability::Unavailable,
            Installation::Error => Availability::Error,
            _ => Availability::Unknown,
        });
        assert!(!codex.checked_at.is_empty());
        assert_eq!(codex.connection, "App Server · stdio JSON-RPC");
    }

    #[test]
    fn probe_does_not_change_existing_conversation_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("conversation.sqlite")).unwrap();
        let message = store.message("user", "preserve this turn", false).unwrap();
        let before = store.snapshot().unwrap();
        let _ = CODEX.probe();
        let _ = ANTIGRAVITY.probe();
        let after = store.snapshot().unwrap();
        assert_eq!(before.revision, after.revision);
        assert_eq!(after.messages.len(), 1);
        assert_eq!(after.messages[0].id, message.id);
        assert_eq!(after.messages[0].delivery, "queued");
        assert_eq!(before.sessions.len(), after.sessions.len());
    }

    #[test]
    fn identity_transport_role_and_capability_are_independent() {
        let agents = registered();
        let codex = agents[0].descriptor();
        let arch = agents[1].descriptor();
        assert_eq!((codex.id, arch.id), ("codex", "antigravity"));
        assert_eq!(codex.connection, ConnectionKind::AppServerStdioJsonRpc);
        assert_eq!(arch.connection, ConnectionKind::AgyCliStreamJson);
        assert_eq!(codex.roles, [AgentRole::Lead]);
        assert_eq!(arch.roles, [AgentRole::Reviewer]);
        assert!(codex.capabilities.lead_conversation && codex.capabilities.tool_call_bridge);
        assert!(!codex.capabilities.delegated_review);
        assert!(arch.capabilities.delegated_review && arch.capabilities.cancellation);
        assert!(!arch.capabilities.lead_conversation && !arch.capabilities.tool_call_bridge);
        assert!(!arch.capabilities.persistent_session && !arch.capabilities.streaming);
        assert_eq!(agents[0].probe().authentication, Authentication::Unknown);
        assert_eq!(agents[1].probe().authentication, Authentication::Unknown);
    }

    struct MockReviewer;
    impl AgentAdapter for MockReviewer {
        fn descriptor(&self) -> AgentDescriptor { ANTIGRAVITY.descriptor() }
    }
    impl ReviewAdapter for MockReviewer {
        fn review<'a>(&'a self, _store: Arc<Store>, question: &'a str, version: i64, cancel: watch::Receiver<bool>) -> AgentFuture<'a, Value> {
            Box::pin(async move {
                if *cancel.borrow() { return Err("cancelled".into()); }
                Ok(json!({"answer":question,"conversationVersion":version}))
            })
        }
    }
    #[tokio::test]
    async fn reviewer_contract_passes_question_version_and_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&dir.path().join("adapter.sqlite")).unwrap());
        let (cancel, receiver) = watch::channel(false);
        let mock = MockReviewer;
        let result = ReviewAdapter::review(&mock, store.clone(), "question", 7, receiver.clone()).await.unwrap();
        assert_eq!(result, json!({"answer":"question","conversationVersion":7}));
        cancel.send(true).unwrap();
        assert!(ReviewAdapter::review(&mock, store, "question", 7, receiver).await.is_err());
    }

    struct MockLeadAdapter;
    struct MockLeadSession { opened: bool, replied: bool }
    impl AgentAdapter for MockLeadAdapter {
        fn descriptor(&self) -> AgentDescriptor { CODEX.descriptor() }
    }
    impl LeadAdapter for MockLeadAdapter {
        type Session = MockLeadSession;
        fn connect(&self, _store: Arc<Store>) -> AgentFuture<'_, Self::Session> {
            Box::pin(async { Ok(MockLeadSession { opened: false, replied: false }) })
        }
    }
    impl LeadSession for MockLeadSession {
        fn open_or_resume<'a>(&'a mut self, saved: &'a Session) -> AgentFuture<'a, ()> {
            Box::pin(async move { self.opened = saved.id == "default-session"; Ok(()) })
        }
        fn begin_turn<'a>(&'a mut self, _input: &'a str, _settings: &'a ModelSettings) -> AgentFuture<'a, ()> {
            Box::pin(async move { if self.opened { Ok(()) } else { Err("not open".into()) } })
        }
        fn next_event(&mut self) -> AgentFuture<'_, Value> {
            Box::pin(async { Ok(json!({"id":"server-call","method":"item/tool/call","params":{"callId":"call"}})) })
        }
        fn reply_to_tool_call<'a>(&'a mut self, call: &'a Value, _result: Value, _success: bool) -> AgentFuture<'a, ()> {
            Box::pin(async move {
                if call["id"] != "server-call" || self.replied { return Err("wrong or duplicate call".into()); }
                self.replied = true;
                Ok(())
            })
        }
        fn cancel_turn(&mut self) -> AgentFuture<'_, String> { Box::pin(async { Ok("cancelled".into()) }) }
        fn close(&mut self) -> AgentFuture<'_, ()> { Box::pin(async { Ok(()) }) }
    }
    #[tokio::test]
    async fn lead_contract_keeps_the_open_tool_call_identity() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&dir.path().join("lead.sqlite")).unwrap());
        let mut session = MockLeadAdapter.connect(store.clone()).await.unwrap();
        session.open_or_resume(&store.session("default-session").unwrap()).await.unwrap();
        session.begin_turn("hello", &ModelSettings { model: None, reasoning_effort: None }).await.unwrap();
        let call = session.next_event().await.unwrap();
        session.reply_to_tool_call(&call, json!({"answer":"review"}), true).await.unwrap();
        assert!(session.reply_to_tool_call(&call, json!({}), true).await.is_err());
        assert_eq!(session.cancel_turn().await.unwrap(), "cancelled");
        session.close().await.unwrap();
    }
}
