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

pub struct SessionManager {
    pub base_path: PathBuf,
}

impl SessionManager {
    pub fn new(app: &AppHandle) -> Self {
        let base_path = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("sessions");
        std::fs::create_dir_all(&base_path).ok();
        Self { base_path }
    }

    fn file_path(&self, id: &str) -> PathBuf {
        let mut safe = id.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");
        if !safe.ends_with(".json") { safe.push_str(".json"); }
        self.base_path.join(safe)
    }

    pub fn list(&self) -> Result<Vec<ChatSession>, String> {
        let mut sessions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.base_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(session) = serde_json::from_str::<ChatSession>(&content) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn get(&self, id: &str) -> Result<ChatSession, String> {
        let path = self.file_path(id);
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Session not found: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Parse error: {}", e))
    }

    pub fn save(&self, session: &ChatSession) -> Result<(), String> {
        let path = self.file_path(&session.id);
        let content = serde_json::to_string_pretty(session)
            .map_err(|e| format!("Serialize error: {}", e))?;
        std::fs::write(&path, content)
            .map_err(|e| format!("Write error: {}", e))
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.file_path(id);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Delete error: {}", e))
        } else {
            Ok(())
        }
    }
}

pub struct SessionsState {
    pub manager: Arc<Mutex<Option<SessionManager>>>,
}

impl SessionsState {
    pub fn new() -> Self {
        Self { manager: Arc::new(Mutex::new(None)) }
    }

    pub fn init(&self, app: &AppHandle) {
        let mut mgr = self.manager.lock().unwrap();
        *mgr = Some(SessionManager::new(app));
    }
}

impl Default for SessionsState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub fn ai_list_sessions(
    state: tauri::State<'_, SessionsState>,
) -> Result<Vec<ChatSession>, String> {
    let mgr = state.manager.lock().unwrap();
    let mgr = mgr.as_ref().ok_or("SessionManager not initialized")?;
    mgr.list()
}

#[tauri::command]
pub fn ai_get_session(
    state: tauri::State<'_, SessionsState>,
    session_id: String,
) -> Result<ChatSession, String> {
    let mgr = state.manager.lock().unwrap();
    let mgr = mgr.as_ref().ok_or("SessionManager not initialized")?;
    mgr.get(&session_id)
}

#[tauri::command]
pub fn ai_save_session(
    state: tauri::State<'_, SessionsState>,
    session: ChatSession,
) -> Result<(), String> {
    let mgr = state.manager.lock().unwrap();
    let mgr = mgr.as_ref().ok_or("SessionManager not initialized")?;
    mgr.save(&session)
}

#[tauri::command]
pub fn ai_delete_session(
    state: tauri::State<'_, SessionsState>,
    session_id: String,
) -> Result<(), String> {
    let mgr = state.manager.lock().unwrap();
    let mgr = mgr.as_ref().ok_or("SessionManager not initialized")?;
    mgr.delete(&session_id)
}
