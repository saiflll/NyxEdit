use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::Manager;

/// Component health states
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Down(String),
}

/// Snapshot of a component's health
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ComponentInfo {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: String,
    pub uptime_secs: u64,
    pub error_count: u64,
    pub last_error: Option<String>,
}

/// SelfHealEngine — tracks component health and provides recovery.
/// Every mutable operation is idempotent and thread-safe.
pub struct SelfHealEngine {
    components: Arc<Mutex<HashMap<String, ComponentInfo>>>,
    started_at: Arc<Mutex<Instant>>,
    restart_count: Arc<Mutex<HashMap<String, u64>>>,
}

impl SelfHealEngine {
    pub fn new() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let mut comps = HashMap::new();
        for name in &[
            "graph", "sessions", "providers", "chain", "ai", "project_intel",
        ] {
            comps.insert(name.to_string(), ComponentInfo {
                name: name.to_string(),
                status: HealthStatus::Healthy,
                last_check: now.clone(),
                uptime_secs: 0,
                error_count: 0,
                last_error: None,
            });
        }
        Self {
            components: Arc::new(Mutex::new(comps)),
            started_at: Arc::new(Mutex::new(Instant::now())),
            restart_count: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Report a component as healthy
    pub fn report_healthy(&self, name: &str) {
        let mut comps = self.components.lock().unwrap();
        if let Some(c) = comps.get_mut(name) {
            c.status = HealthStatus::Healthy;
            c.last_check = chrono::Utc::now().to_rfc3339();
            c.uptime_secs = self.started_at.lock().unwrap().elapsed().as_secs();
        }
    }

    /// Report a component as degraded
    pub fn report_degraded(&self, name: &str, reason: &str) {
        let mut comps = self.components.lock().unwrap();
        if let Some(c) = comps.get_mut(name) {
            c.status = HealthStatus::Degraded(reason.to_string());
            c.last_check = chrono::Utc::now().to_rfc3339();
            c.error_count += 1;
            c.last_error = Some(reason.to_string());
        }
    }

    /// Report a component as down
    pub fn report_down(&self, name: &str, reason: &str) {
        let mut comps = self.components.lock().unwrap();
        if let Some(c) = comps.get_mut(name) {
            c.status = HealthStatus::Down(reason.to_string());
            c.last_check = chrono::Utc::now().to_rfc3339();
            c.error_count += 1;
            c.last_error = Some(reason.to_string());
        }
    }

    /// Get a snapshot of all component health
    pub fn get_status(&self) -> Vec<ComponentInfo> {
        let comps = self.components.lock().unwrap();
        let mut list: Vec<ComponentInfo> = comps.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    /// Increment restart count for a component
    pub fn record_restart(&self, component: &str) -> u64 {
        let mut rc = self.restart_count.lock().unwrap();
        let count = rc.entry(component.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    /// Get overall health status — returns true if all components are healthy
    pub fn is_all_healthy(&self) -> bool {
        let comps = self.components.lock().unwrap();
        comps.values().all(|c| c.status == HealthStatus::Healthy)
    }

    /// Determine if system is in degraded mode
    pub fn is_degraded(&self) -> bool {
        let comps = self.components.lock().unwrap();
        comps.values().any(|c| matches!(c.status, HealthStatus::Degraded(_)))
    }

    /// Get the number of components that are down
    pub fn components_down(&self) -> usize {
        let comps = self.components.lock().unwrap();
        comps.values().filter(|c| matches!(c.status, HealthStatus::Down(_))).count()
    }

    /// Attempt to heal a component by resetting its health.
    /// Returns a message describing the action taken.
    pub fn heal_component(&self, name: &str) -> String {
        let mut comps = self.components.lock().unwrap();
        match comps.get(name) {
            Some(c) if c.status == HealthStatus::Healthy => {
                format!("{}: already healthy", name)
            }
            Some(c) => {
                let prev = format!("{:?}", c.status);
                if let Some(c) = comps.get_mut(name) {
                    c.status = HealthStatus::Healthy;
                    c.last_error = None;
                    c.last_check = chrono::Utc::now().to_rfc3339();
                }
                let restarts = self.record_restart(name);
                format!("{}: healed (was {}, restart #{})", name, prev, restarts)
            }
            None => format!("{}: unknown component", name),
        }
    }
}

/// Set a crash marker file so we know on next startup that we crashed.
pub fn set_crash_marker(app: &tauri::AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(".crash_marker"), "crash");
    }
}

/// Clear the crash marker (called on clean shutdown).
pub fn clear_crash_marker(app: &tauri::AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::remove_file(dir.join(".crash_marker"));
    }
}

/// Check if the previous session crashed (crash marker exists).
pub fn has_crash_marker(app: &tauri::AppHandle) -> bool {
    app.path().app_data_dir()
        .map(|d: PathBuf| d.join(".crash_marker").exists())
        .unwrap_or(false)
}

// ─── Tauri Commands ──────────────────────────────────────────────────

#[tauri::command]
pub fn heal_get_status(state: tauri::State<'_, SelfHealEngine>) -> Result<Vec<ComponentInfo>, String> {
    Ok(state.get_status())
}

#[tauri::command]
pub fn heal_restart_component(
    state: tauri::State<'_, SelfHealEngine>,
    name: String,
) -> Result<String, String> {
    Ok(state.heal_component(&name))
}

#[tauri::command]
pub fn heal_check_startup(
    app: tauri::AppHandle,
    state: tauri::State<'_, SelfHealEngine>,
) -> Result<serde_json::Value, String> {
    let crashed = has_crash_marker(&app);
    let status = state.get_status();
    let degraded = status.iter().any(|c| matches!(c.status, HealthStatus::Degraded(_) | HealthStatus::Down(_)));
    Ok(serde_json::json!({
        "crashed": crashed,
        "degraded": degraded,
        "components": status,
        "restart_counts": *state.restart_count.lock().unwrap(),
    }))
}

#[tauri::command]
pub fn heal_clear_crash_marker_cmd(app: tauri::AppHandle) -> Result<(), String> {
    clear_crash_marker(&app);
    Ok(())
}

#[tauri::command]
pub fn heal_set_crash_marker_cmd(app: tauri::AppHandle) -> Result<(), String> {
    set_crash_marker(&app);
    Ok(())
}
