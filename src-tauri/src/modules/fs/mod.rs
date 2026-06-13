pub mod local;
pub mod search;
pub mod git;
pub mod remote;
pub mod system;

// Re-export type definitions
pub use local::FileEntry;
pub use search::SearchMatch;
pub use git::{GitCommitEntry, GitStatusResult};

// Tauri commands re-exported from submodules
#[allow(dead_code)]
#[tauri::command]
pub fn get_initial_cwd() -> Result<String, String> {
    local::get_initial_cwd()
}

#[tauri::command]
pub fn fs_list_dir(path: String) -> Result<Vec<FileEntry>, String> {
    local::fs_list_dir(path)
}

#[tauri::command]
pub fn fs_read_file(path: String) -> Result<String, String> {
    local::fs_read_file(path)
}

#[tauri::command]
pub fn fs_write_file(path: String, content: String) -> Result<(), String> {
    local::fs_write_file(path, content)
}

#[tauri::command]
pub fn fs_create_dir(path: String) -> Result<(), String> {
    local::fs_create_dir(path)
}

#[tauri::command]
pub fn fs_delete(path: String) -> Result<(), String> {
    local::fs_delete(path)
}

#[tauri::command]
pub fn fs_rename(from: String, to: String) -> Result<(), String> {
    local::fs_rename(from, to)
}

#[tauri::command]
pub fn fs_exists(path: String) -> bool {
    local::fs_exists(path)
}

#[tauri::command]
pub fn fs_stat(path: String) -> Result<FileEntry, String> {
    local::fs_stat(path)
}

#[tauri::command]
pub fn fs_search_files(path: String, query: String) -> Result<Vec<FileEntry>, String> {
    search::fs_search_files(path, query)
}

#[tauri::command]
pub fn fs_search_contents(path: String, query: String, max_results: Option<usize>) -> Result<Vec<SearchMatch>, String> {
    search::fs_search_contents(path, query, max_results)
}

#[tauri::command]
pub fn sys_check_installed(cmd: String) -> bool {
    system::sys_check_installed(cmd)
}

#[tauri::command]
pub fn sys_run_diagnostics(cmd_type: String, directory: String) -> Result<String, String> {
    system::sys_run_diagnostics(cmd_type, directory)
}

#[tauri::command]
pub async fn ssh_list_dir(username: String, host: String, port: u16) -> Result<String, String> {
    remote::ssh_list_dir(username, host, port).await
}

// Git commands
#[tauri::command]
pub fn git_init(repo_path: String) -> Result<(), String> {
    git::git_init(repo_path)
}

#[tauri::command]
pub fn git_log(repo_path: String, max_count: Option<u32>) -> Result<Vec<GitCommitEntry>, String> {
    git::git_log(repo_path, max_count)
}

#[tauri::command]
pub fn git_push(repo_path: String, remote: Option<String>, branch: Option<String>) -> Result<String, String> {
    git::git_push(repo_path, remote, branch)
}

#[tauri::command]
pub fn git_pull(repo_path: String, remote: Option<String>, branch: Option<String>) -> Result<String, String> {
    git::git_pull(repo_path, remote, branch)
}

#[tauri::command]
pub fn git_sync(repo_path: String) -> Result<String, String> {
    git::git_sync(repo_path)
}

#[tauri::command]
pub fn git_diff_uncommitted(repo_path: String, staged: bool) -> Result<String, String> {
    git::git_diff_uncommitted(repo_path, staged)
}

#[tauri::command]
pub fn git_remote_url(repo_path: String) -> Result<String, String> {
    git::git_remote_url(repo_path)
}

#[tauri::command]
pub fn git_get_status(repo_path: String) -> Result<GitStatusResult, String> {
    git::git_get_status(repo_path)
}

#[tauri::command]
pub fn git_commit(repo_path: String, message: String) -> Result<String, String> {
    git::git_commit(repo_path, message)
}

#[tauri::command]
pub fn git_stage_file(repo_path: String, file_path: String) -> Result<(), String> {
    git::git_stage_file(repo_path, file_path)
}

#[tauri::command]
pub fn git_unstage_file(repo_path: String, file_path: String) -> Result<(), String> {
    git::git_unstage_file(repo_path, file_path)
}

#[tauri::command]
pub fn git_discard_file(repo_path: String, file_path: String) -> Result<(), String> {
    git::git_discard_file(repo_path, file_path)
}

#[tauri::command]
pub fn git_stage_all(repo_path: String) -> Result<(), String> {
    git::git_stage_all(repo_path)
}

#[tauri::command]
pub fn git_unstage_all(repo_path: String) -> Result<(), String> {
    git::git_unstage_all(repo_path)
}
