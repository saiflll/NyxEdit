use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

use crate::modules::ai::ChatMessage;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatSession {
    pub id: String,
    pub name: String,
    pub agent_id: String,
    pub messages: Vec<ChatMessage>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChainStepRecord {
    pub id: String,
    pub session_id: String,
    pub chain_plan: String,
    pub current_step: i64,
    pub step_outputs: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct SessionManager {
    pub pool: sqlx::SqlitePool,
    pub db_path: PathBuf,
}

impl SessionManager {
    async fn ensure_tables(pool: &sqlx::SqlitePool) -> Result<(), String> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL DEFAULT 'New Chat',
                agent_id TEXT NOT NULL DEFAULT '',
                messages TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
        ).execute(pool).await.map_err(|e| format!("Create sessions table: {}", e))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chain_steps (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                chain_plan TEXT NOT NULL DEFAULT '{}',
                current_step INTEGER NOT NULL DEFAULT 0,
                step_outputs TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'running',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )"
        ).execute(pool).await.map_err(|e| format!("Create chain_steps table: {}", e))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS scan_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                pattern TEXT NOT NULL,
                result TEXT NOT NULL DEFAULT '[]',
                file_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                UNIQUE(path, pattern)
            )"
        ).execute(pool).await.map_err(|e| format!("Create scan_cache table: {}", e))?;

        Ok(())
    }

    pub fn new(app: &AppHandle) -> Self {
        let data_dir = app.path().app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("data.db");

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = tauri::async_runtime::block_on(async {
            let p = sqlx::SqlitePool::connect_lazy(&db_url)
                .expect("Failed to create SQLite pool");
            Self::ensure_tables(&p).await.ok();
            p
        });

        Self { pool, db_path }
    }

    pub async fn list(&self) -> Result<Vec<ChatSession>, String> {
        let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, name, agent_id, messages, created_at, updated_at FROM sessions ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("DB list error: {}", e))?;

        let mut sessions = Vec::new();
        for (id, name, agent_id, msgs_json, created_at, updated_at) in rows {
            let messages: Vec<ChatMessage> = serde_json::from_str(&msgs_json)
                .unwrap_or_default();
            sessions.push(ChatSession { id, name, agent_id, messages, created_at, updated_at });
        }
        Ok(sessions)
    }

    pub async fn get(&self, id: &str) -> Result<ChatSession, String> {
        let row = sqlx::query_as::<_, (String, String, String, String, String, String)>(
            "SELECT id, name, agent_id, messages, created_at, updated_at FROM sessions WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("DB get error: {}", e))?;

        match row {
            Some((id, name, agent_id, msgs_json, created_at, updated_at)) => {
                let messages: Vec<ChatMessage> = serde_json::from_str(&msgs_json)
                    .unwrap_or_default();
                Ok(ChatSession { id, name, agent_id, messages, created_at, updated_at })
            }
            None => Err(format!("Session not found: {}", id)),
        }
    }

    pub async fn save(&self, session: &ChatSession) -> Result<(), String> {
        let msgs_json = serde_json::to_string(&session.messages)
            .map_err(|e| format!("Serialize messages: {}", e))?;

        sqlx::query(
            "INSERT INTO sessions (id, name, agent_id, messages, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                agent_id = excluded.agent_id,
                messages = excluded.messages,
                updated_at = excluded.updated_at"
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(&session.agent_id)
        .bind(&msgs_json)
        .bind(&session.created_at)
        .bind(&session.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("DB save error: {}", e))?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("DB delete error: {}", e))?;
        Ok(())
    }

    // --- chain step checkpoint ---

    pub async fn save_chain_step(&self, step: &ChainStepRecord) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO chain_steps (id, session_id, chain_plan, current_step, step_outputs, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                current_step = excluded.current_step,
                step_outputs = excluded.step_outputs,
                status = excluded.status,
                updated_at = excluded.updated_at"
        )
        .bind(&step.id)
        .bind(&step.session_id)
        .bind(&step.chain_plan)
        .bind(step.current_step)
        .bind(&step.step_outputs)
        .bind(&step.status)
        .bind(&step.created_at)
        .bind(&step.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("DB save chain step: {}", e))?;
        Ok(())
    }

    pub async fn get_chain_step(&self, id: &str) -> Result<ChainStepRecord, String> {
        let row = sqlx::query_as::<_, (String, String, String, i64, String, String, String, String)>(
            "SELECT id, session_id, chain_plan, current_step, step_outputs, status, created_at, updated_at FROM chain_steps WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("DB get chain step: {}", e))?;

        match row {
            Some((id, session_id, chain_plan, current_step, step_outputs, status, created_at, updated_at)) => {
                Ok(ChainStepRecord { id, session_id, chain_plan, current_step, step_outputs, status, created_at, updated_at })
            }
            None => Err(format!("Chain step not found: {}", id)),
        }
    }

    // --- scan cache ---

    pub async fn save_scan_cache(&self, path: &str, pattern: &str, result: &str, file_count: i64) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO scan_cache (path, pattern, result, file_count, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(path, pattern) DO UPDATE SET
                result = excluded.result,
                file_count = excluded.file_count,
                created_at = excluded.created_at"
        )
        .bind(path)
        .bind(pattern)
        .bind(result)
        .bind(file_count)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("DB save scan cache: {}", e))?;
        Ok(())
    }

    pub async fn get_scan_cache(&self, path: &str, pattern: &str) -> Result<Option<(String, i64)>, String> {
        let row = sqlx::query_as::<_, (String, i64)>(
            "SELECT result, file_count FROM scan_cache WHERE path = ? AND pattern = ?"
        )
        .bind(path)
        .bind(pattern)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("DB get scan cache: {}", e))?;

        Ok(row)
    }
}

