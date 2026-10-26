#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod cli;
mod agents;
#[cfg(test)]
mod mock_agent;
mod registry;
mod opencode;
mod claude;
mod engine;
mod models;
mod store;
mod stream;
mod context;
mod workspace;
mod window_state;
#[cfg(test)]
mod characterization_tests;

use agents::{registered, AgentProbe, LeadAdapter, LeadSession, CODEX};
use engine::Engine;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use store::{Result, Snapshot};
use tauri::Manager;

#[tauri::command]
fn get_snapshot(engine: tauri::State<'_, Arc<Engine>>) -> Result<Snapshot> {
    engine.snapshot()
}

#[tauri::command]
fn probe_agents() -> Vec<AgentProbe> {
    registered().iter().map(|agent| agent.probe()).collect()
}

#[tauri::command]
fn detect_agent_candidates() -> Vec<registry::Candidate> { registry::discover() }

#[tauri::command]
fn list_agent_instances(engine: tauri::State<'_, Arc<Engine>>) -> Result<Vec<registry::Instance>> {
    engine.store.list_agent_instances()
}

#[tauri::command]
fn register_agent_instance(provider: String, path: String, role: String, engine: tauri::State<'_, Arc<Engine>>) -> Result<registry::Instance> {
    let instance = registry::register_instance(&engine.store, engine.admission(), &provider, &path, &role)?;
    // Registration writes an `agents` row, so the open WebView needs the new
    // snapshot instead of waiting for the next unrelated chat event.
    engine.emit()?;
    Ok(instance)
}

#[tauri::command]
fn unregister_agent_instance(id: String, engine: tauri::State<'_, Arc<Engine>>) -> Result<()> {
    registry::unregister_instance(&engine.store, engine.admission(), &id)?;
    // Removal deletes that row; without the emit the Agents view keeps showing
    // the removed instance's last status until another event arrives.
    engine.emit()
}

#[tauri::command]
fn cancel_run(session_id: String, run_id: String, engine: tauri::State<'_, Arc<Engine>>) -> Result<Snapshot> {
    engine.cancel(&session_id, &run_id)
}

#[tauri::command]
async fn list_codex_models(engine: tauri::State<'_, Arc<Engine>>) -> Result<Vec<models::CodexModel>> {
    engine.ensure_accepting()?;
    let mut rpc = CODEX.connect(engine.store.clone()).await?;
    let result = async { rpc.handshake().await?; rpc.list_models().await }.await;
    let closed = LeadSession::close(&mut rpc).await;
    let models = result?;
    closed?;
    engine.cache_models(models.clone())?;
    Ok(models)
}

#[tauri::command]
async fn list_opencode_models(session_id: String, engine: tauri::State<'_, Arc<Engine>>) -> Result<Vec<opencode::OpenCodeModel>> {
    engine.ensure_accepting()?;
    engine.list_opencode_models(&session_id).await
}

#[tauri::command]
async fn set_opencode_model(session_id: String, model: Option<String>, engine: tauri::State<'_, Arc<Engine>>) -> Result<Snapshot> {
    engine.set_opencode_model(&session_id, model).await
}

#[tauri::command]
fn compact_session(session_id: String, engine: tauri::State<'_, Arc<Engine>>) -> Result<Snapshot> {
    engine.request_compact(&session_id)
}

#[tauri::command]
fn set_session_model_settings(session_id: String, settings: models::ModelSettings, engine: tauri::State<'_, Arc<Engine>>) -> Result<Snapshot> {
    engine.set_model_settings(&session_id, &settings)
}

#[tauri::command]
fn send_message(
    text: String,
    target: String,
    session_id: Option<String>,
    engine: tauri::State<'_, Arc<Engine>>,
) -> Result<Snapshot> {
    engine.submit(text, target, session_id)
}

