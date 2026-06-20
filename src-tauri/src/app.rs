use std::sync::{Arc, Mutex};
use tauri::Manager;

use crate::modules::{agent_orch, ai, analysis, db, graph, project_intel, proxy, pty, routing, secrets, self_heal, session, ssh};

pub fn configure_app(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let proxy_state = Arc::new(Mutex::new(
        proxy::ProxyState::new(Arc::new(Mutex::new(Vec::new()))),
    ));
    proxy::start_proxy(&proxy_state);

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .manage(pty::PtyManager::new())
        .manage(ai::AiManager::new())
        .manage(session::sessions::SessionsState::new())
        .manage(secrets::SecretsState::default())
        .manage(ssh::SshManager::new())
        .manage(db::DbManager::new())
        .manage(graph::GraphState::new())
        .manage(project_intel::ProjectIntelState::new())
        .manage(analysis::review::ReviewState::new())
        .manage(routing::provider_stats::ProviderStats::new())
        .manage(agent_orch::AgentOrchestrator::new())
        .manage(self_heal::SelfHealEngine::new())
        .manage(session::cost_routing::CostRouter::new())
        .manage(proxy_state)
        .setup(|app| {
            let handle = app.handle();
            let sessions_state: tauri::State<'_, session::sessions::SessionsState> =
                handle.state();
            sessions_state.init(handle);
            Ok(())
        })
}
