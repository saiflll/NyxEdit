mod modules;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::{Arc, Mutex};
use modules::{agent_orch, ai, cost_routing, db, fs, graph, pio, project_intel, provider_stats, proxy, pty, review, secrets, self_heal, sessions, ssh};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let proxy_state = Arc::new(Mutex::new(proxy::ProxyState::new(Arc::new(Mutex::new(Vec::new())))));
    proxy::start_proxy(&proxy_state);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .manage(pty::PtyManager::new())
        .manage(ai::AiManager::new())
        .manage(sessions::SessionsState::new())
        .manage(secrets::SecretsState::default())
        .manage(ssh::SshManager::new())
        .manage(db::DbManager::new())
        .manage(graph::GraphState::new())
        .manage(project_intel::ProjectIntelState::new())
        .manage(review::ReviewState::new())
        .manage(provider_stats::ProviderStats::new())
        .manage(agent_orch::AgentOrchestrator::new())
        .manage(self_heal::SelfHealEngine::new())
        .manage(cost_routing::CostRouter::new())
        .manage(proxy_state)
        .setup(|app| {
            let handle = app.handle();
            let sessions_state: tauri::State<'_, sessions::SessionsState> = handle.state();
            sessions_state.init(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pty::pty_open,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_close,
            pty::pty_list,
            pty::pty_mark_private,
            pty::pty_unmark_private,
            pty::pty_is_private,
            ai::ai_chat,
            ai::ai_chat_stream,
            ai::ai_respond_bash_permission,
            ai::ai_respond_file_permission,
            ai::ai_list_agents,
            ai::ai_update_agent,
            ai::ai_remove_agent,
            ai::ai_list_models,
            ai::ai_sync_agent_models,
            ai::ai_probe_models,
            ai::ai_classify_request,
            ai::ai_get_usage,
            ai::ai_reset_usage,
            ai::ai_set_workspace,
            ai::ai_get_agent_logs,
            ai::ai_list_personas,
            ai::ai_compute_diff,
            secrets::secrets_get,
            secrets::secrets_set,
            secrets::secrets_delete,
            secrets::secrets_get_all,
            sessions::ai_list_sessions,
            sessions::ai_get_session,
            sessions::ai_save_session,
            sessions::ai_delete_session,
            sessions::recover_last_session,
            proxy::get_proxy_port,
            proxy::get_proxy_logs,
            fs::fs_list_dir,
            fs::fs_read_file,
            fs::fs_write_file,
            fs::fs_create_dir,
            fs::fs_delete,
            fs::fs_rename,
            fs::fs_exists,
            fs::fs_stat,
            fs::fs_search_files,
            fs::fs_search_contents,
            fs::sys_check_installed,
            fs::sys_run_diagnostics,
            fs::ssh_list_dir,
            fs::git_get_status,
            fs::git_commit,
            fs::git_stage_file,
            fs::git_unstage_file,
            fs::git_discard_file,
            fs::git_stage_all,
            fs::git_unstage_all,
            fs::git_init,
            fs::git_push,
            fs::git_pull,
            fs::git_sync,
            fs::git_diff_uncommitted,
            fs::git_remote_url,
            fs::git_log,
            // SSH + SFTP commands
            ssh::ssh_connect,
            ssh::ssh_disconnect,
            ssh::ssh_list_sessions,
            ssh::ssh_exec,
            ssh::sftp_list_dir,
            ssh::sftp_read_file,
            ssh::sftp_write_file,
            ssh::sftp_delete,
            ssh::sftp_mkdir,
            ssh::sftp_rename,
            // Database commands
            db::db_connect,
            db::db_disconnect,
            db::db_list_connections,
            db::db_query,
            db::db_list_databases,
            db::db_list_tables,
            db::db_get_columns,
            // PlatformIO commands
            pio::pio_detect,
            pio::pio_install,
            pio::pio_init,
            pio::pio_run,
            pio::pio_list_boards,
            // Project Intelligence commands
            project_intel::project_detect,
            project_intel::project_get_context,
            // Review System commands
            review::review_text,
            review::review_file,
            // Provider Stats commands
            provider_stats::provider_get_stats,
            provider_stats::provider_reset_stats,
            // Agent Orchestration commands
            agent_orch::orch_get_agents,
            agent_orch::orch_add_agent,
            agent_orch::orch_remove_agent,
            agent_orch::orch_get_tasks,
            // Knowledge graph commands
            graph::graph_index_workspace,
            graph::graph_search,
            graph::graph_find_by_file,
            graph::graph_find_by_name,
            graph::graph_definitions,
            graph::graph_references,
            graph::graph_outgoing,
            graph::graph_traverse,
            graph::graph_subgraph,
            graph::graph_watch,
            graph::graph_unwatch,
            graph::graph_stats,
            graph::graph_load_workspace,
            graph::graph_unload_workspace,
            // Cost Routing commands
            cost_routing::cost_get_summary,
            cost_routing::cost_set_budget,
            cost_routing::cost_recommend,
            // Self-Healing commands
            self_heal::heal_get_status,
            self_heal::heal_restart_component,
            self_heal::heal_check_startup,
            self_heal::heal_clear_crash_marker_cmd,
            self_heal::heal_set_crash_marker_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
