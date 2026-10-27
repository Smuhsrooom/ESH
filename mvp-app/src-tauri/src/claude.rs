//! Claude Code Lead protocol skeleton and turn runner.
//!
//! This module defines the invocation contract, the stream-json result
//! classification and the one-process-per-turn runner for the `claude_code_cli`
//! Lead adapter. `Engine::claude_turn` calls `run` for a session whose provider
//! is `claude_code`, `Store::create_claude_session` creates one from a
//! registration that carries `CONNECTION_KIND`, and `agents::ClaudeCodeAdapter`
//! now advertises that connection. A registration only receives the kind when
//! its executable sits at the native install layout (`.local/bin/claude.exe`),
//! so a registration stored anywhere else stays `not_configured` and cannot own
//! a session or answer a turn. No chat UI offers the Claude Lead as a session
//! agent yet, so a turn is reachable through the commands, not through the
//! screen.
//!
//! Evidence boundary:
//! - The flags and values in `command_args` are checked against the help text
//!   the installed CLI printed on this machine, committed verbatim as
//!   `fixtures/claude-cli-2.1.304-help.txt` (captured 2026-10-25).
//! - A real `claude -p` run (2026-10-27, approved) could not reach the API: the
//!   user's `ANTHROPIC_BASE_URL` pointed at a local port with no server. Its
//!   failure stream is kept as `fixtures/claude-cli-2.1.304-api-error-stream.jsonl`
//!   (init trimmed of local paths, other lines verbatim): about 180 s of
//!   `system/api_retry` events, then an `is_error` result, then exit code 1. The
//!   failure and timeout messages below are built on that capture.
//! - The success-path classification is still written from the public
//!   headless/SDK docs and has **not** been confirmed against a real successful
//!   stream on this machine.
//! - `run` is exercised by tests against a stub child process, which proves the
//!   process plumbing (capture, exit code, cancel, timeout) but not the real CLI.
#![allow(dead_code)]

use crate::{
    cli,
    store::{err, Result},
};
use serde_json::Value;
use std::{
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command, sync::watch, time::timeout};

/// One turn is one process, so these bound a single answer rather than a session.
const TURN_TIMEOUT: Duration = Duration::from_secs(180);
const STDOUT_LIMIT: u64 = 4_194_305;
const STDERR_LIMIT: u64 = 65_537;

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

/// The prompt is the trailing positional argument, as the installed help
/// declares (`Usage: claude [options] [command] [prompt]`).
pub fn turn_args(existing: Option<&str>, model: Option<&str>, prompt: &str) -> Vec<String> {
    let mut args = command_args(existing, model);
    args.push(prompt.to_string());
    args
}

/// Runs one conversation-only turn and returns the classified answer.
///
/// The caller keeps the saved session ID and passes it back as `existing` so the
/// CLI resumes that conversation; a changed session ID fails the turn instead of
/// silently starting a new conversation.
pub async fn run(
    path: &Path,
    work_dir: &Path,
    existing: Option<&str>,
    model: Option<&str>,
    prompt: &str,
    stop: watch::Receiver<bool>,
) -> Result<Answer> {
    if !path.is_file() {
        return Err("등록된 Claude Code CLI를 찾을 수 없습니다".into());
    }
    if prompt.trim().is_empty() {
        return Err("Claude Code에 보낼 요청이 비어 있습니다".into());
    }
    let mut command = Command::new(path);
    cli::hidden(&mut command);
    command
        .current_dir(work_dir)
        .args(turn_args(existing, model, prompt))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    collect(command, existing, stop, TURN_TIMEOUT).await
}

