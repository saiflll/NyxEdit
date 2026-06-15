use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::parsers;
use super::symbol_graph::{EdgeKind, SymbolGraph, SymbolNode};

#[derive(Clone)]
pub struct GraphState {
    pub graph: Arc<Mutex<SymbolGraph>>,
    watcher_root: Arc<Mutex<Option<String>>>,
    workspace_root: Arc<Mutex<Option<String>>>,
}

impl GraphState {
    pub fn new() -> Self {
        Self { graph: Arc::new(Mutex::new(SymbolGraph::new())), watcher_root: Arc::new(Mutex::new(None)), workspace_root: Arc::new(Mutex::new(None)) }
    }

    fn nyx_path(&self, root: &str) -> PathBuf {
        std::path::Path::new(root).join(".nyx").join("symbol_graph.json")
    }

    pub fn try_load_workspace(&self, root: &str) -> Result<bool, String> {
        let path = self.nyx_path(root);
        *self.workspace_root.lock().map_err(|e| format!("Lock: {}", e))? = Some(root.to_string());
        Ok(path.exists())
    }

    pub fn ensure_loaded(&self) -> Result<(), String> {
        let mut g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        if g.nodes.is_empty() {
            let root_lock = self.workspace_root.lock().map_err(|e| format!("Lock: {}", e))?;
            if let Some(ref root) = *root_lock {
                let path = self.nyx_path(root);
                if path.exists() {
                    if let Ok(loaded_g) = SymbolGraph::load_from(&path) {
                        *g = loaded_g;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn unload_workspace(&self) -> Result<(), String> {
        let mut g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        g.clear();
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        let root = self.workspace_root.lock().map_err(|e| format!("Lock: {}", e))?;
        let root = root.as_ref().ok_or("No workspace root set")?;
        let path = self.nyx_path(root);
        let g = self.graph.lock().map_err(|e| format!("Lock: {}", e))?;
        g.save_to(&path).map_err(|e| format!("Failed to save graph: {}", e))
    }

    pub fn index_workspace_with_progress(&self, app: &tauri::AppHandle, root: &str) -> Result<String, String> {
        use tauri::Emitter;

        let root_path = std::path::Path::new(root);
        let mut entries = Vec::new();
        parsers::collect_source_files(root_path, &mut entries, root_path);
        
        let total_files = entries.len();
        
        let mut g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        g.clear();
        
        // Notify start
        let _ = app.emit("graph:index_start", total_files);

        for (i, entry) in entries.iter().enumerate() {
            if let Ok(content) = std::fs::read(entry) {
                let rel_path = entry.strip_prefix(root_path).unwrap_or(entry)
                    .to_string_lossy().to_string();
                
                // Emit progress event
                let progress = if total_files > 0 {
                    (i as f32 / total_files as f32) * 100.0
                } else {
                    100.0
                };
                
                let _ = app.emit("graph:index_progress", serde_json::json!({
                    "progress": progress,
                    "current_file": rel_path,
                    "total_files": total_files,
                    "current_index": i + 1
                }));
                
                parsers::parse_file(&mut g, &content, &rel_path);
            }
        }

        let count = g.nodes.len();
        let msg = format!("Indexed {} symbols across {} files", count, total_files);
        
        // Notify end
        let _ = app.emit("graph:index_end", count);

        // Save to .nyx/
        *self.workspace_root.lock().map_err(|e| format!("Lock: {}", e))? = Some(root.to_string());
        let save_path = self.nyx_path(root);
        if let Some(parent) = save_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = g.save_to(&save_path);

        Ok(msg)
    }

    pub fn index_workspace(&self, root: &str) -> Result<String, String> {
        let mut g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        g.clear();
        parsers::index_workspace(&mut g, std::path::Path::new(root));
        let count = g.nodes.len();
        Ok(format!("Indexed {} symbols", count))
    }

    pub fn search(&self, query: &str) -> Result<Vec<SymbolNode>, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok(g.search(query).into_iter().cloned().collect())
    }

    pub fn find_by_file(&self, path: &str) -> Result<Vec<SymbolNode>, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok(g.find_by_file(path).into_iter().cloned().collect())
    }

    pub fn find_by_name(&self, name: &str) -> Result<Vec<SymbolNode>, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok(g.find_by_name(name).into_iter().cloned().collect())
    }

    pub fn definitions(&self, name: &str) -> Result<Vec<SymbolNode>, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok(g.definitions(name).into_iter().cloned().collect())
    }

    pub fn references(&self, id: &str) -> Result<Vec<SymbolNode>, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        let ref_ids: Vec<String> = g.references(id).into_iter().map(|s| s.to_string()).collect();
        let mut nodes = Vec::new();
        for rid in ref_ids {
            if let Some(n) = g.get_node(&rid) {
                nodes.push(n.clone());
            }
        }
        Ok(nodes)
    }

