//! OpenCode conversation adapter. One CLI process runs one turn; the saved session ID resumes it.
use crate::{cli, store::{err, Result}};
use serde_json::Value;
use serde::Serialize;
use std::{path::{Path, PathBuf}, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command, sync::watch, time::timeout};

pub struct Answer { pub session_id: String, pub text: String }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeModel { pub model: String }

fn parse_models(output: &[u8]) -> Result<Vec<OpenCodeModel>> {
    let output = std::str::from_utf8(output).map_err(|_| "OpenCode 모델 목록 인코딩 오류")?;
    let mut models = Vec::new();
    for line in output.lines() {
        let id = line.strip_suffix('\r').unwrap_or(line);
        if id.is_empty() { continue; }
        if id.chars().any(|c| c.is_whitespace() || c.is_control())
            || id.split('/').count() < 2
            || id.split('/').any(str::is_empty) {
            return Err("OpenCode 모델 목록 형식을 확인할 수 없습니다".into());
        }
        if !models.iter().any(|m: &OpenCodeModel| m.model == id) {
            models.push(OpenCodeModel { model: id.into() });
        }
    }
    if models.is_empty() { return Err("OpenCode 모델 목록이 비어 있습니다".into()); }
    Ok(models)
}

pub async fn list_models(path: &Path, work_dir: &Path) -> Result<Vec<OpenCodeModel>> {
    if !path.is_file() { return Err("등록된 OpenCode CLI를 찾을 수 없습니다".into()); }
    let mut command = Command::new(path);
    cli::hidden(&mut command);
    command.current_dir(work_dir).args(["models", "--pure"])
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
    let output = timeout(Duration::from_secs(20), command.output()).await
        .map_err(|_| "OpenCode 모델 목록 조회 시간 초과")?.map_err(err)?;
    if !output.status.success() { return Err("OpenCode 모델 목록 조회 실패".into()); }
    if output.stdout.len() > 1_048_576 { return Err("OpenCode 모델 목록 크기 제한 초과".into()); }
    parse_models(&output.stdout)
}

fn parse_events(output: &[u8], expected: Option<&str>) -> Result<Answer> {
    let mut session = None::<String>;
    let mut parts = Vec::new();
    let mut finished = false;
    for line in String::from_utf8_lossy(output).lines() {
        let value: Value = serde_json::from_str(line).map_err(|_| "OpenCode JSON 응답을 읽을 수 없습니다".to_string())?;
        let id = value["sessionID"].as_str().ok_or("OpenCode 세션 ID가 없습니다")?;
        if expected.is_some_and(|previous| previous != id) || session.as_deref().is_some_and(|previous| previous != id) {
            return Err("OpenCode 응답의 세션 ID가 일치하지 않습니다".into());
        }
        session = Some(id.into());
        if matches!(value["part"]["type"].as_str(), Some("tool" | "tool-use" | "tool_use")) {
            return Err("OpenCode가 허용되지 않은 도구 호출을 시도했습니다".into());
        }
        match value["type"].as_str().unwrap_or_default() {
            "text" => if let Some(text) = value["part"]["text"].as_str() { parts.push(text.to_string()); },
            "step_finish" => {
                if value["part"]["reason"] == "stop" { finished = true; }
            }
            "error" => return Err(format!("OpenCode 실행 오류: {}", value["error"]["message"].as_str().unwrap_or("알 수 없는 오류"))),
            _ => {}
        }
    }
    if !finished || parts.is_empty() { return Err("OpenCode의 완료 답변을 확인하지 못했습니다".into()); }
    Ok(Answer { session_id: session.ok_or("OpenCode 세션 ID가 없습니다")?, text: parts.join("\n") })
}

