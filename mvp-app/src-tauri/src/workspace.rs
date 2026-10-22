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

pub fn sessions(db: &Connection) -> Result<Vec<Session>> {
    let mut q=db.prepare("SELECT id,codex_session_id,created_at,room_id,name,thread_id,source_thread_id,mode,model,reasoning_effort FROM chat_sessions ORDER BY created_at,id").map_err(err)?;
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
    pub fn bind_codex_session(&self, id: &str, thread: &str, session: &str) -> Result<()> {
        let db = self.db.lock().map_err(err)?;
        db.execute(
            "UPDATE chat_sessions SET thread_id=?1,codex_session_id=?2 WHERE id=?3",
            params![thread, session, id],
        )
        .map_err(err)?;
        db.execute("UPDATE meta SET revision=revision+1", [])
            .map_err(err)?;
        Ok(())
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
}