#[tauri::command]
fn create_room(name: String, engine: tauri::State<'_, Arc<Engine>>) -> Result<workspace::Room> {
    engine.ensure_accepting()?;
    let room = engine.store.create_room(&name)?;
    engine.emit()?;
    Ok(room)
}
#[tauri::command]
fn create_session(
    room_id: String,
    name: String,
    agent_instance_id: Option<String>,
    engine: tauri::State<'_, Arc<Engine>>,
) -> Result<store::Session> {
    engine.ensure_accepting()?;
    // The registration decides the session kind: the front end only passes the
    // selected agent id, and every CLI Lead owns a different provider.
    let session = if let Some(id) = agent_instance_id {
        engine.store.create_agent_session(&room_id, &name, &id)?
    } else { engine.store.create_session(&room_id, &name)? };
    engine.emit()?;
    Ok(session)
}
#[tauri::command]
async fn list_codex_sessions(
    cursor: Option<String>,
    search: Option<String>,
    engine: tauri::State<'_, Arc<Engine>>,
) -> Result<Value> {
    engine.ensure_accepting()?;
    let mut rpc = CODEX.connect(engine.store.clone()).await?;
    let result = async {
        rpc.handshake().await?;
        rpc.list_threads(cursor, search).await
    }
    .await;
    let closed = LeadSession::close(&mut rpc).await;
    match result {
        Ok(v) => {
            closed?;
            Ok(v)
        }
        Err(e) => Err(e),
    }
}
#[tauri::command]
async fn import_codex_session(
    room_id: String,
    thread_id: String,
    name: String,
    mode: String,
    engine: tauri::State<'_, Arc<Engine>>,
) -> Result<store::Session> {
    if mode != "copy" && mode != "resume" {
        return Err("가져오기 방식을 선택해 주세요".into());
    }
    engine.ensure_accepting()?;
    let mut rpc = CODEX.connect(engine.store.clone()).await?;
    let result = async {
        rpc.handshake().await?;
        rpc.read_history(&thread_id).await
    }
    .await;
    let _ = LeadSession::close(&mut rpc).await;
    let (thread, mut history) = result?;
    let snapshot = engine.store.snapshot()?;
    if let Some(local) = snapshot
        .sessions
        .iter()
        .find(|s| s.thread_id.as_deref() == Some(&thread_id))
    {
        history=snapshot.messages.iter().filter(|m|m.chat_session_id==local.id && ["main","activity"].contains(&m.channel.as_str())).map(|m|serde_json::json!({"speaker":m.speaker,"content":m.content,"createdAt":m.created_at,"turnId":m.turn_id,"phase":if m.channel=="activity"{"commentary"}else{"final"}})).collect();
    }
    if mode == "resume" && thread["status"]["type"] == "active" {
        return Err(
            "현재 실행 중인 원본 세션입니다. 완료 후 추가하거나 기록 가져오기를 선택해 주세요.".into(),
        );
    }
    let session = engine
        .store
        .add_session(&room_id, &name, Some(&thread_id), &mode, &history)?;
    engine.emit()?;
    Ok(session)
}

#[tauri::command]
fn get_probe_mode() -> Option<String> {
    if cfg!(debug_assertions) {
        std::env::var("ESH_GUI_PROBE").ok()
    } else {
        None
    }
}

#[tauri::command]
fn get_registry_probe_path() -> Option<String> {
    if cfg!(debug_assertions) && std::env::var_os("ESH_GUI_PROBE").as_deref() == Some(std::ffi::OsStr::new("registry")) {
        std::env::var("ESH_REGISTRY_PROBE_EXE").ok()
    } else { None }
}

// Only active in debug test launches; records what the real WebView rendered.
#[tauri::command]
fn record_gui_probe(payload: Value, engine: tauri::State<'_, Arc<Engine>>) -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("ESH_GUI_PROBE").is_none() {
        return Err("GUI 검증 모드가 아닙니다".into());
    }
    let path = engine.data_dir.join("gui-probe.json");
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&payload).map_err(store::err)?,
    )
    .map_err(store::err)
}

#[tauri::command]
async fn close_probe(app: tauri::AppHandle, engine: tauri::State<'_, Arc<Engine>>) -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("ESH_GUI_PROBE").is_none() {
        return Err("GUI 검증 모드가 아닙니다".into());
    }
    engine.shutdown().await?;
    app.exit(0);
    Ok(())
}

fn main() {
    let closing = Arc::new(AtomicBool::new(false));
    tauri::Builder::default()
        // Must stay the first plugin: it decides whether this process owns the
        // app before any other plugin or setup work touches the shared WebView2
        // user data folder. A second launch only surfaces the running window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let probe_dir = if cfg!(debug_assertions) {
                std::env::args_os().collect::<Vec<_>>().windows(2)
                    .find(|args| args[0] == "--esh-data-dir")
                    .map(|args| PathBuf::from(&args[1]))
            } else { None };
            let data_dir = probe_dir.or_else(|| std::env::var_os("ESH_DATA_DIR").map(PathBuf::from))
                .unwrap_or(app.path().app_local_data_dir()?);
            let window_data_dir = data_dir.clone();
            let engine =
                Engine::start(app.handle().clone(), data_dir).map_err(std::io::Error::other)?;
            app.manage(engine);
            if let Some(window) = app.get_webview_window("main") {
                window_state::restore(&window, &window_data_dir);
                // Show explicitly instead of trusting the window config: a
                // `visible: false` (or a future default change) would otherwise
                // leave a running app with no window the user can reach. Focus
                // stays untouched here so launching does not steal it.
                let _ = window.show();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            probe_agents,
            detect_agent_candidates,
            list_agent_instances,
            register_agent_instance,
            unregister_agent_instance,
            cancel_run,
            list_codex_models,
            list_opencode_models,
            set_opencode_model,
            compact_session,
            set_session_model_settings,
            send_message,
            create_room,
            create_session,
            list_codex_sessions,
            import_codex_session,
            get_probe_mode,
            get_registry_probe_path,
            record_gui_probe,
            close_probe
        ])
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if closing.swap(true, Ordering::SeqCst) {
                    return;
                }
                let app = window.app_handle().clone();
                let engine = app.state::<Arc<Engine>>().inner().clone();
                let geometry = window_state::capture(window);
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = engine.shutdown().await {
                        let _ = engine.store.status("codex", "unknown", &e);
                        let _ = engine.store.checkpoint();
                    }
                    if let Some(geometry) = geometry {
                        let _ = window_state::save(&engine.data_dir, &geometry);
                    }
                    app.exit(0);
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("ESH 앱 시작에 실패했습니다");
}