/// Waits for the child while capturing both pipes.
///
/// Cancel and timeout kill the whole tree before returning, so a stopped turn
/// cannot leave an orphan process behind. stderr is read only to detect the size
/// limit and is never shown or logged: it can carry internal context.
async fn collect(
    mut command: Command,
    expected: Option<&str>,
    mut stop: watch::Receiver<bool>,
    limit: Duration,
) -> Result<Answer> {
    let mut child = command.spawn().map_err(|e| format!("Claude Code 시작 실패: {e}"))?;
    let stdout = child.stdout.take().ok_or("Claude Code stdout이 없습니다")?;
    let stderr = child.stderr.take().ok_or("Claude Code stderr가 없습니다")?;
    let out = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.take(STDOUT_LIMIT).read_to_end(&mut bytes).await.map_err(err)?;
        Ok::<Vec<u8>, String>(bytes)
    });
    let err_out = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.take(STDERR_LIMIT).read_to_end(&mut bytes).await.map_err(err)?;
        Ok::<Vec<u8>, String>(bytes)
    });
    let status = tokio::select! {
        _ = stop.changed() => {
            cli::stop_tree(&mut child).await?;
            return Err("Claude Code 실행 중단".into());
        },
        result = timeout(limit, child.wait()) => match result {
            Ok(value) => value.map_err(err)?,
            Err(_) => {
                cli::stop_tree(&mut child).await?;
                // Killing the tree closes stdout, so whatever the CLI printed while
                // it was still retrying the API stays readable. The short wait keeps
                // a pipe held open elsewhere from hanging the turn.
                let captured = match timeout(Duration::from_secs(2), out).await {
                    Ok(Ok(Ok(bytes))) => Some(bytes),
                    _ => None,
                };
                return Err(timeout_message(captured.as_deref()));
            }
        },
    };
    let output = out.await.map_err(err)??;
    let errors = err_out.await.map_err(err)??;
    if output.len() as u64 >= STDOUT_LIMIT || errors.len() as u64 >= STDERR_LIMIT {
        return Err("Claude Code 출력 크기 제한을 초과했습니다".into());
    }
    if !status.success() {
        // A failed request still ends with a stream-json error result on stdout,
        // and that public text explains the failure better than the exit code.
        if let Some(reason) = error_result(&output) {
            return Err(format!("Claude 실행 오류: {reason}"));
        }
        let code = status.code().map(|code| code.to_string()).unwrap_or_else(|| "알 수 없음".into());
        return Err(format!("Claude Code가 종료 코드 {code}로 실패했습니다. CLI 로그를 확인해 주세요"));
    }
    parse_events(&output, expected)
}

/// The CLI's own error result, if the stream carries one. The text comes from
/// the `result` field on stdout, the same field `parse_events` shows for an
/// error result, never from stderr.
fn error_result(output: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(output).ok()?;
    text.lines().rev().find_map(|line| {
        let value: Value = serde_json::from_str(line.trim()).ok()?;
        if value["type"] != "result" || value["is_error"] != true {
            return None;
        }
        let reason = value["result"].as_str().map(str::trim).filter(|reason| !reason.is_empty());
        Some(reason.unwrap_or("알 수 없는 오류").to_string())
    })
}

/// How many `system/api_retry` events the CLI printed and the HTTP status of
/// the last one (`None` when no HTTP response arrived at all). Only these two
/// values leave this function; the rest of each event is not shown.
fn api_retries(output: &[u8]) -> Option<(usize, Option<i64>)> {
    let mut count = 0;
    let mut status = None;
    for line in String::from_utf8_lossy(output).lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else { continue };
        if value["type"] == "system" && value["subtype"] == "api_retry" {
            count += 1;
            status = value["error_status"].as_i64();
        }
    }
    (count > 0).then_some((count, status))
}

