use serde::{Deserialize, Serialize};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HandoffRecord {
    pub id: String,
    pub created_at: String,
    pub intent: Option<String>,
    pub original_prompt: Option<String>,
    pub summary: Option<String>,
    pub files_touched: Vec<String>,
    pub result_excerpt: Option<String>,
    pub next_steps: Vec<String>,
    pub model_chain: Vec<String>,
    pub completed: bool,
}

fn handoff_dir_for(workspace_root: Option<&str>) -> PathBuf {
    let base = if let Some(root) = workspace_root {
        if !root.is_empty() { PathBuf::from(root) } else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")) }
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join(".nyx").join("handoff")
}

fn ensure_handoff_dir(dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Create dir failed: {}", e))
}

#[tauri::command]
pub fn handoff_create(workspace_root: Option<String>, task_id: String, prompt: String) -> Result<HandoffRecord, String> {
    let dir = handoff_dir_for(workspace_root.as_deref());
    ensure_handoff_dir(&dir)?;
    let rec = HandoffRecord {
        id: task_id.clone(),
        created_at: Utc::now().to_rfc3339(),
        intent: Some("unspecified".into()),
        original_prompt: Some(prompt),
        summary: None,
        files_touched: vec![],
        result_excerpt: None,
        next_steps: vec![],
        model_chain: vec![],
        completed: false,
    };
    let path = dir.join(format!("{}.json", task_id));
    let j = serde_json::to_string_pretty(&rec).map_err(|e| e.to_string())?;
    fs::write(&path, j).map_err(|e| format!("Write failed: {}", e))?;
    Ok(rec)
}

#[tauri::command]
pub fn handoff_update(workspace_root: Option<String>, task_id: String, summary: Option<String>, next_steps: Option<Vec<String>>) -> Result<HandoffRecord, String> {
    let dir = handoff_dir_for(workspace_root.as_deref());
    ensure_handoff_dir(&dir)?;
    let path = dir.join(format!("{}.json", task_id));
    let mut rec: HandoffRecord = if path.exists() {
        let txt = fs::read_to_string(&path).map_err(|e| format!("Read failed: {}", e))?;
        serde_json::from_str(&txt).map_err(|e| format!("Parse failed: {}", e))?
    } else {
        return Err(format!("Handoff {} not found", task_id));
    };
    if let Some(s) = summary { rec.summary = Some(s); }
    if let Some(ns) = next_steps { rec.next_steps = ns; }
    let j = serde_json::to_string_pretty(&rec).map_err(|e| e.to_string())?;
    fs::write(&path, j).map_err(|e| format!("Write failed: {}", e))?;
    Ok(rec)
}

#[tauri::command]
pub fn handoff_get_latest(workspace_root: Option<String>) -> Result<Option<HandoffRecord>, String> {
    let dir = handoff_dir_for(workspace_root.as_deref());
    if !dir.exists() { return Ok(None); }
    let mut latest: Option<(std::time::SystemTime, HandoffRecord)> = None;
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?;
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
        let mtime = meta.modified().map_err(|e| e.to_string())?;
        let txt = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let rec: HandoffRecord = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        if let Some((t, _)) = &latest {
            if mtime > *t { latest = Some((mtime, rec)); }
        } else {
            latest = Some((mtime, rec));
        }
    }
    Ok(latest.map(|(_, r)| r))
}

#[tauri::command]
pub fn handoff_list(workspace_root: Option<String>) -> Result<Vec<HandoffRecord>, String> {
    let dir = handoff_dir_for(workspace_root.as_deref());
    if !dir.exists() { return Ok(vec![]); }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?;
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let txt = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let rec: HandoffRecord = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        out.push(rec);
    }
    // Sort by created_at descending
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

#[tauri::command]
pub fn handoff_complete(workspace_root: Option<String>, task_id: String) -> Result<HandoffRecord, String> {
    let dir = handoff_dir_for(workspace_root.as_deref());
    let path = dir.join(format!("{}.json", task_id));
    if !path.exists() { return Err("not found".into()); }
    let txt = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut rec: HandoffRecord = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    rec.completed = true;
    let j = serde_json::to_string_pretty(&rec).map_err(|e| e.to_string())?;
    fs::write(&path, j).map_err(|e| format!("Write failed: {}", e))?;
    Ok(rec)
}