pub struct SessionsState {
    pub pool: Arc<Mutex<Option<sqlx::SqlitePool>>>,
}

impl SessionsState {
    pub fn new() -> Self {
        Self { pool: Arc::new(Mutex::new(None)) }
    }

    pub fn init(&self, app: &AppHandle) {
        let mgr = SessionManager::new(app);
        let pool = mgr.pool;
        let mut guard = self.pool.lock().unwrap();
        *guard = Some(pool);
    }
}

impl Default for SessionsState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn ai_list_sessions(
    state: tauri::State<'_, SessionsState>,
) -> Result<Vec<ChatSession>, String> {
    let pool = {
        let guard = state.pool.lock().unwrap();
        guard.clone().ok_or("SessionManager not initialized")?
    };
    let mgr = SessionManager { pool, db_path: PathBuf::new() };
    mgr.list().await
}

#[tauri::command]
pub async fn ai_get_session(
    state: tauri::State<'_, SessionsState>,
    session_id: String,
) -> Result<ChatSession, String> {
    let pool = {
        let guard = state.pool.lock().unwrap();
        guard.clone().ok_or("SessionManager not initialized")?
    };
    let mgr = SessionManager { pool, db_path: PathBuf::new() };
    mgr.get(&session_id).await
}

#[tauri::command]
pub async fn ai_save_session(
    state: tauri::State<'_, SessionsState>,
    session: ChatSession,
) -> Result<(), String> {
    let pool = {
        let guard = state.pool.lock().unwrap();
        guard.clone().ok_or("SessionManager not initialized")?
    };
    let mgr = SessionManager { pool, db_path: PathBuf::new() };
    mgr.save(&session).await
}

#[tauri::command]
pub async fn ai_delete_session(
    state: tauri::State<'_, SessionsState>,
    session_id: String,
) -> Result<(), String> {
    let pool = {
        let guard = state.pool.lock().unwrap();
        guard.clone().ok_or("SessionManager not initialized")?
    };
    let mgr = SessionManager { pool, db_path: PathBuf::new() };
    mgr.delete(&session_id).await
}

#[tauri::command]
pub async fn recover_last_session(
    _app: tauri::AppHandle,
    state: tauri::State<'_, SessionsState>,
) -> Result<Option<ChatSession>, String> {
    let pool = {
        let guard = state.pool.lock().unwrap();
        guard.clone().ok_or("SessionManager not initialized")?
    };
    let mgr = SessionManager { pool, db_path: PathBuf::new() };
    let sessions = mgr.list().await?;
    // Recover the most recently updated session
    Ok(sessions.into_iter().next())
}
