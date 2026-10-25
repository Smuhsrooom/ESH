//! Claude Code Lead protocol skeleton.
//!
//! This module defines the invocation contract and the stream-json result
//! classification for a future `claude_code_cli` Lead adapter. Nothing here is
//! registered or executed yet: the adapter, the session path and the Engine
//! wiring are a later change, so the items stay unused until then.
//!
//! Evidence boundary:
//! - The flags and values in `command_args` are checked against the help text
//!   the installed CLI printed on this machine, committed verbatim as
//!   `fixtures/claude-cli-2.1.304-help.txt` (captured 2026-10-25).
//! - The event classification is written from the public headless/SDK docs and
//!   has **not** been confirmed against a real `claude -p` stream on this
//!   machine. A real run sends a prompt to an external API, so it needs
//!   approval first; until then this parser is unverified protocol shape, not a
//!   working connection.
#![allow(dead_code)]

use crate::store::Result;
use serde_json::Value;

/// Connection kind a Claude Code registration will carry once it is wired.
pub const CONNECTION_KIND: &str = "claude_code_cli";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub session_id: String,
    pub text: String,
}

/// Conversation-only invocation: one process per turn, newline-delimited JSON.
///
/// `--permission-mode plan` is the conservative choice among the modes the
/// installed CLI declares, but the tool policy for this adapter has not been
/// verified yet and is not settled by this function.
pub fn command_args(existing: Option<&str>, model: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = ["-p", "--output-format", "stream-json", "--verbose", "--permission-mode", "plan"]
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    if let Some(id) = existing {
        args.push("--resume".into());
        args.push(id.into());
    }
    if let Some(model) = model {
        args.push("--model".into());
        args.push(model.into());
    }
    args
}

