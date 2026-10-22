use crate::store::{err, timestamp, Result, Store};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_output_tokens: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub last: Tokens,
    pub total: Tokens,
    pub model_context_window: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub session_id: String,
    pub thread_id: String,
    pub usage: Option<Usage>,
    pub usage_updated_at: Option<String>,
    pub usage_stale: bool,
    pub compaction: String,
    pub compaction_item_id: Option<String>,
    pub compaction_count: u64,
    pub compacted_at: Option<String>,
}
pub fn migrate(db: &mut Connection) -> Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(err)?;
    if version >= 4 { return Ok(()); }
    let tx = db.transaction().map_err(err)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS session_context(session_id TEXT PRIMARY KEY REFERENCES chat_sessions(id),payload TEXT NOT NULL); PRAGMA user_version=4;").map_err(err)?;
    tx.commit().map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn compact(method: &str, id: &str) -> Value {
        json!({"method":method,"params":{"threadId":"thread-a","turnId":"turn","item":{"id":id,"type":"contextCompaction"}}})
    }
    #[test]
    fn usage_and_compaction_preserve_sessions_and_recover_uncertainty() {
        let d = tempfile::tempdir().unwrap(); let path = d.path().join("context.db");
        let s = Store::open(&path).unwrap();
        s.bind_codex_session("default-session","thread-a","rpc").unwrap();
        let b = s.create_session("default","B").unwrap();
        s.bind_codex_session(&b.id,"thread-b","rpc-b").unwrap();
        let tokens = json!({"inputTokens":900,"outputTokens":100,"totalTokens":1000,"cachedInputTokens":400,"reasoningOutputTokens":20});
        let event = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread-a","turnId":"turn","tokenUsage":{"last":tokens,"total":{"inputTokens":90000,"outputTokens":10000,"totalTokens":100000,"cachedInputTokens":0,"reasoningOutputTokens":0},"modelContextWindow":10000}}});
        assert!(!s.observe_context(&b.id,"thread-b",&event).unwrap());
        s.observe_context("default-session","thread-a",&event).unwrap();
        let c = s.snapshot().unwrap().contexts.remove(0);
        assert_eq!(c.usage.as_ref().unwrap().last.total_tokens,1000);
        assert_eq!(c.usage.unwrap().total.total_tokens,100000);
        s.observe_context("default-session","thread-a",&compact("item/started","c1")).unwrap();
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction,"compacting");
        s.observe_context("default-session","thread-a",&compact("item/completed","c1")).unwrap();
        s.observe_context("default-session","thread-a",&compact("item/started","c1")).unwrap();
        s.observe_context("default-session","thread-a",&compact("item/completed","c1")).unwrap();
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction_count,1);
        assert!(s.snapshot().unwrap().contexts[0].usage_stale);
        s.observe_context("default-session","thread-a",&event).unwrap();
        assert!(!s.snapshot().unwrap().contexts[0].usage_stale);
        s.observe_context("default-session","thread-a",&compact("item/started","c2")).unwrap();
        s.finish_context("default-session","cancelled").unwrap();
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction,"cancelled");
        s.observe_context("default-session","thread-a",&compact("item/started","c3")).unwrap();
        drop(s);
        let s=Store::open(&path).unwrap();
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction,"unknown");
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction_count,1);
        assert_eq!(s.snapshot().unwrap().contexts.len(),1);
    }
    #[test]
    fn manual_compact_requires_bound_thread_and_clears_previous_item() {
        let d = tempfile::tempdir().unwrap(); let path = d.path().join("manual.db");
        let s = Store::open(&path).unwrap();
        assert!(s.begin_compact("default-session", "thread-a").is_err());
        s.bind_codex_session("default-session", "thread-a", "rpc").unwrap();
        assert!(s.begin_compact("default-session", "thread-b").is_err());
        assert!(s.begin_compact("default-session", "thread-a").unwrap());
        let state = s.snapshot().unwrap().contexts.remove(0);
        assert_eq!(state.compaction, "compacting");
        assert!(state.usage_stale);
        assert_eq!(state.compaction_item_id, None);
        assert!(!s.begin_compact("default-session", "thread-a").unwrap());
        s.finish_context("default-session", "failed").unwrap();
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction, "failed");
        assert!(s.begin_compact("default-session", "thread-a").unwrap());
        s.finish_context("default-session", "unknown").unwrap();
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction, "unknown");
        assert!(s.begin_compact("default-session", "thread-a").unwrap());
        s.observe_context("default-session", "thread-a", &compact("item/started", "manual-1")).unwrap();
        assert_eq!(s.snapshot().unwrap().contexts[0].compaction_item_id.as_deref(), Some("manual-1"));
        s.observe_context("default-session", "thread-a", &compact("item/completed", "manual-1")).unwrap();
        let state = s.snapshot().unwrap().contexts.remove(0);
        assert_eq!(state.compaction, "completed");
        assert_eq!(state.compaction_count, 1);
    }
    #[test]
    fn migrates_v3_history_and_keeps_unknown_window_and_invalid_usage_honest() {
        let d=tempfile::tempdir().unwrap(); let path=d.path().join("v3.db");
        let s=Store::open(&path).unwrap(); let message=s.message("user","preserve",false).unwrap(); drop(s);
        let db=Connection::open(&path).unwrap(); db.execute_batch("DROP TABLE session_context; PRAGMA user_version=3;").unwrap(); drop(db);
        let s=Store::open(&path).unwrap();
        assert_eq!(s.snapshot().unwrap().messages[0].id,message.id);
        assert!(s.snapshot().unwrap().contexts.is_empty());
        s.bind_codex_session("default-session","thread-a","rpc").unwrap();
        let t=json!({"inputTokens":1,"outputTokens":2,"totalTokens":3,"cachedInputTokens":0,"reasoningOutputTokens":0});
        let mut e=json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread-a","tokenUsage":{"last":t,"total":t,"modelContextWindow":null}}});
        s.observe_context("default-session","thread-a",&e).unwrap();
        assert!(s.snapshot().unwrap().contexts[0].usage.as_ref().unwrap().model_context_window.is_none());
        e["params"]["tokenUsage"]["last"]["totalTokens"]=json!(-1);
        assert!(s.observe_context("default-session","thread-a",&e).is_err());
        assert_eq!(s.snapshot().unwrap().contexts[0].usage.as_ref().unwrap().last.total_tokens,3);
        s.invalidate_context("default-session").unwrap();
        assert!(s.snapshot().unwrap().contexts[0].usage_stale);
    }
}
pub fn read(db: &Connection) -> Result<Vec<Context>> {
    let mut q = db.prepare("SELECT payload FROM session_context ORDER BY session_id").map_err(err)?;
    let rows = q.query_map([], |r| r.get::<_,String>(0)).map_err(err)?;
    rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err)).collect()
}
fn save(db: &Connection, state: &Context) -> Result<()> {
    db.execute("INSERT INTO session_context VALUES(?1,?2) ON CONFLICT(session_id) DO UPDATE SET payload=excluded.payload", params![state.session_id, serde_json::to_string(state).map_err(err)?]).map_err(err)?;
    db.execute("UPDATE meta SET revision=revision+1", []).map_err(err)?;
    Ok(())
}
pub fn recover(db: &Connection) -> Result<()> {
    for mut state in read(db)? {
        if state.compaction == "compacting" { state.compaction = "unknown".into(); save(db,&state)?; }
    }
    Ok(())
}
pub fn is_event(event: &Value) -> bool {
    event["method"] == "thread/tokenUsage/updated" ||
    (matches!(event["method"].as_str(), Some("item/started" | "item/completed")) && event["params"]["item"]["type"] == "contextCompaction")
}
impl Store {
    pub fn observe_context(&self, session: &str, thread: &str, event: &Value) -> Result<bool> {
        if !is_event(event) || event["params"]["threadId"] != thread { return Ok(false); }
        let mut db = self.db.lock().map_err(err)?;
        let bound: Option<String> = db.query_row("SELECT thread_id FROM chat_sessions WHERE id=?1", [session], |r| r.get(0)).map_err(err)?;
        if bound.as_deref() != Some(thread) { return Ok(false); }
        let mut state = read(&db)?.into_iter().find(|s| s.session_id == session && s.thread_id == thread).unwrap_or(Context {
            session_id: session.into(), thread_id: thread.into(), usage: None, usage_updated_at: None,
            usage_stale: false, compaction: "not_observed".into(), compaction_item_id: None, compaction_count: 0, compacted_at: None,
        });
        let before = state.clone();
        let p = &event["params"];
        if event["method"] == "thread/tokenUsage/updated" {
            // Reject malformed/negative counters instead of manufacturing zero usage.
            let usage: Usage = serde_json::from_value(p["tokenUsage"].clone()).map_err(err)?;
            state.usage = Some(usage); state.usage_updated_at = Some(timestamp()); state.usage_stale = false;
        } else {
            let id = p["item"]["id"].as_str().ok_or("압축 item ID가 없습니다")?;
            if event["method"] == "item/started" {
                if state.compaction_item_id.as_deref() != Some(id) {
                    state.compaction = "compacting".into(); state.compaction_item_id = Some(id.into()); state.usage_stale = true;
                }
            } else if state.compaction_item_id.as_deref() != Some(id) || state.compaction != "completed" {
                state.compaction = "completed".into(); state.compaction_item_id = Some(id.into());
                state.compaction_count += 1; state.compacted_at = Some(timestamp()); state.usage_stale = true;
            }
        }
        if state == before { return Ok(false); }
        let tx = db.transaction().map_err(err)?; save(&tx, &state)?; tx.commit().map_err(err)?;
        Ok(true)
    }
    pub fn finish_context(&self, session: &str, status: &str) -> Result<()> {
        let mut db = self.db.lock().map_err(err)?;
        if let Some(mut state) = read(&db)?.into_iter().find(|s| s.session_id == session && s.compaction == "compacting") {
            state.compaction = if ["cancelled","failed"].contains(&status) { status } else { "unknown" }.into();
            let tx = db.transaction().map_err(err)?; save(&tx,&state)?; tx.commit().map_err(err)?;
        }
        Ok(())
    }
    /// Marks a user-requested compaction as started. Returns false when one is already running.
    pub fn begin_compact(&self, session: &str, thread: &str) -> Result<bool> {
        let mut db = self.db.lock().map_err(err)?;
        let bound: Option<String> = db
            .query_row("SELECT thread_id FROM chat_sessions WHERE id=?1", [session], |r| r.get(0))
            .map_err(err)?;
        if bound.as_deref() != Some(thread) {
            return Err("이 세션의 Codex 스레드가 현재 연결과 일치하지 않습니다".into());
        }
        let mut state = read(&db)?.into_iter().find(|s| s.session_id == session && s.thread_id == thread).unwrap_or(Context {
            session_id: session.into(), thread_id: thread.into(), usage: None, usage_updated_at: None,
            usage_stale: true, compaction: "not_observed".into(), compaction_item_id: None, compaction_count: 0, compacted_at: None,
        });
        if state.compaction == "compacting" {
            return Ok(false);
        }
        // Clear the previous item ID so the incoming started event is accepted as this run.
        state.compaction = "compacting".into();
        state.compaction_item_id = None;
        state.usage_stale = true;
        let tx = db.transaction().map_err(err)?; save(&tx, &state)?; tx.commit().map_err(err)?;
        Ok(true)
    }
    pub fn invalidate_context(&self, session: &str) -> Result<()> {
        let db = self.db.lock().map_err(err)?;
        if let Some(mut state) = read(&db)?.into_iter().find(|s| s.session_id == session) {
            state.usage_stale = true; save(&db,&state)?;
        }
        Ok(())
    }
}
