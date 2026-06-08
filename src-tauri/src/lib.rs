mod modules;

use std::sync::{Arc, Mutex};
use modules::{ai, fs, pio, proxy, pty, secrets, sessions};
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
        .manage(pty::PtyManager::new())
        .manage(ai::AiManager::new())
        .manage(sessions::SessionsState::new())
        .manage(secrets::SecretsState::default())
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
            ai::ai_chat,
            ai::ai_chat_stream,
            ai::ai_list_agents,
            ai::ai_update_agent,
            ai::ai_remove_agent,
            ai::ai_list_models,
            ai::ai_get_usage,
            ai::ai_reset_usage,
            ai::ai_list_personas,
            secrets::secrets_get,
            secrets::secrets_set,
            secrets::secrets_delete,
            secrets::secrets_get_all,
            sessions::ai_list_sessions,
            sessions::ai_get_session,
            sessions::ai_save_session,
            sessions::ai_delete_session,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
