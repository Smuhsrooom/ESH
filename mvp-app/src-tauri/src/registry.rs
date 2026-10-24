//! Read-only CLI discovery and ESH-local registration. This never launches a candidate.
use crate::{agents::{registered, AgentRole, ConnectionKind}, store::{err, timestamp, Result, Store}};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};

const PROVIDERS: [(&str, &str, &str); 3] = [
    ("claude_code", "Claude Code", "claude.exe"),
    ("gemini_cli", "Gemini CLI", "gemini.exe"),
    ("opencode", "OpenCode", "opencode.exe"),
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub provider: String,
    pub name: String,
    pub installation: &'static str,
    pub path: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub path: Option<String>,
    pub connection: String,
    pub role: String,
    pub active_role: Option<&'static str>,
    pub builtin: bool,
    pub support: &'static str,
    pub installation: &'static str,
    pub authentication: &'static str,
    pub availability: &'static str,
    pub execution: &'static str,
    pub checked_at: String,
    pub failure_reason: Option<String>,
}

pub fn migrate(db: &mut Connection) -> Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(err)?;
    if version >= 5 { return Ok(()); }
    let tx = db.transaction().map_err(err)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS agent_instances(
        id TEXT PRIMARY KEY,
        provider TEXT NOT NULL,
        name TEXT NOT NULL,
        executable_path TEXT,
        path_key TEXT,
        connection_kind TEXT NOT NULL,
        role TEXT NOT NULL,
        source TEXT NOT NULL CHECK(source IN ('builtin','user')),
        created_at TEXT NOT NULL,
        UNIQUE(provider,path_key)
    );
    PRAGMA user_version=5;").map_err(err)?;
    for (id, name, connection, role) in [
        ("codex", "Codex Lead", "app_server_stdio_json_rpc", "lead"),
        ("antigravity", "Antigravity Arch", "agy_cli_stream_json", "reviewer"),
    ] {
        tx.execute("INSERT OR IGNORE INTO agent_instances(id,provider,name,connection_kind,role,source,created_at) VALUES(?1,?1,?2,?3,?4,'builtin',?5)",
            params![id, name, connection, role, timestamp()]).map_err(err)?;
    }
    tx.commit().map_err(err)
}

fn provider(provider: &str) -> Result<(&'static str, &'static str)> {
    PROVIDERS.iter().find(|p| p.0 == provider).map(|p| (p.1, p.2))
        .ok_or_else(|| "지원 후보 목록에 없는 에이전트 종류입니다".into())
}

fn active_role(provider: &str, connection: &str, role: &str, builtin: bool, path: Option<&str>) -> Option<&'static str> {
    if !builtin && !(provider == "opencode" && path.is_some_and(|p| p.replace('\\', "/").to_lowercase().ends_with("/node_modules/opencode-ai/bin/opencode.exe"))) { return None; }
    registered().iter().find_map(|adapter| {
        let descriptor = adapter.descriptor();
        let expected_connection = match descriptor.connection {
            ConnectionKind::AppServerStdioJsonRpc => "app_server_stdio_json_rpc",
            ConnectionKind::AgyCliStreamJson => "agy_cli_stream_json",
            ConnectionKind::OpenCodeCliJson => "opencode_cli_json",
            #[cfg(test)] ConnectionKind::MockInProcess => "mock_in_process",
        };
        if descriptor.id != provider || connection != expected_connection { return None; }
        match role {
            "lead" if descriptor.roles.contains(&AgentRole::Lead) && descriptor.capabilities.lead_conversation => Some("lead"),
            "reviewer" if descriptor.roles.contains(&AgentRole::Reviewer) && descriptor.capabilities.delegated_review => Some("reviewer"),
            _ => None,
        }
    })
}

fn inspect(path: &Path) -> (&'static str, Option<String>) {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => ("installed", None),
        Ok(_) => ("missing", Some("실행 파일 경로가 파일이 아닙니다".into())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ("missing", Some("실행 파일을 찾을 수 없습니다".into())),
        Err(e) => ("error", Some(format!("실행 파일 확인 실패: {e}"))),
    }
}

