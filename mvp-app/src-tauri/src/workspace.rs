use crate::store::{err, timestamp, Result, Session, Store};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 {
        return Err("이름은 1~100자로 입력해 주세요".into());
    }
    Ok(value.into())
}

pub fn migrate(db: &mut Connection) -> Result<()> {
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(err)?;
    if version >= 2 {
        return Ok(());
    }
    let tx = db.transaction().map_err(err)?;
    tx.execute_batch("CREATE TABLE rooms(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,created_at TEXT NOT NULL);
      CREATE TABLE chat_sessions(id TEXT PRIMARY KEY,room_id TEXT NOT NULL REFERENCES rooms(id),name TEXT NOT NULL,thread_id TEXT,codex_session_id TEXT NOT NULL DEFAULT '',source_thread_id TEXT,mode TEXT NOT NULL,version INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL);
      CREATE UNIQUE INDEX imported_session_unique ON chat_sessions(room_id,source_thread_id,mode) WHERE source_thread_id IS NOT NULL AND mode IN ('copy','resume');
      ALTER TABLE messages ADD COLUMN chat_session_id TEXT NOT NULL DEFAULT 'default-session';
      CREATE INDEX messages_local_session ON messages(chat_session_id,sequence);").map_err(err)?;
    tx.execute(
        "INSERT INTO rooms VALUES('default','ESH',?1)",
        [timestamp()],
    )
    .map_err(err)?;
    tx.execute("INSERT INTO chat_sessions(id,room_id,name,mode,created_at) VALUES('default-session','default','기본 세션','new',?1)",[timestamp()]).map_err(err)?;
    let entries = {
        let mut q = tx
            .prepare(
                "SELECT created_at,payload FROM audit WHERE kind='codex_session' ORDER BY sequence",
            )
            .map_err(err)?;
        let rows = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    for (index, (at, payload)) in entries.into_iter().enumerate() {
        let Ok(v) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };
        let Some(id) = v["threadId"].as_str() else {
            continue;
        };
        tx.execute("INSERT OR IGNORE INTO chat_sessions(id,room_id,name,codex_session_id,source_thread_id,mode,created_at) VALUES(?1,'default',?2,?3,?1,'legacy',?4)",params![id,format!("이전 세션 {}",index+1),v["sessionId"].as_str().unwrap_or(id),at]).map_err(err)?;
        tx.execute(
            "UPDATE messages SET chat_session_id=?1 WHERE thread_id=?1",
            [id],
        )
        .map_err(err)?;
    }
    tx.execute("UPDATE chat_sessions SET version=COALESCE((SELECT MAX(version) FROM messages WHERE chat_session_id=chat_sessions.id),0)",[]).map_err(err)?;
    tx.execute_batch("PRAGMA user_version=2;").map_err(err)?;
    tx.commit().map_err(err)
}

pub fn migrate_model_settings(db: &mut Connection) -> Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(err)?;
    if version >= 3 { return Ok(()); }
    let tx = db.transaction().map_err(err)?;
    tx.execute_batch("ALTER TABLE chat_sessions ADD COLUMN model TEXT; ALTER TABLE chat_sessions ADD COLUMN reasoning_effort TEXT; PRAGMA user_version=3;").map_err(err)?;
    tx.commit().map_err(err)
}

pub fn migrate_agent_sessions(db: &mut Connection) -> Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(err)?;
    if version >= 6 { return Ok(()); }
    let columns = db.prepare("PRAGMA table_info(chat_sessions)").map_err(err)?
        .query_map([], |r| r.get::<_, String>(1)).map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(err)?;
    let tx = db.transaction().map_err(err)?;
    if !columns.iter().any(|c| c == "provider") {
        tx.execute_batch("ALTER TABLE chat_sessions ADD COLUMN provider TEXT NOT NULL DEFAULT 'codex'").map_err(err)?;
    }
    if !columns.iter().any(|c| c == "agent_instance_id") {
        tx.execute_batch("ALTER TABLE chat_sessions ADD COLUMN agent_instance_id TEXT").map_err(err)?;
    }
    tx.execute_batch("UPDATE agent_instances SET connection_kind='opencode_cli_json'
          WHERE provider='opencode' AND role='lead' AND source='user'
          AND lower(replace(executable_path,'\\','/')) LIKE '%/node_modules/opencode-ai/bin/opencode.exe';
        INSERT OR IGNORE INTO agents(id,status,detail,updated_at)
          SELECT id,'idle','대기 중',created_at FROM agent_instances
          WHERE provider='opencode' AND connection_kind='opencode_cli_json';
        PRAGMA user_version=6;").map_err(err)?;
    tx.commit().map_err(err)
}