pub async fn run(path: &Path, work_dir: &Path, existing: Option<&str>, model: Option<&str>, prompt: &str, mut stop: watch::Receiver<bool>) -> Result<Answer> {
    if !path.is_file() { return Err("등록된 OpenCode CLI를 찾을 수 없습니다".into()); }
    let path = PathBuf::from(path);
    let mut command = Command::new(path);
    cli::hidden(&mut command);
    command.current_dir(work_dir)
        .args(["run", "--pure", "--format", "json", "--agent", "plan", "--dir"])
        .arg(work_dir);
    if let Some(id) = existing { command.args(["--session", id]); }
    else { command.args(["--title", "ESH 대화"]); }
    add_model_arg(&mut command, model);
    command.arg(format!("사용자에게 한국어 존댓말로 답하세요. 도구를 사용하거나 파일·명령을 실행하지 마세요.\n사용자 요청: {prompt}"))
        .env("OPENCODE_CONFIG_CONTENT", r#"{"permission":{"*":"deny"}}"#)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut child = command.spawn().map_err(|e| format!("OpenCode 시작 실패: {e}"))?;
    let stdout = child.stdout.take().ok_or("OpenCode stdout이 없습니다")?;
    let stderr = child.stderr.take().ok_or("OpenCode stderr가 없습니다")?;
    let out = tokio::spawn(async move { let mut bytes = Vec::new(); stdout.take(4_194_305).read_to_end(&mut bytes).await.map_err(err)?; Ok::<Vec<u8>,String>(bytes) });
    let err_out = tokio::spawn(async move { let mut bytes = Vec::new(); stderr.take(65_537).read_to_end(&mut bytes).await.map_err(err)?; Ok::<Vec<u8>,String>(bytes) });
    let status = tokio::select! {
        _ = stop.changed() => { cli::stop_tree(&mut child).await?; return Err("OpenCode 실행 중단".into()); },
        result = timeout(Duration::from_secs(180), child.wait()) => match result {
            Ok(value) => value.map_err(err)?,
            Err(_) => { cli::stop_tree(&mut child).await?; return Err("OpenCode 응답 시간 초과".into()); }
        },
    };
    let output = out.await.map_err(err)??;
    let errors = err_out.await.map_err(err)??;
    if output.len() > 4_194_304 || errors.len() > 65_536 { return Err("OpenCode 출력 크기 제한을 초과했습니다".into()); }
    if !status.success() {
        // stderr can contain internal context; keep it out of the GUI and audit.
        return Err(format!("OpenCode가 종료 코드 {:?}로 실패했습니다. CLI 로그를 확인해 주세요", status.code()));
    }
    parse_events(&output, existing)
}

fn add_model_arg(command: &mut Command, model: Option<&str>) {
    if let Some(model) = model { command.args(["--model", model]); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_list_accepts_nested_model_ids_and_rejects_invalid_segments() {
        let models = parse_models(b"openrouter/free\nnvidia/deepseek-ai/deepseek-r1\nopencode/beam-free\nnvidia/deepseek-ai/deepseek-r1\n").unwrap();
        assert_eq!(models.iter().map(|item| item.model.as_str()).collect::<Vec<_>>(),
            ["openrouter/free", "nvidia/deepseek-ai/deepseek-r1", "opencode/beam-free"]);
        for invalid in ["warning: unavailable", "provider", "/model", "provider/", "provider//model", "provider/model/", "provider/model name", " provider/model", "provider/model ", "provider/\tmodel"] {
            assert!(parse_models(format!("{invalid}\n").as_bytes()).is_err(), "{invalid}");
        }
        assert!(parse_models(b"\n").is_err());
    }
    #[test]
    fn explicit_model_is_passed_only_when_selected() {
        let mut default = Command::new("opencode");
        add_model_arg(&mut default, None);
        assert_eq!(default.as_std().get_args().count(), 0);
        let mut selected = Command::new("opencode");
        add_model_arg(&mut selected, Some("nvidia/deepseek-ai/deepseek-r1"));
        let args = selected.as_std().get_args().map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>();
        assert_eq!(args, ["--model", "nvidia/deepseek-ai/deepseek-r1"]);
    }
    #[test]
    fn accepts_only_finished_matching_session_with_public_text() {
        let good = br#"{"type":"text","sessionID":"ses_a","part":{"text":"hello"}}
{"type":"step_finish","sessionID":"ses_a","part":{"reason":"stop"}}
"#;
        assert_eq!(parse_events(good, None).unwrap().text, "hello");
        assert!(parse_events(good, Some("ses_b")).is_err());
        assert!(parse_events(br#"{"type":"text","sessionID":"ses_a","part":{"text":"hello"}}"#, None).is_err());
    }
}