/// Classify one turn's stream-json output.
///
/// The last `result` line is authoritative for the answer text; the assistant
/// text blocks are only a fallback, because the `result` payload shape is
/// documented for `--output-format json` and assumed for `stream-json`.
pub fn parse_events(output: &[u8], expected: Option<&str>) -> Result<Answer> {
    let text = std::str::from_utf8(output).map_err(|_| "Claude 응답 인코딩 오류".to_string())?;
    let mut session: Option<String> = None;
    let mut parts: Vec<String> = Vec::new();
    let mut result: Option<Value> = None;
    let mut events = 0usize;
    for line in text.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|_| "Claude stream-json 응답을 읽을 수 없습니다".to_string())?;
        let kind = value["type"].as_str().ok_or("Claude 이벤트 종류가 없습니다")?;
        events += 1;
        if let Some(id) = value["session_id"].as_str() {
            if expected.is_some_and(|previous| previous != id)
                || session.as_deref().is_some_and(|previous| previous != id)
            {
                return Err("Claude 응답의 세션 ID가 일치하지 않습니다".into());
            }
            session.get_or_insert_with(|| id.to_string());
        }
        match kind {
            "assistant" => {
                for block in value["message"]["content"].as_array().into_iter().flatten() {
                    match block["type"].as_str().unwrap_or_default() {
                        // The adapter is conversation-only: a tool call means the
                        // turn left the contract, so fail instead of answering.
                        "tool_use" => return Err("Claude가 허용되지 않은 도구 호출을 시도했습니다".into()),
                        "text" => {
                            if let Some(chunk) = block["text"].as_str().filter(|chunk| !chunk.is_empty()) {
                                parts.push(chunk.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            "result" => {
                if result.is_some() {
                    return Err("Claude 결과 이벤트가 중복되었습니다".into());
                }
                result = Some(value);
            }
            // `system`/`user`/`stream_event` carry no public answer here, and an
            // unknown type is ignored so a newer CLI does not break the turn.
            _ => {}
        }
    }
    if events == 0 {
        return Err("Claude 응답이 비어 있습니다".into());
    }
    let result = result.ok_or("Claude 완료 결과를 확인하지 못했습니다")?;
    if result["is_error"].as_bool() == Some(true) {
        return Err(format!(
            "Claude 실행 오류: {}",
            result["result"].as_str().unwrap_or("알 수 없는 오류")
        ));
    }
    let answer = result["result"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let joined = parts.join("\n");
            if joined.trim().is_empty() { None } else { Some(joined) }
        })
        .ok_or("Claude의 완료 답변을 확인하지 못했습니다")?;
    Ok(Answer { session_id: session.ok_or("Claude 세션 ID가 없습니다")?, text: answer })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The help text the installed CLI (2.1.304) printed on this machine.
    const HELP: &str = include_str!("../fixtures/claude-cli-2.1.304-help.txt");
    const SESSION: &str = "11111111-1111-4111-8111-111111111111";

    fn line(value: Value) -> String {
        value.to_string()
    }

    #[test]
    fn the_invocation_contract_only_uses_flags_the_installed_cli_declares() {
        let args = command_args(Some(SESSION), Some("claude-sonnet-4-5"));
        // Every flag ESH passes must be one this binary documents, otherwise the
        // adapter would be built on a guessed interface.
        for flag in ["-p", "--output-format", "--verbose", "--permission-mode", "--resume", "--model"] {
            assert!(HELP.contains(flag), "{flag} is not declared by the installed CLI");
            assert!(args.iter().any(|arg| arg == flag), "{flag} is missing from the invocation");
        }
        assert!(HELP.contains("stream-json"), "stream-json is not a declared output format");
        assert!(HELP.contains("\"plan\""), "plan is not a declared permission mode");
        assert_eq!(
            args,
            ["-p", "--output-format", "stream-json", "--verbose", "--permission-mode", "plan",
                "--resume", SESSION, "--model", "claude-sonnet-4-5"]
        );
    }

    #[test]
    fn a_first_turn_resumes_nothing_and_uses_the_default_model() {
        let args = command_args(None, None);
        assert_eq!(args, ["-p", "--output-format", "stream-json", "--verbose", "--permission-mode", "plan"]);
        assert!(!args.iter().any(|arg| arg == "--resume" || arg == "--model"));
    }

    #[test]
    fn the_result_line_is_the_answer_and_the_session_must_not_change() {
        let output = [
            line(serde_json::json!({"type":"system","subtype":"init","session_id":SESSION})),
            line(serde_json::json!({"type":"assistant","session_id":SESSION,
                "message":{"content":[{"type":"text","text":"draft"}]}})),
            line(serde_json::json!({"type":"result","subtype":"success","is_error":false,
                "session_id":SESSION,"result":"final answer"})),
        ]
        .join("\n");
        let answer = parse_events(output.as_bytes(), Some(SESSION)).unwrap();
        assert_eq!(answer, Answer { session_id: SESSION.into(), text: "final answer".into() });
        assert!(parse_events(output.as_bytes(), Some("22222222-2222-4222-8222-222222222222")).is_err());
    }

    #[test]
    fn an_assistant_text_block_is_only_the_fallback_for_the_answer() {
        let output = [
            line(serde_json::json!({"type":"assistant","session_id":SESSION,
                "message":{"content":[{"type":"text","text":"first"},{"type":"text","text":"second"}]}})),
            line(serde_json::json!({"type":"result","session_id":SESSION,"result":""})),
        ]
        .join("\n");
        assert_eq!(parse_events(output.as_bytes(), None).unwrap().text, "first\nsecond");
    }

    #[test]
    fn an_error_result_or_a_tool_call_fails_instead_of_answering() {
        let error = line(serde_json::json!({"type":"result","is_error":true,"session_id":SESSION,
            "result":"rate limited"}));
        let message = parse_events(error.as_bytes(), None).unwrap_err();
        assert!(message.contains("rate limited"), "{message}");
        let tool = [
            line(serde_json::json!({"type":"assistant","session_id":SESSION,
                "message":{"content":[{"type":"tool_use","name":"Bash"}]}})),
            line(serde_json::json!({"type":"result","session_id":SESSION,"result":"done"})),
        ]
        .join("\n");
        assert!(parse_events(tool.as_bytes(), None).unwrap_err().contains("도구 호출"));
    }

    #[test]
    fn a_truncated_or_duplicated_stream_is_rejected() {
        let missing_result = line(serde_json::json!({"type":"system","session_id":SESSION}));
        assert!(parse_events(missing_result.as_bytes(), None).is_err());
        let no_session = line(serde_json::json!({"type":"result","result":"answer"}));
        assert!(parse_events(no_session.as_bytes(), None).is_err());
        let duplicated = [
            line(serde_json::json!({"type":"result","session_id":SESSION,"result":"a"})),
            line(serde_json::json!({"type":"result","session_id":SESSION,"result":"b"})),
        ]
        .join("\n");
        assert!(parse_events(duplicated.as_bytes(), None).is_err());
        assert!(parse_events(b"", None).is_err());
        assert!(parse_events(b"not json", None).is_err());
    }

    #[test]
    fn unknown_event_types_and_crlf_do_not_break_a_valid_turn() {
        let output = [
            line(serde_json::json!({"type":"system","subtype":"init","session_id":SESSION})),
            line(serde_json::json!({"type":"rate_limit_event","session_id":SESSION})),
            line(serde_json::json!({"type":"result","session_id":SESSION,"result":"ok"})),
        ]
        .join("\r\n");
        assert_eq!(parse_events(output.as_bytes(), None).unwrap().text, "ok");
    }
}