/// The CLI retries a failing API call for about as long as `TURN_TIMEOUT`
/// (measured 2026-10-27: ten retries, ~180 s), so a plain timeout would hide
/// that the turn was stuck reconnecting. Name the retries when the output
/// shows them.
fn timeout_message(captured: Option<&[u8]>) -> String {
    match captured.and_then(api_retries) {
        Some((count, Some(status))) => format!(
            "Claude Code 응답 시간 초과: Claude API 요청이 HTTP {status}로 실패해 {count}회 재시도하던 중이었습니다"
        ),
        Some((count, None)) => format!(
            "Claude Code 응답 시간 초과: Claude API 응답을 받지 못해 {count}회 재시도하던 중이었습니다. 네트워크·프록시·API 주소 설정을 확인해 주세요"
        ),
        None => "Claude Code 응답 시간 초과".into(),
    }
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
    fn a_real_api_error_stream_fails_with_its_result_text_and_keeps_the_resumed_session() {
        // Captured from a real `claude -p … --resume` turn on 2026-10-27 whose API
        // connection was refused. The init line is trimmed to drop local paths; the
        // retry, assistant and result lines are verbatim. This is the failure path
        // only: a successful real answer has not been captured yet.
        const STREAM: &str = include_str!("../fixtures/claude-cli-2.1.304-api-error-stream.jsonl");
        const REAL: &str = "6e71e4ec-e2f3-4fd5-a818-d2599d26e00e";
        for expected in [None, Some(REAL)] {
            let message = parse_events(STREAM.as_bytes(), expected).unwrap_err();
            assert!(message.starts_with("Claude 실행 오류: API Error: Connection refused"), "{message}");
        }
        assert!(parse_events(STREAM.as_bytes(), Some(SESSION)).unwrap_err().contains("세션 ID"));
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

    #[test]
    fn the_prompt_is_the_trailing_positional_argument() {
        let args = turn_args(Some(SESSION), Some("claude-sonnet-4-5"), "안녕하세요");
        let expected = command_args(Some(SESSION), Some("claude-sonnet-4-5"));
        assert_eq!(args.len(), expected.len() + 1);
        assert_eq!(&args[..expected.len()], expected.as_slice());
        assert_eq!(args.last().unwrap(), "안녕하세요");
        // The prompt stays a positional argument, never a flag value, so a prompt
        // that looks like a flag cannot change the invocation.
        assert!(!args[..expected.len()].iter().any(|arg| arg == "안녕하세요"));
    }

    /// A stub child process is enough to inspect the real spawn/capture path
    /// without calling any model. It only proves the plumbing.
    #[cfg(windows)]
    fn stub(script: &str) -> Command {
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_stub_stream_is_captured_and_classified_through_the_real_process_path() {
        let script = format!(
            "[Console]::WriteLine('{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"{SESSION}\"}}');\
             [Console]::WriteLine('{{\"type\":\"assistant\",\"session_id\":\"{SESSION}\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"draft\"}}]}}}}');\
             [Console]::WriteLine('{{\"type\":\"result\",\"is_error\":false,\"session_id\":\"{SESSION}\",\"result\":\"stub answer\"}}')"
        );
        let (_stop, stopped) = watch::channel(false);
        let answer = collect(stub(&script), None, stopped, Duration::from_secs(60)).await.unwrap();
        assert_eq!(answer, Answer { session_id: SESSION.into(), text: "stub answer".into() });
        // The same output fails when the saved session is not the one that answered.
        let (_stop, stopped) = watch::channel(false);
        let mismatch = collect(
            stub(&script),
            Some("22222222-2222-4222-8222-222222222222"),
            stopped,
            Duration::from_secs(60),
        )
        .await
        .unwrap_err();
        assert!(mismatch.contains("세션 ID가 일치하지 않습니다"), "{mismatch}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_failed_exit_reports_the_code_without_leaking_stderr() {
        let (_stop, stopped) = watch::channel(false);
        let message = collect(
            stub("[Console]::Error.WriteLine('secret-context-value'); exit 3"),
            None,
            stopped,
            Duration::from_secs(60),
        )
        .await
        .unwrap_err();
        assert!(message.contains("종료 코드"), "{message}");
        assert!(message.contains('3'), "{message}");
        // stderr can carry internal context, so it must not reach the caller.
        assert!(!message.contains("secret-context-value"), "{message}");
    }

    const REAL_FAILURE: &str = include_str!("../fixtures/claude-cli-2.1.304-api-error-stream.jsonl");

    #[test]
    fn the_real_failure_stream_yields_its_error_text_and_its_retry() {
        let reason = error_result(REAL_FAILURE.as_bytes()).expect("the captured stream ends with an error result");
        assert!(reason.starts_with("API Error: Connection refused") && reason.ends_with("(ECONNREFUSED)"), "{reason}");
        // The captured retry had no HTTP response, so it carries no status.
        assert_eq!(api_retries(REAL_FAILURE.as_bytes()), Some((1, None)));
        // A successful stream has no error result and no retries to report.
        let ok = line(serde_json::json!({"type":"result","is_error":false,"session_id":SESSION,"result":"ok"}));
        assert_eq!(error_result(ok.as_bytes()), None);
        assert_eq!(api_retries(ok.as_bytes()), None);
        assert_eq!(timeout_message(None), "Claude Code 응답 시간 초과");
        assert_eq!(timeout_message(Some(ok.as_bytes())), "Claude Code 응답 시간 초과");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_real_failed_exit_reports_the_cli_error_result_instead_of_the_code() {
        // Replays the captured stdout bytes unchanged and exits 1, as the real CLI did.
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/claude-cli-2.1.304-api-error-stream.jsonl");
        let script = format!(
            "$b=[IO.File]::ReadAllBytes('{fixture}'); $o=[Console]::OpenStandardOutput(); $o.Write($b,0,$b.Length); $o.Flush();\
             [Console]::Error.WriteLine('secret-context-value'); exit 1"
        );
        let (_stop, stopped) = watch::channel(false);
        let message = collect(stub(&script), None, stopped, Duration::from_secs(60)).await.unwrap_err();
        assert!(message.starts_with("Claude 실행 오류: API Error: Connection refused"), "{message}");
        assert!(!message.contains("종료 코드"), "{message}");
        assert!(!message.contains("secret-context-value"), "{message}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_timeout_while_the_cli_retries_names_the_retries() {
        // Retry events in the shape the real CLI printed, then the CLI keeps
        // retrying past the deadline.
        let retry = |attempt: u32, status: &str| {
            format!(
                "[Console]::WriteLine('{{\"type\":\"system\",\"subtype\":\"api_retry\",\"attempt\":{attempt},\"max_retries\":10,\"retry_delay_ms\":594,\"error_status\":{status},\"error\":\"unknown\",\"session_id\":\"{SESSION}\"}}');"
            )
        };
        let unreachable = format!("{}{} Start-Sleep -Seconds 30", retry(1, "null"), retry(2, "null"));
        let overloaded = format!("{}{} Start-Sleep -Seconds 30", retry(1, "null"), retry(2, "529"));
        let (_first, first) = watch::channel(false);
        let (_second, second) = watch::channel(false);
        let limit = Duration::from_secs(6);
        let (unreachable, overloaded) = tokio::join!(
            collect(stub(&unreachable), None, first, limit),
            collect(stub(&overloaded), None, second, limit),
        );
        let unreachable = unreachable.unwrap_err();
        assert!(unreachable.starts_with("Claude Code 응답 시간 초과"), "{unreachable}");
        assert!(unreachable.contains("응답을 받지 못해 2회"), "{unreachable}");
        let overloaded = overloaded.unwrap_err();
        assert!(overloaded.contains("HTTP 529") && overloaded.contains("2회"), "{overloaded}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn a_cancelled_or_slow_turn_stops_the_child_instead_of_waiting() {
        let script = "[Console]::WriteLine('{\"type\":\"system\"}'); Start-Sleep -Seconds 30";
        let (stop, stopped) = watch::channel(false);
        stop.send(true).unwrap();
        let cancelled = collect(stub(script), None, stopped, Duration::from_secs(60)).await.unwrap_err();
        assert!(cancelled.contains("중단"), "{cancelled}");
        let (_stop, stopped) = watch::channel(false);
        let expired = collect(stub(script), None, stopped, Duration::from_millis(200)).await.unwrap_err();
        assert!(expired.contains("시간 초과"), "{expired}");
    }

    #[tokio::test]
    async fn a_missing_cli_or_an_empty_request_fails_before_any_process_starts() {
        let dir = tempfile::tempdir().unwrap();
        let (_stop, stopped) = watch::channel(false);
        let missing = run(&dir.path().join("claude.exe"), dir.path(), None, None, "질문", stopped).await.unwrap_err();
        assert!(missing.contains("찾을 수 없습니다"), "{missing}");
        // The placeholder file is never executed: the empty request is refused first.
        let placeholder = dir.path().join("claude.exe");
        std::fs::write(&placeholder, b"do not execute").unwrap();
        let (_stop, stopped) = watch::channel(false);
        let empty = run(&placeholder, dir.path(), None, None, "   ", stopped).await.unwrap_err();
        assert!(empty.contains("비어 있습니다"), "{empty}");
    }
}