fn names(provider: &str, filename: &'static str) -> Vec<&'static str> {
    if provider == "gemini_cli" { vec![filename, "gemini.cmd"] } else { vec![filename] }
}

fn default_path(provider: &str, home: Option<&Path>, appdata: Option<&Path>) -> Option<PathBuf> {
    match provider {
        "claude_code" => home.map(|p| p.join(".local").join("bin").join("claude.exe")),
        "gemini_cli" => appdata.map(|p| p.join("npm").join("gemini.cmd")),
        "opencode" => appdata.map(|p| p.join("npm").join("node_modules").join("opencode-ai").join("bin").join("opencode.exe")),
        _ => None,
    }
}

fn discover_in(path_dirs: &[PathBuf], home: Option<&Path>, appdata: Option<&Path>) -> Vec<Candidate> {
    PROVIDERS.iter().map(|(provider, name, filename)| {
        let mut found = None;
        let mut failure = None;
        let suggested = default_path(provider, home, appdata);
        let paths = suggested.iter().cloned().chain(path_dirs.iter().flat_map(|dir| names(provider, filename).into_iter().map(|file| dir.join(file))));
        for path in paths {
            let (status, reason) = inspect(&path);
            if status == "installed" {
                found = Some(std::fs::canonicalize(&path).unwrap_or(path));
                break;
            }
            if status == "error" { failure = reason; }
        }
        Candidate { provider: (*provider).into(), name: (*name).into(),
            installation: if found.is_some() { "installed" } else if failure.is_some() { "error" } else { "missing" },
            path: found.or(suggested).map(|p| p.to_string_lossy().into_owned()), failure_reason: failure }
    }).collect()
}

pub fn discover() -> Vec<Candidate> {
    let dirs = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    discover_in(&dirs, home.as_deref(), appdata.as_deref())
}

impl Store {
    #[cfg(test)]
    pub(crate) fn register_mock_for_test(&self, id: &str, role: &str) -> Result<()> {
        if role != "lead" && role != "reviewer" { return Err("invalid mock role".into()); }
        self.db.lock().map_err(err)?.execute(
            "INSERT INTO agent_instances(id,provider,name,connection_kind,role,source,created_at) VALUES(?1,'mock_agent','Mock Agent','mock_in_process',?2,'builtin',?3)",
            params![id, role, timestamp()],
        ).map_err(err)?;
        Ok(())
    }

    pub fn list_agent_instances(&self) -> Result<Vec<Instance>> {
        let db = self.db.lock().map_err(err)?;
        let mut stmt = db.prepare("SELECT id,provider,name,executable_path,connection_kind,role,source FROM agent_instances ORDER BY CASE source WHEN 'builtin' THEN 0 ELSE 1 END,created_at,id").map_err(err)?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?))).map_err(err)?;
        let mut instances = Vec::new();
        for row in rows {
            let (id, provider, name, path, connection, role, source) = row.map_err(err)?;
            let builtin = source == "builtin";
            let (installation, failure_reason) = path.as_deref().map(|p| inspect(Path::new(p))).unwrap_or(("unknown", None));
            let active_role = active_role(&provider, &connection, &role, builtin, path.as_deref());
            let supported = builtin || active_role.is_some();
            instances.push(Instance { id, provider, name, path, connection, role, active_role, builtin,
                support: if supported { "supported" } else { "unsupported" },
                installation, authentication: "unknown", availability: if supported { "unknown" } else { "unavailable" },
                execution: "idle", checked_at: timestamp(), failure_reason });
        }
        Ok(instances)
    }

    pub fn register_agent_instance(&self, provider_id: &str, path: &str, role: &str) -> Result<Instance> {
        let (name, filename) = provider(provider_id)?;
        if role != "lead" && role != "reviewer" { return Err("역할은 Lead 또는 Reviewer여야 합니다".into()); }
        let path = PathBuf::from(path.trim());
        if !path.is_absolute() { return Err("실행 파일의 절대 경로를 입력해 주세요".into()); }
        if !path.file_name().is_some_and(|v| names(provider_id, filename).iter().any(|expected| v.to_string_lossy().eq_ignore_ascii_case(expected))) {
            return Err(format!("{name} 실행 파일 이름이 올바르지 않습니다"));
        }
        if inspect(&path).0 != "installed" { return Err("실행 파일을 찾을 수 없습니다".into()); }
        let canonical = std::fs::canonicalize(&path).map_err(err)?;
        let path_text = canonical.to_string_lossy().into_owned();
        let key = path_text.to_lowercase();
        let id = uuid::Uuid::new_v4().to_string();
        let db = self.db.lock().map_err(err)?;
        let exists: Option<String> = db.query_row("SELECT id FROM agent_instances WHERE provider=?1 AND path_key=?2", params![provider_id,key], |r| r.get(0)).optional().map_err(err)?;
        if exists.is_some() { return Err("이미 등록된 실행 파일입니다".into()); }
        let connection = if provider_id == "opencode" && role == "lead" && path_text.replace('\\', "/").to_lowercase().ends_with("/node_modules/opencode-ai/bin/opencode.exe") { "opencode_cli_json" } else { "not_configured" };
        db.execute("INSERT INTO agent_instances(id,provider,name,executable_path,path_key,connection_kind,role,source,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,'user',?8)",
            params![id,provider_id,name,path_text,key,connection,role,timestamp()]).map_err(err)?;
        if connection == "opencode_cli_json" {
            db.execute("INSERT OR IGNORE INTO agents(id,status,detail,updated_at) VALUES(?1,'idle','대기 중',?2)", params![id,timestamp()]).map_err(err)?;
        }
        drop(db);
        self.list_agent_instances()?.into_iter().find(|item| item.id == id).ok_or_else(|| "등록 결과를 읽을 수 없습니다".into())
    }

    pub fn unregister_agent_instance(&self, id: &str) -> Result<()> {
        let db = self.db.lock().map_err(err)?;
        let sessions: i64 = db.query_row("SELECT COUNT(*) FROM chat_sessions WHERE agent_instance_id=?1", [id], |r| r.get(0)).map_err(err)?;
        if sessions > 0 { return Err("이 에이전트를 사용하는 세션이 있어 등록 해제할 수 없습니다".into()); }
        let count = db.execute("DELETE FROM agent_instances WHERE id=?1 AND source='user'", [id]).map_err(err)?;
        if count == 0 { return Err("등록 해제할 사용자 에이전트를 찾을 수 없습니다".into()); }
        db.execute("DELETE FROM agents WHERE id=?1", [id]).map_err(err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_reads_known_filenames_without_starting_them() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("claude.exe"), b"not executable").unwrap();
        std::fs::write(temp.path().join("unknown.exe"), b"untouched").unwrap();
        let found = discover_in(&[temp.path().to_path_buf()], Some(temp.path()), Some(temp.path()));
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].installation, "installed");
        assert_eq!(found[1].installation, "missing");
        assert_eq!(found[2].installation, "missing");
        assert!(found[1].path.as_deref().unwrap().ends_with("npm\\gemini.cmd") || found[1].path.as_deref().unwrap().ends_with("npm/gemini.cmd"));
        assert!(found[2].path.as_deref().unwrap().ends_with("opencode.exe"));
        assert!(found.iter().all(|item| item.provider != "unknown"));
    }
    #[test]
    fn discovery_prefers_installed_cli_defaults_and_registers_known_npm_shim_without_execution() {
        let temp = tempfile::tempdir().unwrap();
        let appdata = temp.path().join("roaming");
        let cli = appdata.join("npm/node_modules/opencode-ai/bin/opencode.exe");
        let shim = appdata.join("npm/gemini.cmd");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, b"not executable").unwrap();
        std::fs::write(&shim, b"not executable").unwrap();
        let found = discover_in(&[], Some(temp.path()), Some(&appdata));
        assert_eq!(found[1].installation, "installed");
        assert_eq!(found[2].installation, "installed");
        assert_eq!(found[2].path.as_deref(), Some(std::fs::canonicalize(&cli).unwrap().to_string_lossy().as_ref()));
        let store = Store::open(&temp.path().join("chat.sqlite")).unwrap();
        let item = store.register_agent_instance("gemini_cli", &shim.to_string_lossy(), "reviewer").unwrap();
        assert_eq!((item.support, item.connection.as_str()), ("unsupported", "not_configured"));
    }
    #[test]
    fn migration_preserves_existing_records_and_builtin_instances() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("chat.sqlite");
        let store = Store::open(&path).unwrap();
        let message = store.message("user", "기존 메시지", false).unwrap();
        assert_eq!(store.list_agent_instances().unwrap().len(), 2);
        drop(store);
        let db = Connection::open(&path).unwrap();
        db.execute_batch("DROP TABLE agent_instances; PRAGMA user_version=4;").unwrap();
        drop(db);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.list_agent_instances().unwrap().len(), 2);
        drop(reopened);
        let db = Connection::open(&path).unwrap();
        assert_eq!(db.query_row("PRAGMA user_version", [], |r| r.get::<_,i64>(0)).unwrap(), 6);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM messages WHERE id=?1", [&message.id], |r| r.get::<_,i64>(0)).unwrap(), 1);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM agents", [], |r| r.get::<_,i64>(0)).unwrap(), 2);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM chat_sessions", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
    }

    #[test]
    fn registration_validates_path_prevents_duplicates_and_only_removes_local_row() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("chat.sqlite")).unwrap();
        let exe = temp.path().join("claude.exe");
        std::fs::write(&exe, b"do not execute").unwrap();
        assert!(store.register_agent_instance("claude_code", "relative/claude.exe", "lead").is_err());
        assert!(store.register_agent_instance("claude_code", &temp.path().join("missing/claude.exe").to_string_lossy(), "lead").is_err());
        assert!(store.register_agent_instance("unknown", &exe.to_string_lossy(), "lead").is_err());
        let item = store.register_agent_instance("claude_code", &exe.to_string_lossy(), "lead").unwrap();
        assert_eq!((item.support, item.installation, item.authentication, item.availability), ("unsupported", "installed", "unknown", "unavailable"));
        assert_eq!(item.role, "lead");
        assert_eq!(item.active_role, None);
        let instances = store.list_agent_instances().unwrap();
        assert_eq!(instances.iter().find(|entry| entry.id == "codex").unwrap().active_role, Some("lead"));
        assert_eq!(instances.iter().find(|entry| entry.id == "antigravity").unwrap().active_role, Some("reviewer"));
        assert!(store.register_agent_instance("claude_code", &exe.to_string_lossy(), "reviewer").is_err());
        assert!(store.unregister_agent_instance("codex").is_err());
        drop(store);
        let reopened = Store::open(&temp.path().join("chat.sqlite")).unwrap();
        assert_eq!(reopened.list_agent_instances().unwrap().iter().filter(|entry| entry.id == item.id).count(), 1);
        reopened.unregister_agent_instance(&item.id).unwrap();
        assert!(exe.exists());
        assert_eq!(reopened.list_agent_instances().unwrap().len(), 2);
    }

    #[test]
    fn active_role_requires_matching_adapter_connection_and_capability() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("chat.sqlite")).unwrap();
        let db = store.db.lock().unwrap();
        db.execute("UPDATE agent_instances SET connection_kind='not_configured' WHERE id='codex'", []).unwrap();
        db.execute("UPDATE agent_instances SET role='lead' WHERE id='antigravity'", []).unwrap();
        drop(db);
        let instances = store.list_agent_instances().unwrap();
        assert_eq!(instances.iter().find(|entry| entry.id == "codex").unwrap().active_role, None);
        assert_eq!(instances.iter().find(|entry| entry.id == "antigravity").unwrap().active_role, None);
    }

    #[test]
    fn opencode_lead_session_survives_reopen_and_keeps_registered_cli() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("chat.sqlite");
        let cli = temp.path().join("node_modules/opencode-ai/bin/opencode.exe");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, b"probe only").unwrap();
        let store = Store::open(&path).unwrap();
        let instance = store.register_agent_instance("opencode", &cli.to_string_lossy(), "lead").unwrap();
        assert_eq!((instance.connection.as_str(), instance.active_role), ("opencode_cli_json", Some("lead")));
        let session = store.create_opencode_session("default", "OpenCode test", &instance.id).unwrap();
        assert_eq!((session.provider.as_str(), session.agent_instance_id.as_deref()), ("opencode", Some(instance.id.as_str())));
        assert!(store.unregister_agent_instance(&instance.id).is_err());
        drop(store);
        let reopened = Store::open(&path).unwrap();
        let saved = reopened.session(&session.id).unwrap();
        assert_eq!((saved.provider.as_str(), saved.agent_instance_id.as_deref()), ("opencode", Some(instance.id.as_str())));
        assert_eq!(reopened.list_agent_instances().unwrap().into_iter().find(|a| a.id == instance.id).unwrap().active_role, Some("lead"));
    }

    #[test]
    fn v5_opencode_registration_migrates_without_losing_chat() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("chat.sqlite");
        let cli = temp.path().join("node_modules/opencode-ai/bin/opencode.exe");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, b"probe only").unwrap();
        let store = Store::open(&path).unwrap();
        let message = store.message("user", "preserved", false).unwrap();
        let instance = store.register_agent_instance("opencode", &cli.to_string_lossy(), "lead").unwrap();
        drop(store);
        let db = Connection::open(&path).unwrap();
        db.execute("UPDATE agent_instances SET connection_kind='not_configured' WHERE id=?1", [&instance.id]).unwrap();
        db.execute_batch("ALTER TABLE chat_sessions DROP COLUMN provider;
            ALTER TABLE chat_sessions DROP COLUMN agent_instance_id;
            PRAGMA user_version=5;").unwrap();
        drop(db);
        let migrated = Store::open(&path).unwrap();
        assert_eq!(migrated.snapshot().unwrap().messages[0].id, message.id);
        assert_eq!(migrated.session("default-session").unwrap().provider, "codex");
        assert_eq!(migrated.list_agent_instances().unwrap().into_iter().find(|a| a.id == instance.id).unwrap().active_role, Some("lead"));
    }
}
