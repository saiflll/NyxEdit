mod modules;

use std::sync::{Arc, Mutex};
use modules::{ai, db, fs, pio, proxy, pty, secrets, sessions, ssh};
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
            ai::ai_get_usage,
            ai::ai_reset_usage,
            ai::ai_set_workspace,
            ai::ai_get_agent_logs,
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
            fs::sys_check_installed,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
