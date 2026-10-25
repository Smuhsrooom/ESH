use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::Path, sync::Mutex};

pub type Result<T> = std::result::Result<T, String>;
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub sequence: i64,
    pub id: String,
    pub speaker: String,
    pub content: String,
    pub created_at: String,
    pub delivery: String,
    pub version: i64,
    pub is_error: bool,
    pub channel: String,
    pub kind: String,
    pub room_id: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub collaboration_id: Option<String>,
    pub chat_session_id: String,
}

#[derive(Default)]
pub struct Route {
    pub channel: &'static str,
    pub kind: &'static str,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub collaboration_id: Option<String>,
    pub chat_session_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub session_id: String,
    pub created_at: String,
    pub room_id: String,
    pub name: String,
    pub thread_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub mode: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub provider: String,
    pub agent_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub status: String,
    pub detail: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub contexts: Vec<crate::context::Context>,
    pub active_run: Option<ActiveRunSnapshot>,
    pub messages: Vec<Message>,
    pub agents: Vec<Agent>,
    pub conversation_version: i64,
    pub revision: i64,
    pub sessions: Vec<Session>,
    pub rooms: Vec<crate::workspace::Room>,
    pub busy_session_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRunSnapshot {
    pub id: String,
    pub session_id: String,
    pub cancelling: bool,
}

pub struct Store {
    pub(crate) db: Mutex<Connection>,
    execution_session: Mutex<String>,
    busy_session: Mutex<Option<String>>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).map_err(err)?;
        }
        let mut db = Connection::open(path).map_err(err)?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(err)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS meta(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL,revision INTEGER NOT NULL);
          INSERT OR IGNORE INTO meta VALUES(1,0,0);
          CREATE TABLE IF NOT EXISTS messages(sequence INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT NOT NULL UNIQUE,speaker TEXT NOT NULL,content TEXT NOT NULL,created_at TEXT NOT NULL,delivery TEXT NOT NULL,version INTEGER NOT NULL,is_error INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS agents(id TEXT PRIMARY KEY,status TEXT NOT NULL,detail TEXT NOT NULL,updated_at TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS audit(sequence INTEGER PRIMARY KEY AUTOINCREMENT,created_at TEXT NOT NULL,kind TEXT NOT NULL,payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS tool_deliveries(call_key TEXT PRIMARY KEY,rpc_id TEXT NOT NULL,state TEXT NOT NULL,updated_at TEXT NOT NULL);").map_err(err)?;
        migrate_channels(&mut db)?;
        crate::workspace::migrate(&mut db)?;
        crate::workspace::migrate_model_settings(&mut db)?;
        crate::context::migrate(&mut db)?;
        crate::registry::migrate(&mut db)?;
        crate::workspace::migrate_agent_sessions(&mut db)?;
        crate::context::recover(&db)?;
        for agent in ["codex", "antigravity"] {
            db.execute(
                "INSERT OR IGNORE INTO agents VALUES(?1,'idle','대기 중',?2)",
                params![agent, timestamp()],
            )
            .map_err(err)?;
        }
        // `agents` rows are execution status derived from the registry, not user
        // data. A build that removed a registration without deleting its status
        // row left those rows behind, and nothing else ever removed them: they
        // kept counting in `snapshot.agents.length`, which `main.tsx` uses for the
        // Agents badge, while the list only renders builtin ids and instances that
        // still exist. Drop only rows whose instance is gone; the two builtin rows
        // and every registered instance keep theirs.
        db.execute(
            "DELETE FROM agents WHERE id NOT IN (SELECT id FROM agent_instances) AND id NOT IN ('codex','antigravity')",
            [],
        )
        .map_err(err)?;
        // The same invariant in the other direction. `main.tsx` counts
        // `snapshot.agents.length` for the Agents badge while the Agents view
        // lists the builtin ids plus every `agent_instances` row, so a
        // registration that never wrote a status row made the badge count fewer
        // agents than the view showed. Backfill only the missing rows: existing
        // statuses are never overwritten, and an instance without an execution
        // connection stays `unknown` instead of claiming to be idle.
        db.execute(
            "INSERT OR IGNORE INTO agents(id,status,detail,updated_at)
             SELECT id,
                    CASE WHEN connection_kind='opencode_cli_json' THEN 'idle' ELSE 'unknown' END,
                    CASE WHEN connection_kind='opencode_cli_json' THEN '대기 중' ELSE '실행 연결이 설정되지 않았습니다' END,
                    created_at
             FROM agent_instances",
            [],
        )
        .map_err(err)?;
        // A previous live request cannot be resumed after process death.
        db.execute("UPDATE agents SET status='unknown',detail='이전 실행이 중단됐습니다. 열린 호출은 자동 재개하지 않습니다.',updated_at=?1 WHERE status NOT IN ('idle','completed','failed','cancelled','unknown')", [timestamp()]).map_err(err)?;
        db.execute(
            "UPDATE tool_deliveries SET state='unknown' WHERE state IN ('prepared','sent')",
            [],
        )
        .map_err(err)?;
        db.execute(
            "UPDATE messages SET delivery='interrupted' WHERE speaker='user' AND delivery='queued'",
            [],
        )
        .map_err(err)?;
        db.execute("UPDATE messages SET delivery='interrupted' WHERE speaker='codex' AND delivery='streaming'", []).map_err(err)?;
        db.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        Ok(Self {
            db: Mutex::new(db),
            execution_session: Mutex::new("default-session".into()),
            busy_session: Mutex::new(None),
        })
    }

    pub fn message(&self, speaker: &str, text: &str, error: bool) -> Result<Message> {
        let channel = match speaker {
            "system" => "system",
            "antigravity" => "legacy",
            "user" if text.starts_with("@arch ") => "legacy",
            _ => "main",
        };
        self.routed_message(
            speaker,
            text,
            error,
            Route {
                channel,
                kind: if speaker == "user" {
                    "user_message"
                } else {
                    "answer"
                },
                ..Route::default()
            },
        )
    }

    pub fn routed_message(
        &self,
        speaker: &str,
        text: &str,
        error: bool,
        route: Route,
    ) -> Result<Message> {
        let local_id = route
            .chat_session_id
            .clone()
            .unwrap_or(self.execution_session_id()?);
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        let room_id: String = tx
            .query_row(
                "SELECT room_id FROM chat_sessions WHERE id=?1",
                [&local_id],
                |r| r.get(0),
            )
            .map_err(|_| "세션을 찾을 수 없습니다".to_string())?;
        if speaker == "user" {
            tx.execute("UPDATE meta SET version=version+1", [])
                .map_err(err)?;
            tx.execute(
                "UPDATE chat_sessions SET version=version+1 WHERE id=?1",
                [&local_id],
            )
            .map_err(err)?;
        }
        let version: i64 = tx
            .query_row(
                "SELECT version FROM chat_sessions WHERE id=?1",
                [&local_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        let id = uuid::Uuid::new_v4().to_string();
        let at = timestamp();
        let delivery = if speaker == "user" {
            "queued"
        } else if route.kind == "answer_stream" || route.kind == "commentary_stream" {
            "streaming"
        } else {
            "delivered"
        };
        tx.execute("INSERT INTO messages(id,speaker,content,created_at,delivery,version,is_error,channel,kind,room_id,thread_id,turn_id,collaboration_id,chat_session_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",params![id,speaker,text,at,delivery,version,error,route.channel,route.kind,room_id,route.thread_id,route.turn_id,route.collaboration_id,local_id]).map_err(err)?;
        let sequence = tx.last_insert_rowid();
        tx.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Message {
            sequence,
            id,
            speaker: speaker.into(),
            content: text.into(),
            created_at: at,
            delivery: delivery.into(),
            version,
            is_error: error,
            channel: route.channel.into(),
            kind: route.kind.into(),
            room_id,
            thread_id: route.thread_id,
            turn_id: route.turn_id,
            collaboration_id: route.collaboration_id,
            chat_session_id: local_id,
        })
    }

    pub fn bind_messages(&self, ids: &[String], thread: &str, turn: &str) -> Result<()> {
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        for id in ids {
            tx.execute(
                "UPDATE messages SET thread_id=?1,turn_id=?2 WHERE id=?3",
                params![thread, turn, id],
            )
            .map_err(err)?;
        }
        tx.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        tx.commit().map_err(err)
    }

    /// Commit the user's turn binding and delivery together, only after turn/start returned an ID.
    pub fn confirm_turn_start(&self, id: &str, thread: &str, turn: &str) -> Result<()> {
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        let changed = tx.execute(
            "UPDATE messages SET thread_id=?1,turn_id=?2,delivery='delivered' WHERE id=?3 AND speaker='user' AND delivery='queued'",
            params![thread, turn, id],
        ).map_err(err)?;
        if changed != 1 {
            return Err("이 사용자 메시지는 이미 실행됐거나 중단됐습니다".into());
        }
        tx.execute("UPDATE meta SET revision=revision+1", []).map_err(err)?;
        tx.commit().map_err(err)
    }

    pub fn interrupt_queued(&self, id: &str) -> Result<bool> {
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        let changed = tx.execute(
            "UPDATE messages SET delivery='interrupted' WHERE id=?1 AND speaker='user' AND delivery='queued'",
            [id],
        ).map_err(err)?;
        if changed == 1 {
            tx.execute("UPDATE meta SET revision=revision+1", []).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(changed == 1)
    }

    pub fn delivery(&self, ids: &[String], state: &str) -> Result<()> {
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        for id in ids {
            tx.execute(
                "UPDATE messages SET delivery=?1 WHERE id=?2",
                params![state, id],
            )
            .map_err(err)?;
        }
        tx.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        tx.commit().map_err(err)
    }
    /// Stores the execution status of a builtin agent or a registered instance
    /// and reports whether it was actually written. A session can run under an
    /// `agent_instances` id that has no `agents` row yet, so that row is created
    /// instead of updating nothing. Unknown ids are not invented as rows and
    /// leave the revision untouched.
    pub fn status(&self, agent: &str, status: &str, detail: &str) -> Result<bool> {
        if agent.trim().is_empty() {
            return Ok(false);
        }
        let db = self.db.lock().map_err(err)?;
        let known: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM agents WHERE id=?1 UNION ALL SELECT 1 FROM agent_instances WHERE id=?1)",
                params![agent],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !known {
            return Ok(false);
        }
        db.execute(
            "INSERT OR IGNORE INTO agents(id,status,detail,updated_at) VALUES(?1,'idle','대기 중',?2)",
            params![agent, timestamp()],
        )
        .map_err(err)?;
        let rows = db
            .execute(
                "UPDATE agents SET status=?1,detail=?2,updated_at=?3 WHERE id=?4",
                params![status, detail, timestamp(), agent],
            )
            .map_err(err)?;
        if rows == 0 {
            return Ok(false);
        }
        db.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        Ok(true)
    }
    pub fn audit(&self, kind: &str, payload: Value) -> Result<()> {
        self.db
            .lock()
            .map_err(err)?
            .execute(
                "INSERT INTO audit(created_at,kind,payload) VALUES(?1,?2,?3)",
                params![timestamp(), kind, payload.to_string()],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn reserve_tool(&self, key: &str, id: &Value) -> Result<bool> {
        let rows = self
            .db
            .lock()
            .map_err(err)?
            .execute(
                "INSERT OR IGNORE INTO tool_deliveries VALUES(?1,?2,'prepared',?3)",
                params![key, id.to_string(), timestamp()],
            )
            .map_err(err)?;
        Ok(rows == 1)
    }
    /// Returns whether a delivery row actually matched. An update that touches no row
    /// is not a success: the caller would otherwise record a state that was never stored.
    pub fn tool_state(&self, key: &str, state: &str) -> Result<bool> {
        let changed = self
            .db
            .lock()
            .map_err(err)?
            .execute(
                "UPDATE tool_deliveries SET state=?1,updated_at=?2 WHERE call_key=?3",
                params![state, timestamp(), key],
            )
            .map_err(err)?;
        Ok(changed == 1)
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let db = self.db.lock().map_err(err)?;
        let mut stmt=db.prepare("SELECT sequence,id,speaker,content,created_at,delivery,version,is_error,channel,kind,room_id,thread_id,turn_id,collaboration_id,chat_session_id FROM messages ORDER BY sequence").map_err(err)?;
        let messages = stmt
            .query_map([], |r| {
                Ok(Message {
                    sequence: r.get(0)?,
                    id: r.get(1)?,
                    speaker: r.get(2)?,
                    content: r.get(3)?,
                    created_at: r.get(4)?,
                    delivery: r.get(5)?,
                    version: r.get(6)?,
                    is_error: r.get(7)?,
                    channel: r.get(8)?,
                    kind: r.get(9)?,
                    room_id: r.get(10)?,
                    thread_id: r.get(11)?,
                    turn_id: r.get(12)?,
                    collaboration_id: r.get(13)?,
                    chat_session_id: r.get(14)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        let mut stmt = db
            .prepare("SELECT id,status,detail,updated_at FROM agents ORDER BY id")
            .map_err(err)?;
        let agents = stmt
            .query_map([], |r| {
                Ok(Agent {
                    id: r.get(0)?,
                    status: r.get(1)?,
                    detail: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        let (version, revision) = db
            .query_row("SELECT version,revision FROM meta", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(err)?;
        let sessions = crate::workspace::sessions(&db)?;
        let rooms = crate::workspace::rooms(&db)?;
        Ok(Snapshot {
            contexts: crate::context::read(&db)?,
            active_run: None,
            messages,
            agents,
            conversation_version: version,
            revision,
            sessions,
            rooms,
            busy_session_id: self.busy_session.lock().map_err(err)?.clone(),
        })
    }
    /// Reads the queued requirements a pending review still has to deliver and
    /// the session version they belong to in one lock. Reading them separately
    /// let a message submitted between the two reads land in one half of the
    /// envelope only: `latestUserRequirements` advertised a version whose
    /// `messages` list did not contain it, or the reverse.
    pub fn pending_requirements(&self, session: &str) -> Result<(i64, Vec<Message>)> {
        let db = self.db.lock().map_err(err)?;
        let version: i64 = db
            .query_row("SELECT version FROM chat_sessions WHERE id=?1", [session], |r| r.get(0))
            .map_err(|_| "세션을 찾을 수 없습니다".to_string())?;
        let mut stmt = db
            .prepare("SELECT sequence,id,speaker,content,created_at,delivery,version,is_error,channel,kind,room_id,thread_id,turn_id,collaboration_id,chat_session_id FROM messages WHERE speaker='user' AND chat_session_id=?1 AND delivery='queued' AND substr(content,1,6)!='@arch ' ORDER BY sequence")
            .map_err(err)?;
        let messages = stmt
            .query_map([session], |r| {
                Ok(Message {
                    sequence: r.get(0)?,
                    id: r.get(1)?,
                    speaker: r.get(2)?,
                    content: r.get(3)?,
                    created_at: r.get(4)?,
                    delivery: r.get(5)?,
                    version: r.get(6)?,
                    is_error: r.get(7)?,
                    channel: r.get(8)?,
                    kind: r.get(9)?,
                    room_id: r.get(10)?,
                    thread_id: r.get(11)?,
                    turn_id: r.get(12)?,
                    collaboration_id: r.get(13)?,
                    chat_session_id: r.get(14)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok((version, messages))
    }
    pub fn cancel_queued(&self, session: &str) -> Result<()> {
        let mut db = self.db.lock().map_err(err)?;
        let tx = db.transaction().map_err(err)?;
        tx.execute("UPDATE messages SET delivery='cancelled' WHERE chat_session_id=?1 AND speaker='user' AND delivery='queued'", [session]).map_err(err)?;
        tx.execute("UPDATE meta SET revision=revision+1", []).map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn update_stream(&self, id: &str, text: &str, delivery: &str, channel: &str, kind: &str) -> Result<()> {
        let mut db=self.db.lock().map_err(err)?;
        let tx=db.transaction().map_err(err)?;
        tx.execute("UPDATE messages SET content=?1,delivery=?2,channel=?3,kind=?4 WHERE id=?5 AND speaker='codex'",params![text,delivery,channel,kind,id]).map_err(err)?;
        tx.execute("UPDATE meta SET revision=revision+1",[]).map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn context(&self) -> Result<String> {
        let s = self.snapshot()?;
        let local_id = self.execution_session_id()?;
        Ok(serde_json::to_string(&json!({"conversationVersion":self.session_version(&local_id)?,"messages":s.messages.iter().filter(|m|m.chat_session_id==local_id && m.delivery!="cancelled").rev().take(60).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>()})).map_err(err)?)
    }
    pub fn execution_session_id(&self) -> Result<String> {
        Ok(self.execution_session.lock().map_err(err)?.clone())
    }
    pub fn set_execution_session(&self, id: &str) -> Result<()> {
        *self.execution_session.lock().map_err(err)? = id.into();
        Ok(())
    }
    pub fn set_busy_session(&self, id: Option<String>) -> Result<()> {
        *self.busy_session.lock().map_err(err)? = id;
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<()> {
        self.db
            .lock()
            .map_err(err)?
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(err)
    }
}

fn migrate_channels(db: &mut Connection) -> Result<()> {
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(err)?;
    if version >= 1 {
        return Ok(());
    }
    let tx = db.transaction().map_err(err)?;
    tx.execute_batch("ALTER TABLE messages ADD COLUMN channel TEXT NOT NULL DEFAULT 'main';
        ALTER TABLE messages ADD COLUMN kind TEXT NOT NULL DEFAULT 'answer';
        ALTER TABLE messages ADD COLUMN room_id TEXT NOT NULL DEFAULT 'default';
        ALTER TABLE messages ADD COLUMN thread_id TEXT;
        ALTER TABLE messages ADD COLUMN turn_id TEXT;
        ALTER TABLE messages ADD COLUMN collaboration_id TEXT;
        UPDATE messages SET kind='user_message' WHERE speaker='user';
        UPDATE messages SET channel='system' WHERE speaker='system';
        UPDATE messages SET channel='legacy' WHERE speaker='antigravity' OR (speaker='user' AND content LIKE '@arch %');
        CREATE INDEX IF NOT EXISTS messages_route ON messages(room_id,channel,thread_id,sequence);").map_err(err)?;
    let events = {
        let mut q = tx
            .prepare("SELECT created_at,kind,payload FROM audit ORDER BY sequence")
            .map_err(err)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        rows
    };
    let mut calls = std::collections::HashSet::new();
    for (at, kind, payload) in events {
        let Ok(v) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };
        if kind == "codex_rx"
            && v["method"] == "item/completed"
            && v["params"]["item"]["type"] == "agentMessage"
        {
            let p = &v["params"];
            let item = &p["item"];
            let channel = if item["phase"] == "commentary" {
                "activity"
            } else {
                "main"
            };
            if let Some(text) = item["text"].as_str() {
                tx.execute("UPDATE messages SET channel=?1,kind=?2,thread_id=?3,turn_id=?4 WHERE id=(SELECT id FROM messages WHERE speaker='codex' AND content=?5 AND thread_id IS NULL ORDER BY sequence LIMIT 1)",params![channel,if channel=="activity"{"commentary"}else{"answer"},p["threadId"].as_str(),p["turnId"].as_str(),text]).map_err(err)?;
            }
        }
        if kind == "codex_rx"
            && v["method"] == "item/tool/call"
            && v["params"]["tool"] == "request_arch_review"
        {
            let p = &v["params"];
            let Some(question) = p["arguments"]["question"].as_str() else {
                continue;
            };
            let first = calls.insert((p["threadId"].to_string(), p["turnId"].to_string()));
            let request_kind = if first {
                "review_request"
            } else {
                "review_followup"
            };
            tx.execute("INSERT INTO messages(id,speaker,content,created_at,delivery,version,is_error,channel,kind,room_id,thread_id,turn_id,collaboration_id) VALUES(?1,'codex',?2,?3,'delivered',0,0,'collaboration',?4,'default',?5,?6,?7)",params![uuid::Uuid::new_v4().to_string(),question,at,request_kind,p["threadId"].as_str(),p["turnId"].as_str(),p["callId"].as_str()]).map_err(err)?;
        }
        if kind == "review_delivered" {
            if let Some(answer) = v["envelope"]["archReview"]["answer"].as_str() {
                tx.execute("UPDATE messages SET channel='collaboration',kind='review_reply',thread_id=?1,turn_id=?2,collaboration_id=?3 WHERE id=(SELECT id FROM messages WHERE speaker='antigravity' AND content=?4 AND collaboration_id IS NULL ORDER BY sequence LIMIT 1)",params![v["threadId"].as_str(),v["turnId"].as_str(),v["callId"].as_str(),answer]).map_err(err)?;
            }
        }
    }
    // Attach legacy user messages to the next recorded Codex turn, without guessing their audience.
    tx.execute("UPDATE messages SET thread_id=(SELECT c.thread_id FROM messages c WHERE c.speaker='codex' AND c.channel IN ('main','activity') AND c.thread_id IS NOT NULL AND c.sequence>messages.sequence ORDER BY c.sequence LIMIT 1),turn_id=(SELECT c.turn_id FROM messages c WHERE c.speaker='codex' AND c.channel IN ('main','activity') AND c.thread_id IS NOT NULL AND c.sequence>messages.sequence ORDER BY c.sequence LIMIT 1) WHERE speaker='user' AND channel='main' AND thread_id IS NULL",[]).map_err(err)?;
    tx.execute_batch("PRAGMA user_version=1;").map_err(err)?;
    tx.commit().map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_queue_preserves_history_other_sessions_and_new_messages() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("cancel.db")).unwrap();
        let other = s.create_session("default", "other").unwrap();
        let delivered = s.message("user", "already sent", false).unwrap();
        s.delivery(&[delivered.id.clone()], "delivered").unwrap();
        let pending = s.message("user", "cancel this queued input", false).unwrap();
        s.set_execution_session(&other.id).unwrap();
        let untouched = s.message("user", "other queued input", false).unwrap();
        s.cancel_queued("default-session").unwrap();
        s.set_execution_session("default-session").unwrap();
        let fresh = s.message("user", "new request after cancellation", false).unwrap();
        let snap = s.snapshot().unwrap();
        for (id, state) in [(delivered.id,"delivered"), (pending.id,"cancelled"), (untouched.id,"queued"), (fresh.id,"queued")] {
            assert_eq!(snap.messages.iter().find(|m|m.id==id).unwrap().delivery, state);
        }
        assert!(!s.context().unwrap().contains("cancel this queued input"));
        assert!(s.context().unwrap().contains("new request after cancellation"));
    }
    #[test]
    fn status_reports_whether_it_was_stored() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("status.db")).unwrap();

        assert!(s.status("codex", "running", "실행 중").unwrap());
        assert_eq!(
            s.snapshot().unwrap().agents.iter().find(|a| a.id == "codex").unwrap().status,
            "running"
        );

        s.db.lock()
            .unwrap()
            .execute(
                "INSERT INTO agent_instances(id,provider,name,connection_kind,role,source,created_at) VALUES('inst-open-1','opencode','OpenCode CLI','not_configured','lead','user','2026-10-25T00:00:00Z')",
                [],
            )
            .unwrap();
        assert!(s.status("inst-open-1", "running", "실행 중").unwrap());
        assert_eq!(
            s.snapshot().unwrap().agents.iter().find(|a| a.id == "inst-open-1").unwrap().status,
            "running"
        );

        let before = s.snapshot().unwrap().revision;
        assert!(!s.status("ghost-agent", "running", "실행 중").unwrap());
        assert!(!s.status("   ", "running", "실행 중").unwrap());
        let after = s.snapshot().unwrap();
        assert!(after.agents.iter().all(|a| a.id != "ghost-agent"));
        assert_eq!(after.revision, before);
    }

    #[test]
    fn pending_requirements_share_one_version_and_skip_arch_and_foreign_sessions() {
        // The review envelope pairs this version with this list, so they have to
        // come from the same read. Direct `@arch` questions belong to the legacy
        // channel and other sessions' queued input must never travel with them.
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("requirements.db")).unwrap();
        let other = s.create_session("default", "other").unwrap();
        let first = s.message("user", "first requirement", false).unwrap();
        let arch = s
            .routed_message(
                "user",
                "@arch direct question",
                false,
                Route {
                    channel: "legacy",
                    kind: "user_message",
                    chat_session_id: Some("default-session".into()),
                    ..Route::default()
                },
            )
            .unwrap();
        let foreign = s
            .routed_message(
                "user",
                "other session work",
                false,
                Route {
                    channel: "main",
                    kind: "user_message",
                    chat_session_id: Some(other.id.clone()),
                    ..Route::default()
                },
            )
            .unwrap();
        let (version, pending) = s.pending_requirements("default-session").unwrap();
        assert_eq!(version, s.session_version("default-session").unwrap());
        assert_eq!(pending.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), [first.id.as_str()]);
        assert!(pending.iter().all(|m| m.id != arch.id && m.id != foreign.id));
        // A delivered requirement stops being pending, but the version keeps growing.
        s.delivery(&[first.id.clone()], "delivered").unwrap();
        let (after_version, after) = s.pending_requirements("default-session").unwrap();
        assert!(after.is_empty());
        assert_eq!(after_version, version);
        assert!(s.pending_requirements("missing-session").is_err());
    }

    #[test]
    fn open_drops_status_rows_whose_instance_is_gone() {
        // A registration removed by an older build could leave its `agents` row
        // behind. Those rows are not user data, they only inflate the Agents badge
        // (`snapshot.agents.length`), so reopening the store removes them while
        // keeping the two builtin rows and every instance that still exists.
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("orphans.db");
        {
            let s = Store::open(&p).unwrap();
            {
                let db = s.db.lock().unwrap();
                db.execute("INSERT INTO agent_instances(id,provider,name,connection_kind,role,source,created_at) VALUES('inst-keep','claude_code','Claude Code','not_configured','lead','user','2026-10-25T00:00:00Z')", []).unwrap();
                db.execute("INSERT INTO agents(id,status,detail,updated_at) VALUES('inst-keep','idle','대기 중','2026-10-25T00:00:00Z')", []).unwrap();
                db.execute("INSERT INTO agents(id,status,detail,updated_at) VALUES('inst-orphan','running','실행 중','2026-10-25T00:00:00Z')", []).unwrap();
            }
            assert_eq!(s.snapshot().unwrap().agents.len(), 4);
        }
        let s = Store::open(&p).unwrap();
        let first = s.snapshot().unwrap();
        let ids = first.agents.iter().map(|a| a.id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids, vec!["antigravity", "codex", "inst-keep"]);
        assert!(first.messages.is_empty());
        drop(s);
        // Reopening is idempotent and keeps the rows that still have an owner.
        let s = Store::open(&p).unwrap();
        assert_eq!(s.snapshot().unwrap().agents.len(), 3);
    }

    #[test]
    fn restores_messages_and_marks_inflight_unknown() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("chat.sqlite");
        {
            let s = Store::open(&p).unwrap();
            s.message("user", "요청", false).unwrap();
            s.message("codex", "답변", false).unwrap();
            s.status("codex", "waiting_for_arch", "검토 대기").unwrap();
        }
        let s = Store::open(&p).unwrap();
        let snap = s.snapshot().unwrap();
        assert_eq!(snap.messages.len(), 2);
        assert!(snap.messages[0].sequence < snap.messages[1].sequence);
        assert_eq!(
            snap.agents.iter().find(|a| a.id == "codex").unwrap().status,
            "unknown"
        );
        assert!(s.reserve_tool("key", &json!(0)).unwrap());
        assert!(!s.reserve_tool("key", &json!(0)).unwrap());
    }

    #[test]
    fn legacy_history_migration_preserves_records_and_uses_protocol_channels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.sqlite");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch("CREATE TABLE messages(sequence INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT NOT NULL UNIQUE,speaker TEXT NOT NULL,content TEXT NOT NULL,created_at TEXT NOT NULL,delivery TEXT NOT NULL,version INTEGER NOT NULL,is_error INTEGER NOT NULL); CREATE TABLE audit(sequence INTEGER PRIMARY KEY AUTOINCREMENT,created_at TEXT NOT NULL,kind TEXT NOT NULL,payload TEXT NOT NULL);").unwrap();
            for (id, speaker, text) in [
                ("u", "user", "검토해줘"),
                ("progress", "codex", "검토할게"),
                ("review", "antigravity", "실제 리뷰"),
                ("final", "codex", "최종 답변"),
                ("old-user", "user", "@arch 이전 직접 질문"),
                ("old-arch", "antigravity", "이전 직접 답변"),
                ("unanswered", "user", "아직 답이 없는 요청"),
            ] {
                db.execute("INSERT INTO messages(id,speaker,content,created_at,delivery,version,is_error) VALUES(?1,?2,?3,'2026-10-18T00:01:00Z','delivered',1,0)",params![id,speaker,text]).unwrap();
            }
            let events = [
                (
                    "codex_session",
                    json!({"threadId":"thread-1","sessionId":"session-1"}),
                ),
                (
                    "codex_rx",
                    json!({"method":"item/completed","params":{"threadId":"thread-1","turnId":"turn-1","item":{"type":"agentMessage","phase":"commentary","text":"검토할게"}}}),
                ),
                (
                    "codex_rx",
                    json!({"method":"item/tool/call","params":{"threadId":"thread-1","turnId":"turn-1","callId":"call-1","tool":"request_arch_review","arguments":{"question":"실제 검토 요청"}}}),
                ),
                (
                    "review_delivered",
                    json!({"threadId":"thread-1","turnId":"turn-1","callId":"call-1","envelope":{"archReview":{"answer":"실제 리뷰"}}}),
                ),
                (
                    "codex_rx",
                    json!({"method":"item/tool/call","params":{"threadId":"thread-1","turnId":"turn-1","callId":"call-2","tool":"request_arch_review","arguments":{"question":"실제 후속 질문"}}}),
                ),
                (
                    "codex_rx",
                    json!({"method":"item/completed","params":{"threadId":"thread-1","turnId":"turn-1","item":{"type":"agentMessage","phase":"final","text":"최종 답변"}}}),
                ),
            ];
            for (kind, payload) in events {
                db.execute("INSERT INTO audit(created_at,kind,payload) VALUES('2026-10-18T00:00:00Z',?1,?2)",params![kind,payload.to_string()]).unwrap();
            }
        }
        let store = Store::open(&path).unwrap();
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.messages.len(), 9);
        for (index, id) in ["u", "progress", "review", "final", "old-user", "old-arch"]
            .iter()
            .enumerate()
        {
            let message = &snapshot.messages[index];
            assert_eq!(&message.id, id);
            assert_eq!(message.sequence, index as i64 + 1);
            assert_eq!(message.created_at, "2026-10-18T00:01:00Z");
        }
        assert_eq!(snapshot.messages[0].channel, "main");
        assert_eq!(snapshot.messages[0].thread_id.as_deref(), Some("thread-1"));
        assert_eq!(snapshot.messages[1].channel, "activity");
        assert_eq!(snapshot.messages[2].channel, "collaboration");
        assert_eq!(
            snapshot.messages[2].collaboration_id.as_deref(),
            Some("call-1")
        );
        assert_eq!(snapshot.messages[3].channel, "main");
        assert_eq!(snapshot.messages[4].channel, "legacy");
        assert_eq!(snapshot.messages[5].channel, "legacy");
        assert!(snapshot.messages[6].thread_id.is_none());
        assert_eq!(snapshot.messages[7].kind, "review_request");
        assert_eq!(snapshot.messages[8].kind, "review_followup");
        assert_eq!(
            snapshot
                .sessions
                .iter()
                .find(|s| s.id == "thread-1")
                .unwrap()
                .session_id,
            "session-1"
        );
        drop(store);
        assert_eq!(
            Store::open(&path)
                .unwrap()
                .snapshot()
                .unwrap()
                .messages
                .len(),
            9
        );
    }

    #[test]
        fn channel_routing_preserves_version_and_call_correlation() {
            let dir = tempfile::tempdir().unwrap();
            let store = Store::open(&dir.path().join("chat.sqlite")).unwrap();
            let user = store.message("user", "검토해줘", false).unwrap();
            let request = store
                .routed_message(
                    "codex",
                    "질문",
                    false,
                    Route {
                        channel: "collaboration",
                        kind: "review_request",
                        thread_id: Some("thread".into()),
                        turn_id: Some("turn".into()),
                        collaboration_id: Some("call".into()),
                        chat_session_id: None,
                    },
                )
                .unwrap();
            assert_eq!(user.version, request.version);
            assert_eq!(request.channel, "collaboration");
            assert_eq!(request.collaboration_id.as_deref(), Some("call"));
            store.bind_messages(&[user.id], "thread", "turn").unwrap();
            assert!(store
                .snapshot()
                .unwrap()
                .messages
                .iter()
                .all(|m| m.thread_id.as_deref() == Some("thread")));
        }
        #[test]
                fn closing_foreign_rpc_restores_previous_execution_session() {
                // The worker's execution_session is a scratch cursor for context().
                // Closing a foreign session's RPC must not leak its id forward.
                let dir = tempfile::tempdir().unwrap();
                let store = Store::open(&dir.path().join("close.sqlite")).unwrap();
                let a = store.create_session("default", "A").unwrap();
                let b = store.create_session("default", "B").unwrap();
                store.bind_codex_session(&a.id, "thread-a", "rpc-a").unwrap();
                store.bind_codex_session(&b.id, "thread-b", "rpc-b").unwrap();
                store.set_execution_session(&a.id).unwrap();
                let _ = store.message("user", "hello from A", false).unwrap();
                store.set_execution_session(&b.id).unwrap();
                let _ = store.message("user", "hello from B", false).unwrap();
                // Simulate: worker is on B, then a message for A arrives and closes B's RPC.
                store.set_execution_session(&a.id).unwrap();
                assert_eq!(store.execution_session_id().unwrap(), a.id);
                let ctx = store.context().unwrap();
                assert!(ctx.contains("hello from A"));
                assert!(!ctx.contains("hello from B"));
            }
}