    pub fn outgoing_edges(&self, id: &str) -> Result<Vec<(SymbolNode, EdgeKind)>, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        let edges = g.outgoing(id);
        let mut result = Vec::new();
        for (target_id, kind) in edges {
            if let Some(n) = g.get_node(target_id) {
                result.push((n.clone(), kind.clone()));
            }
        }
        Ok(result)
    }

    pub fn traverse(&self, start_id: &str, max_depth: usize) -> Result<Vec<SymbolNode>, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok(g.traverse(start_id, max_depth).into_iter().cloned().collect())
    }

    pub fn subgraph(&self, center_id: &str, max_hops: usize) -> Result<SymbolGraph, String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok(g.subgraph(center_id, max_hops))
    }

    pub fn stats(&self) -> Result<(usize, usize), String> {
        self.ensure_loaded()?;
        let g = self.graph.lock().map_err(|e| format!("Lock error: {}", e))?;
        Ok((g.nodes.len(), g.edges.len()))
    }

    /// Start a file watcher that re-indexes changed files incrementally.
    /// Runs in a background thread until `stop_watching` is called or root changes.
    pub fn start_watching(&self, root: &str) -> Result<String, String> {
        use notify::{Config, EventKind, RecursiveMode, Watcher};
        use std::path::Path;
        use std::sync::mpsc;

        self.ensure_loaded()?;

        let root_path = Path::new(root);
        if !root_path.is_dir() {
            return Err("Workspace root is not a directory".to_string());
        }

        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(tx)
            .map_err(|e| format!("Failed to create watcher: {}", e))?;
        watcher.watch(root_path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;

        *self.watcher_root.lock().unwrap() = Some(root.to_string());

        let graph = self.graph.clone();
        let root_owned = root.to_string();
        let self_clone = self.clone();

        std::thread::spawn(move || {
            for event in rx {
                if let Ok(event) = event {
                    let is_source = event.paths.iter().any(|p| {
                        p.extension().and_then(|e| e.to_str()).map_or(false, |ext| {
                            matches!(ext, "rs" | "js" | "jsx" | "ts" | "tsx" | "mjs" | "py")
                        })
                    });
                    if !is_source { continue; }

                    let should_reindex = match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => true,
                        _ => false,
                    };
                    if !should_reindex { continue; }

                    let _ = self_clone.ensure_loaded();

                    if let Ok(mut g) = graph.lock() {
                        for path in &event.paths {
                            if path.is_file() {
                                let rel = path.strip_prefix(&root_owned)
                                    .unwrap_or(path)
                                    .to_string_lossy().to_string();
                                // Remove old nodes for this file
                                let old_ids: Vec<String> = g.nodes.values()
                                    .filter(|n| n.file_path == rel)
                                    .map(|n| n.id.clone())
                                    .collect();
                                for id in &old_ids {
                                    g.nodes.remove(id);
                                }
                                g.edges.retain(|e| !old_ids.contains(&e.source_id) && !old_ids.contains(&e.target_id));
                                // Re-parse if file exists
                                if path.exists() {
                                    if let Ok(content) = std::fs::read(path) {
                                        parsers::parse_file(&mut g, &content, &rel);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(format!("Watching {} for changes", root))
    }

    pub fn stop_watching(&self) -> Result<String, String> {
        *self.watcher_root.lock().unwrap() = None;
        Ok("File watcher stopped".to_string())
    }
}

impl Default for GraphState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn graph_index_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, GraphState>,
    root: String,
) -> Result<String, String> {
    let state_clone = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        state_clone.index_workspace_with_progress(&app, &root)
    })
    .await
    .map_err(|e| format!("Background task failed to spawn: {}", e))?
}

#[tauri::command]
pub fn graph_search(
    state: tauri::State<'_, GraphState>,
    query: String,
) -> Result<Vec<SymbolNode>, String> {
    state.search(&query)
}

#[tauri::command]
pub fn graph_find_by_file(
    state: tauri::State<'_, GraphState>,
    path: String,
) -> Result<Vec<SymbolNode>, String> {
    state.find_by_file(&path)
}

#[tauri::command]
pub fn graph_find_by_name(
    state: tauri::State<'_, GraphState>,
    name: String,
) -> Result<Vec<SymbolNode>, String> {
    state.find_by_name(&name)
}

#[tauri::command]
pub fn graph_definitions(
    state: tauri::State<'_, GraphState>,
    name: String,
) -> Result<Vec<SymbolNode>, String> {
    state.definitions(&name)
}

#[tauri::command]
pub fn graph_references(
    state: tauri::State<'_, GraphState>,
    id: String,
) -> Result<Vec<SymbolNode>, String> {
    state.references(&id)
}

#[tauri::command]
pub fn graph_outgoing(
    state: tauri::State<'_, GraphState>,
    id: String,
) -> Result<Vec<(SymbolNode, String)>, String> {
    let edges = state.outgoing_edges(&id)?;
    Ok(edges.into_iter().map(|(n, k)| (n, format!("{:?}", k))).collect())
}

#[tauri::command]
pub fn graph_traverse(
    state: tauri::State<'_, GraphState>,
    start_id: String,
    max_depth: usize,
) -> Result<Vec<SymbolNode>, String> {
    state.traverse(&start_id, max_depth)
}

#[tauri::command]
pub fn graph_subgraph(
    state: tauri::State<'_, GraphState>,
    center_id: String,
    max_hops: usize,
) -> Result<SymbolGraph, String> {
    state.subgraph(&center_id, max_hops)
}

#[tauri::command]
pub fn graph_watch(
    state: tauri::State<'_, GraphState>,
    root: String,
) -> Result<String, String> {
    state.start_watching(&root)
}

#[tauri::command]
pub fn graph_unwatch(
    state: tauri::State<'_, GraphState>,
) -> Result<String, String> {
    state.stop_watching()
}

#[tauri::command]
pub fn graph_stats(
    state: tauri::State<'_, GraphState>,
) -> Result<(usize, usize), String> {
    state.stats()
}

#[tauri::command]
pub fn graph_load_workspace(
    state: tauri::State<'_, GraphState>,
    root: String,
) -> Result<bool, String> {
    state.try_load_workspace(&root)
}

#[tauri::command]
pub fn graph_unload_workspace(
    state: tauri::State<'_, GraphState>,
) -> Result<(), String> {
    state.unload_workspace()
}