pub fn sessions(db: &Connection) -> Result<Vec<Session>> {
    let mut q=db.prepare("SELECT id,codex_session_id,created_at,room_id,name,thread_id,source_thread_id,mode,model,reasoning_effort,provider,agent_instance_id FROM chat_sessions ORDER BY created_at,id").map_err(err)?;
    let rows = q
        .query_map([], |r| {
            Ok(Session {
                id: r.get(0)?,
                session_id: r.get(1)?,
                created_at: r.get(2)?,
                room_id: r.get(3)?,
                name: r.get(4)?,
                thread_id: r.get(5)?,
                source_thread_id: r.get(6)?,
                mode: r.get(7)?,
                model: r.get(8)?,
                reasoning_effort: r.get(9)?,
                provider: r.get(10)?,
                agent_instance_id: r.get(11)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(rows)
}
pub fn rooms(db: &Connection) -> Result<Vec<Room>> {
    let mut q = db
        .prepare("SELECT id,name,created_at FROM rooms ORDER BY created_at,id")
        .map_err(err)?;
    let rows = q
        .query_map([], |r| {
            Ok(Room {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(rows)
}

impl Store {
    pub fn set_model_settings(&self, id: &str, settings: &crate::models::ModelSettings) -> Result<()> {
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        let changed = tx.execute("UPDATE chat_sessions SET model=?1,reasoning_effort=?2 WHERE id=?3", params![settings.model, settings.reasoning_effort, id]).map_err(err)?;
        if changed != 1 { return Err("세션을 찾을 수 없습니다".into()); }
        tx.execute("UPDATE meta SET revision=revision+1", []).map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn create_room(&self, value: &str) -> Result<Room> {
        let value = name(value)?;
        let room = Room {
            id: uuid::Uuid::new_v4().to_string(),
            name: value,
            created_at: timestamp(),
        };
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        tx.execute(
            "INSERT INTO rooms VALUES(?1,?2,?3)",
            params![room.id, room.name, room.created_at],
        )
        .map_err(|e| format!("프로젝트·룸 생성 실패 (이름 중복 여부를 확인해 주세요): {e}"))?;
        tx.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(room)
    }
    pub fn create_session(&self, room_id: &str, value: &str) -> Result<Session> {
        self.add_session(room_id, value, None, "new", &[])
    }
    pub fn create_opencode_session(&self, room_id: &str, value: &str, instance_id: &str) -> Result<Session> {
        let instance = self.list_agent_instances()?.into_iter().find(|i| i.id == instance_id)
            .ok_or("OpenCode 등록을 찾을 수 없습니다")?;
        if instance.provider != "opencode" || instance.active_role != Some("lead") || instance.installation != "installed" {
            return Err("실행 가능한 OpenCode Lead 등록이 아닙니다".into());
        }
        self.create_lead_session(room_id, value, instance_id, "opencode")
    }
    /// Claude Code Lead sessions. The registration check keys on the connection
    /// kind instead of `active_role` because `active_role` stays `None` until an
    /// adapter claims that connection, and the adapter is registered after this
    /// session path. Gating on the connection kind means a plain registration
    /// (which carries `not_configured` today) cannot own a session, and the path
    /// opens by itself when the adapter starts assigning `claude_code_cli`.
    pub fn create_claude_session(&self, room_id: &str, value: &str, instance_id: &str) -> Result<Session> {
        let instance = self.list_agent_instances()?.into_iter().find(|i| i.id == instance_id)
            .ok_or("Claude Code 등록을 찾을 수 없습니다")?;
        if instance.provider != "claude_code" || instance.role != "lead" || instance.installation != "installed" {
            return Err("실행 가능한 Claude Code Lead 등록이 아닙니다".into());
        }
        if instance.connection != crate::claude::CONNECTION_KIND {
            return Err("Claude Code 실행 연결이 설정되지 않은 등록입니다".into());
        }
        self.create_lead_session(room_id, value, instance_id, "claude_code")
    }
    /// Picks the session kind from the registration instead of assuming every
    /// `agent_instance_id` is an OpenCode Lead. `antigravity` is a reviewer and
    /// `codex` is the builtin Lead, so neither can own a chat session.
    pub fn create_agent_session(&self, room_id: &str, value: &str, instance_id: &str) -> Result<Session> {
        let instance = self.list_agent_instances()?.into_iter().find(|i| i.id == instance_id)
            .ok_or("에이전트 등록을 찾을 수 없습니다")?;
        match instance.provider.as_str() {
            "opencode" => self.create_opencode_session(room_id, value, instance_id),
            "claude_code" => self.create_claude_session(room_id, value, instance_id),
            _ => Err("이 에이전트로는 대화 세션을 만들 수 없습니다".into()),
        }
    }
    /// The write both CLI Leads share. The row is inserted with its provider and
    /// owning registration already in place, in the same transaction as the
    /// revision bump. The previous version inserted through `add_session` (which
    /// commits on its own) and then patched the provider, so a snapshot emitted
    /// between the two commits showed a Lead-owned session as a `codex` one, and
    /// a failed patch left that `codex` row behind as a phantom session.
    fn create_lead_session(&self, room_id: &str, value: &str, instance_id: &str, provider: &str) -> Result<Session> {
        let value = name(value)?;
        let id = uuid::Uuid::new_v4().to_string();
        let at = timestamp();
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        tx.execute(
            "INSERT INTO chat_sessions(id,room_id,name,provider,agent_instance_id,mode,created_at) VALUES(?1,?2,?3,?4,?5,'new',?6)",
            params![id, room_id, value, provider, instance_id, at],
        )
        .map_err(|e| format!("세션 추가 실패 (대상 룸이나 중복 추가를 확인해 주세요): {e}"))?;
        tx.execute("UPDATE meta SET revision=revision+1", []).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Session {
            id,
            session_id: String::new(),
            created_at: at,
            room_id: room_id.into(),
            name: value,
            thread_id: None,
            source_thread_id: None,
            mode: "new".into(),
            model: None,
            reasoning_effort: None,
            provider: provider.into(),
            agent_instance_id: Some(instance_id.into()),
        })
    }
    pub fn session(&self, id: &str) -> Result<Session> {
        sessions(&*self.db.lock().map_err(err)?)?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or("세션을 찾을 수 없습니다".into())
    }
    pub fn session_version(&self, id: &str) -> Result<i64> {
        self.db
            .lock()
            .map_err(err)?
            .query_row("SELECT version FROM chat_sessions WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(err)
    }
    /// Points a local session at the external session it is talking to. The
    /// updated-row count is checked because the previous version bumped
    /// `meta.revision` and returned `Ok(())` even when the session row did not
    /// exist, so a caller could not tell a real bind from a no-op. `thread_id` is
    /// part of the snapshot, so the revision bump belongs in the same transaction
    /// as the update: either the WebView sees the new thread or nothing changed.
    pub fn bind_codex_session(&self, id: &str, thread: &str, session: &str) -> Result<()> {
        if id.trim().is_empty() || thread.trim().is_empty() || session.trim().is_empty() {
            return Err("세션 연결 정보가 비어 있습니다".into());
        }
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        let changed = tx
            .execute(
                "UPDATE chat_sessions SET thread_id=?1,codex_session_id=?2 WHERE id=?3",
                params![thread, session, id],
            )
            .map_err(err)?;
        if changed != 1 {
            return Err("세션을 찾을 수 없습니다".into());
        }
        tx.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn add_session(
        &self,
        room_id: &str,
        value: &str,
        source: Option<&str>,
        mode: &str,
        history: &[Value],
    ) -> Result<Session> {
        if !["new", "copy", "resume"].contains(&mode) {
            return Err("지원하지 않는 가져오기 방식입니다".into());
        }
        let value = name(value)?;
        let id = uuid::Uuid::new_v4().to_string();
        let at = timestamp();
        let thread = if mode == "resume" {
            source.map(str::to_string)
        } else {
            None
        };
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        tx.execute("INSERT INTO chat_sessions(id,room_id,name,thread_id,source_thread_id,mode,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,room_id,value,thread,source,mode,at]).map_err(|e|format!("세션 추가 실패 (대상 룸이나 중복 추가를 확인해 주세요): {e}"))?;
        for entry in history {
            let speaker = entry["speaker"].as_str().unwrap_or("codex");
            let content = entry["content"].as_str().unwrap_or("");
            if content.is_empty() {
                continue;
            }
            let channel = if entry["phase"] == "commentary" {
                "activity"
            } else {
                "main"
            };
            tx.execute("INSERT INTO messages(id,speaker,content,created_at,delivery,version,is_error,channel,kind,room_id,thread_id,turn_id,chat_session_id) VALUES(?1,?2,?3,?4,'delivered',0,0,?5,?6,?7,?8,?9,?10)",params![uuid::Uuid::new_v4().to_string(),speaker,content,entry["createdAt"].as_str().unwrap_or(&at),channel,if speaker=="user"{"user_message"}else{"answer"},room_id,source,entry["turnId"].as_str(),id]).map_err(err)?;
        }
        tx.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Session {
            id,
            session_id: String::new(),
            created_at: at,
            room_id: room_id.into(),
            name: value,
            thread_id: thread,
            source_thread_id: source.map(str::to_string),
            mode: mode.into(),
            model: None,
            reasoning_effort: None,
            provider: "codex".into(),
            agent_instance_id: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_settings_migrate_without_losing_history_and_restore_per_session() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("models.db");
        let s = Store::open(&path).unwrap();
        let a = s.create_session("default", "A").unwrap();
        let b = s.create_session("default", "B").unwrap();
        s.set_execution_session(&a.id).unwrap();
        let message = s.message("user", "preserve history", false).unwrap();
        drop(s);
        let db = Connection::open(&path).unwrap();
        db.execute_batch("ALTER TABLE chat_sessions DROP COLUMN model; ALTER TABLE chat_sessions DROP COLUMN reasoning_effort; PRAGMA user_version=2;").unwrap();
        drop(db);
        let s = Store::open(&path).unwrap();
        assert!(s.session(&a.id).unwrap().model.is_none());
        assert_eq!(s.snapshot().unwrap().messages[0].id, message.id);
        let settings = crate::models::ModelSettings { model: Some("model-a".into()), reasoning_effort: Some("medium".into()) };
        s.set_model_settings(&a.id, &settings).unwrap();
        assert!(s.session(&b.id).unwrap().model.is_none());
        assert!(s.set_model_settings("missing", &settings).is_err());
        drop(s);
        let s = Store::open(&path).unwrap();
        assert_eq!(s.session(&a.id).unwrap().model.as_deref(), Some("model-a"));
        assert_eq!(s.session(&a.id).unwrap().reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(s.snapshot().unwrap().messages[0].content, "preserve history");
    }
    #[test]
    fn rooms_and_sessions_isolate_context_and_versions() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("test.db")).unwrap();
        let a = s.create_room("프로젝트 A").unwrap();
        let b = s.create_room("프로젝트 B").unwrap();
        let sa = s.create_session(&a.id, "논의 A").unwrap();
        let sb = s.create_session(&b.id, "논의 B").unwrap();
        s.set_execution_session(&sa.id).unwrap();
        s.message("user", "ONLY_A", false).unwrap();
        s.set_execution_session(&sb.id).unwrap();
        s.message("user", "ONLY_B", false).unwrap();
        assert!(!s.context().unwrap().contains("ONLY_A"));
        assert_eq!(s.session_version(&sa.id).unwrap(), 1);
        assert_eq!(s.session_version(&sb.id).unwrap(), 1);
        assert_eq!(s.snapshot().unwrap().rooms.len(), 3);
        assert!(s.create_session("missing", "fail").is_err());
    }

    #[test]
    fn imported_history_has_distinct_copy_and_resume_semantics() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("test.db");
        let s = Store::open(&path).unwrap();
        let room = s.create_room("가져오기").unwrap();
        let history = vec![
            serde_json::json!({"speaker":"user","content":"기억할 내용","createdAt":"2026-10-18T00:00:00Z","turnId":"old-turn"}),
        ];
        let copy = s
            .add_session(&room.id, "사본", Some("original-thread"), "copy", &history)
            .unwrap();
        let resume = s
            .add_session(
                &room.id,
                "이어쓰기",
                Some("original-thread"),
                "resume",
                &history,
            )
            .unwrap();
        assert!(copy.thread_id.is_none());
        assert_eq!(resume.thread_id.as_deref(), Some("original-thread"));
        assert!(s
            .add_session(&room.id, "중복", Some("original-thread"), "copy", &history)
            .is_err());
        assert!(s
            .add_session(
                &room.id,
                "잘못된 모드",
                Some("original-thread"),
                "invalid",
                &history
            )
            .is_err());
        let snap = s.snapshot().unwrap();
        assert_eq!(
            snap.messages
                .iter()
                .filter(|m| m.chat_session_id == copy.id)
                .count(),
            1
        );
        drop(s);
        let restored = Store::open(&path).unwrap();
        assert_eq!(
            restored.session(&resume.id).unwrap().thread_id.as_deref(),
            Some("original-thread")
        );
    }

    #[test]
    fn a_claude_session_needs_a_registration_the_adapter_claims() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("claude.db")).unwrap();
        let exe = d.path().join("claude.exe");
        std::fs::write(&exe, b"do not execute").unwrap();
        let lead = s.register_agent_instance("claude_code", &exe.to_string_lossy(), "lead").unwrap();
        // A registration that no adapter claims has no execution connection, so
        // it must not be able to own a session (and `support` stays honest).
        assert_eq!(lead.connection, "not_configured");
        assert_eq!((lead.support, lead.active_role), ("unsupported", None));
        let connection_error = s.create_claude_session("default", "claude", &lead.id).err().expect("no connection");
        assert!(connection_error.contains("실행 연결이 설정되지 않은"), "{connection_error}");
        let dispatch_error = s.create_agent_session("default", "claude", &lead.id).err().expect("no connection");
        assert!(dispatch_error.contains("실행 연결이 설정되지 않은"), "{dispatch_error}");

        // The adapter (a later change) is what assigns the connection kind.
        s.db.lock().unwrap()
            .execute("UPDATE agent_instances SET connection_kind=?1 WHERE id=?2", params![crate::claude::CONNECTION_KIND, lead.id])
            .unwrap();
        let created = s.create_claude_session("default", "Claude test", &lead.id).unwrap();
        assert_eq!((created.provider.as_str(), created.agent_instance_id.as_deref()), ("claude_code", Some(lead.id.as_str())));
        // The session row, not just the returned value, carries the provider.
        let saved = s.session(&created.id).unwrap();
        assert_eq!((saved.provider.as_str(), saved.agent_instance_id.as_deref()), ("claude_code", Some(lead.id.as_str())));
        // Creating the session does not promote the registration: `support` only
        // moves once the adapter exists, which is the point of this ordering.
        let listed = s.list_agent_instances().unwrap().into_iter().find(|i| i.id == lead.id).unwrap();
        assert_eq!((listed.support, listed.active_role), ("unsupported", None));
        // A session pins its registration, so removal stays refused.
        assert!(s.unregister_agent_instance(&lead.id).is_err());

        // The command path dispatches on the provider of the registration.
        let via_agent = s.create_agent_session("default", "via agent", &lead.id).unwrap();
        assert_eq!(via_agent.provider, "claude_code");
        let other = d.path().join("review/claude.exe");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::write(&other, b"do not execute").unwrap();
        let reviewer = s.register_agent_instance("claude_code", &other.to_string_lossy(), "reviewer").unwrap();
        s.db.lock().unwrap()
            .execute("UPDATE agent_instances SET connection_kind=?1 WHERE id=?2", params![crate::claude::CONNECTION_KIND, reviewer.id])
            .unwrap();
        let role_error = s.create_claude_session("default", "reviewer", &reviewer.id).err().expect("reviewer");
        assert!(role_error.contains("실행 가능한 Claude Code Lead"), "{role_error}");
        // A registration whose file disappeared is not runnable either.
        std::fs::remove_file(&exe).unwrap();
        let missing_error = s.create_claude_session("default", "gone", &lead.id).err().expect("missing file");
        assert!(missing_error.contains("실행 가능한 Claude Code Lead"), "{missing_error}");
        // Codex and Antigravity cannot own a chat session.
        assert!(s.create_agent_session("default", "codex", "codex").is_err());
        assert!(s.create_agent_session("default", "arch", "antigravity").is_err());
        assert!(s.create_agent_session("default", "missing", "no-such-instance").is_err());

        drop(s);
        let restored = Store::open(&d.path().join("claude.db")).unwrap();
        assert_eq!(restored.session(&created.id).unwrap().provider, "claude_code");
    }

    #[test]
    fn a_lead_session_is_one_published_change_with_no_placeholder_row() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("lead.db")).unwrap();
        let cli = d.path().join("node_modules/opencode-ai/bin/opencode.exe");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, b"probe only").unwrap();
        let lead = s.register_agent_instance("opencode", &cli.to_string_lossy(), "lead").unwrap();
        let before = s.snapshot().unwrap().revision;
        let created = s.create_opencode_session("default", "Lead one", &lead.id).unwrap();
        // One created session is one publishable change. The previous version
        // inserted the row through `add_session` and patched the provider after,
        // so it bumped the revision twice for the same session.
        assert_eq!(s.snapshot().unwrap().revision - before, 1);
        // The row never exists in a `codex` shape: the provider and the owning
        // registration come from the insert itself.
        let saved = s.session(&created.id).unwrap();
        assert_eq!((saved.provider.as_str(), saved.agent_instance_id.as_deref()), ("opencode", Some(lead.id.as_str())));
        assert_eq!((saved.name.as_str(), saved.mode.as_str(), saved.thread_id.as_deref()), ("Lead one", "new", None));

        // A rejected room leaves neither a session row nor a revision bump.
        let after = s.snapshot().unwrap().revision;
        let sessions_before = s.snapshot().unwrap().sessions.len();
        let error = s.create_opencode_session("no-such-room", "orphan", &lead.id).err().expect("missing room");
        assert!(error.contains("세션 추가 실패"), "{error}");
        assert_eq!(s.snapshot().unwrap().revision, after);
        assert_eq!(s.snapshot().unwrap().sessions.len(), sessions_before);
    }

    #[test]
    fn binding_an_external_session_reports_misses_instead_of_storing_nothing() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("bind.db");
        let s = Store::open(&path).unwrap();
        let session = s.create_session("default", "A").unwrap();
        let before = s.snapshot().unwrap().revision;
        s.bind_codex_session(&session.id, "thread-a", "rpc-a").unwrap();
        let bound = s.session(&session.id).unwrap();
        assert_eq!(bound.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(bound.session_id, "rpc-a");
        // `thread_id` is part of the snapshot, so the bind has to be visible to
        // snapshot consumers without waiting for an unrelated change.
        assert!(s.snapshot().unwrap().revision > before);

        // A missing row must fail loudly and leave the previous binding alone.
        let after = s.snapshot().unwrap().revision;
        assert!(s.bind_codex_session("missing-session", "thread-b", "rpc-b").is_err());
        assert_eq!(s.snapshot().unwrap().revision, after);
        assert_eq!(s.session(&session.id).unwrap().thread_id.as_deref(), Some("thread-a"));
        // An empty thread or external id is not a binding; reject it instead of
        // storing an empty string that reads like "not bound yet".
        assert!(s.bind_codex_session(&session.id, "", "rpc-c").is_err());
        assert!(s.bind_codex_session(&session.id, "thread-c", "  ").is_err());
        assert_eq!(s.snapshot().unwrap().revision, after);
        assert_eq!(s.session(&session.id).unwrap().thread_id.as_deref(), Some("thread-a"));

        drop(s);
        let restored = Store::open(&path).unwrap();
        assert_eq!(restored.session(&session.id).unwrap().thread_id.as_deref(), Some("thread-a"));
        assert_eq!(restored.session(&session.id).unwrap().session_id, "rpc-a");
    }
}
