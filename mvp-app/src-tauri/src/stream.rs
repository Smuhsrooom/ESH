use crate::store::{Result, Route, Store};
use std::{collections::{HashMap, HashSet}, time::{Duration, Instant}};

struct Draft {
    thread: String,
    turn: String,
    session: String,
    channel: &'static str,
    text: String,
    message_id: Option<String>,
    flushed_at: Option<Instant>,
}

#[derive(Default)]
pub struct Streams {
    drafts: HashMap<String, Draft>,
    completed: HashSet<String>,
}

impl Streams {
    pub fn start(&mut self, item: &str, phase: Option<&str>, thread: &str, turn: &str, session: &str) {
        if self.completed.contains(item) { return; }
        self.drafts.entry(item.into()).or_insert_with(|| Draft {
            thread: thread.into(), turn: turn.into(), session: session.into(),
            // Unknown audience stays in progress history until the final item classifies it.
            channel: if phase == Some("final_answer") { "main" } else { "activity" },
            text: String::new(), message_id: None, flushed_at: None,
        });
    }
    pub fn delta(&mut self, store: &Store, item: &str, delta: &str) -> Result<bool> {
        let Some(draft) = self.drafts.get_mut(item) else { return Ok(false); };
        if delta.is_empty() { return Ok(false); }
        draft.text.push_str(delta);
        if draft.flushed_at.is_some_and(|at| at.elapsed() < Duration::from_millis(40)) { return Ok(false); }
        Self::flush(store, draft, "streaming")?;
        Ok(true)
    }
    pub fn complete(&mut self, store: &Store, item: &str, text: &str, phase: Option<&str>, thread: &str, turn: &str, session: &str) -> Result<bool> {
        if self.completed.contains(item) { return Ok(false); }
        self.start(item, phase, thread, turn, session);
        let draft = self.drafts.get_mut(item).unwrap();
        draft.channel = if phase == Some("commentary") { "activity" } else { "main" };
        if !text.trim().is_empty() { draft.text = text.into(); }
        let visible = !draft.text.is_empty();
        if visible { Self::flush(store, draft, if text.trim().is_empty() { "interrupted" } else { "delivered" })?; }
        self.drafts.remove(item);
        self.completed.insert(item.into());
        Ok(visible)
    }
    pub fn interrupt(&mut self, store: &Store) -> Result<usize> {
        let mut count = 0;
        for draft in self.drafts.values_mut() {
            if !draft.text.is_empty() { Self::flush(store, draft, "interrupted")?; count += 1; }
        }
        self.drafts.clear();
        self.completed.clear();
        Ok(count)
    }
    fn flush(store: &Store, draft: &mut Draft, delivery: &str) -> Result<()> {
        let kind = if draft.channel == "main" { "answer" } else { "commentary" };
        if let Some(id) = &draft.message_id {
            store.update_stream(id, &draft.text, delivery, draft.channel, kind)?;
        } else {
            let message = store.routed_message("codex", &draft.text, false, Route {
                channel: draft.channel,
                kind: if draft.channel == "main" { "answer_stream" } else { "commentary_stream" },
                thread_id: Some(draft.thread.clone()), turn_id: Some(draft.turn.clone()),
                chat_session_id: Some(draft.session.clone()), ..Route::default()
            })?;
            draft.message_id = Some(message.id.clone());
            store.update_stream(&message.id, &draft.text, delivery, draft.channel, kind)?;
        }
        draft.flushed_at = Some(Instant::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streams_finalize_once_and_preserve_interrupted_tail() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("stream.db")).unwrap();
        let mut stream = Streams::default();
        assert!(!stream.delta(&s, "unregistered-reasoning", "hidden").unwrap());
        stream.start("a", Some("final_answer"), "thread", "turn", "default-session");
        stream.delta(&s,"a","partial ").unwrap();
        let first = s.snapshot().unwrap().messages[0].clone();
        assert_eq!(first.delivery,"streaming");
        stream.complete(&s,"a","authoritative final",Some("final_answer"),"thread","turn","default-session").unwrap();
        assert!(!stream.complete(&s,"a","duplicate",Some("final_answer"),"thread","turn","default-session").unwrap());
        let messages=s.snapshot().unwrap().messages;
        assert_eq!(messages.len(),1);
        assert_eq!(messages[0].id,first.id);
        assert_eq!(messages[0].content,"authoritative final");
        assert_eq!(messages[0].delivery,"delivered");
        stream.start("b",Some("final_answer"),"thread","next","default-session");
        stream.delta(&s,"b","kept ").unwrap();
        stream.delta(&s,"b","buffered tail").unwrap();
        stream.interrupt(&s).unwrap();
        let messages=s.snapshot().unwrap().messages;
        assert_eq!(messages[1].content,"kept buffered tail");
        assert_eq!(messages[1].delivery,"interrupted");
    }
    #[test]
    fn commentary_is_separate_and_crash_marks_draft_interrupted() {
        let d=tempfile::tempdir().unwrap(); let path=d.path().join("crash.db");
        let s=Store::open(&path).unwrap(); let mut stream=Streams::default();
        stream.start("progress",Some("commentary"),"t","turn","default-session");
        stream.delta(&s,"progress","public progress").unwrap();
        assert_eq!(s.snapshot().unwrap().messages[0].channel,"activity");
        drop(s);
        let s=Store::open(&path).unwrap();
        assert_eq!(s.snapshot().unwrap().messages[0].delivery,"interrupted");
    }
}
