use russh::client;
use russh_sftp::client::SftpSession;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SshSessionInfo {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub connected: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SftpEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: String,
    pub modified: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SshExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// ── Russh client handler ──────────────────────────────────────────────────────

struct ClientHandler;

#[async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Accept all server keys (like StrictHostKeyChecking=no)
        Ok(true)
    }
}

// ── Session store ─────────────────────────────────────────────────────────────

struct SshSessionEntry {
    handle: Arc<tokio::sync::Mutex<client::Handle<ClientHandler>>>,
    info: SshSessionInfo,
}

pub struct SshManager {
    sessions: Mutex<HashMap<String, SshSessionEntry>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Connect to SSH server. Returns session ID.
#[tauri::command]
pub async fn ssh_connect(
    state: tauri::State<'_, SshManager>,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    private_key_path: Option<String>,
) -> Result<SshSessionInfo, String> {
    let config = Arc::new(russh::client::Config::default());
    let handler = ClientHandler;

    let addr = format!("{}:{}", host, port);
    let mut handle = russh::client::connect(config, &addr, handler)
        .await
        .map_err(|e| format!("SSH connect failed: {}", e))?;

    // Try password auth
    let authenticated = if let Some(ref pass) = password {
        if !pass.is_empty() {
            handle.authenticate_password(&username, pass)
                .await
                .map_err(|e| format!("Password auth failed: {}", e))?
        } else {
            false
        }
    } else {
        false
    };

    // Try private key auth if password failed
    if !authenticated {
        if let Some(ref key_path) = private_key_path {
            let key = russh::keys::load_secret_key(Path::new(key_path), None)
                .map_err(|e| format!("Key load failed: {}", e))?;
            handle.authenticate_publickey(&username, Arc::new(key))
                .await
                .map_err(|e| format!("Key auth failed: {}", e))?;
        } else {
            return Err("Authentication failed: no password or key provided".to_string());
        }
    }

    let id = Uuid::new_v4().to_string();
    let info = SshSessionInfo {
        id: id.clone(),
        host: host.clone(),
        port,
        username: username.clone(),
        connected: true,
    };

    let mut sessions = state.sessions.lock().unwrap();
    sessions.insert(id.clone(), SshSessionEntry {
        handle: Arc::new(tokio::sync::Mutex::new(handle)),
        info: info.clone(),
    });

    Ok(info)
}

/// Disconnect an SSH session.
#[tauri::command]
pub fn ssh_disconnect(
    state: tauri::State<'_, SshManager>,
    session_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().unwrap();
    sessions.remove(&session_id);
    Ok(())
}

/// List all active SSH sessions.
#[tauri::command]
pub fn ssh_list_sessions(
    state: tauri::State<'_, SshManager>,
) -> Vec<SshSessionInfo> {
    let sessions = state.sessions.lock().unwrap();
    sessions.values().map(|e| e.info.clone()).collect()
}

/// Execute a command on the remote SSH server.
#[tauri::command]
pub async fn ssh_exec(
    state: tauri::State<'_, SshManager>,
    session_id: String,
    command: String,
) -> Result<SshExecResult, String> {
    let handle = {
        let sessions = state.sessions.lock().unwrap();
        let entry = sessions.get(&session_id).ok_or("Session not found")?;
        entry.handle.clone()
    };

    let handle_guard = handle.lock().await;
    let mut channel = handle_guard.channel_open_session()
        .await
        .map_err(|e| format!("Channel open failed: {}", e))?;

    channel.exec(true, command.as_str())
        .await
        .map_err(|e| format!("Exec failed: {}", e))?;

    let mut stdout = String::new();
    let mut exit_code = 0i32;

    loop {
        match channel.wait().await {
            Some(russh::ChannelMsg::Data { ref data }) => {
                stdout.push_str(&String::from_utf8_lossy(data));
            }
            Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                exit_code = exit_status as i32;
            }
            Some(russh::ChannelMsg::Eof) | None => break,
            _ => {}
        }
    }

    Ok(SshExecResult { stdout, stderr: String::new(), exit_code })
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SftpListResult {
    pub path: String,
    pub entries: Vec<SftpEntry>,
}

/// List directory contents via SFTP.
#[tauri::command]
pub async fn sftp_list_dir(
    state: tauri::State<'_, SshManager>,
    session_id: String,
    remote_path: String,
) -> Result<SftpListResult, String> {
    let handle = {
        let sessions = state.sessions.lock().unwrap();
        let entry = sessions.get(&session_id).ok_or("Session not found")?;
        entry.handle.clone()
    };

    let handle_guard = handle.lock().await;
    let channel = handle_guard.channel_open_session()
        .await
        .map_err(|e| format!("Channel open failed: {}", e))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP session failed: {}", e))?;

    let resolved_path = if remote_path.is_empty() || remote_path == "." {
        sftp.canonicalize(".")
            .await
            .unwrap_or_else(|_| "/".to_string())
    } else {
        remote_path.clone()
    };

    let entries: russh_sftp::client::fs::ReadDir = sftp.read_dir(&resolved_path)
        .await
        .map_err(|e| format!("readdir failed: {}", e))?;

    let mut result = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." { continue; }

        let attrs = entry.metadata();
        let is_dir = attrs.is_dir();
        let size = attrs.size.unwrap_or(0);
        let modified = attrs.modified()
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let perm = format!("{}{}", if is_dir { 'd' } else { '-' }, attrs.permissions());
        let full_path = format!("{}/{}", resolved_path.trim_end_matches('/'), name);

        result.push(SftpEntry {
            name: name.to_string(),
            path: full_path,
            is_dir,
            size,
            permissions: perm,
            modified,
        });
    }

    result.sort_by(|a, b| {
        if a.is_dir != b.is_dir { b.is_dir.cmp(&a.is_dir) }
        else { a.name.to_lowercase().cmp(&b.name.to_lowercase()) }
    });
    Ok(SftpListResult {
        path: resolved_path,
        entries: result,
    })
}



/// Read a remote file via SFTP.
#[tauri::command]
pub async fn sftp_read_file(
    state: tauri::State<'_, SshManager>,
    session_id: String,
    remote_path: String,
) -> Result<String, String> {
    let handle = {
        let sessions = state.sessions.lock().unwrap();
        let entry = sessions.get(&session_id).ok_or("Session not found")?;
        entry.handle.clone()
    };

    let handle_guard = handle.lock().await;
    let channel = handle_guard.channel_open_session()
        .await
        .map_err(|e| format!("Channel open failed: {}", e))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP session failed: {}", e))?;

    let mut file = sftp.open(&remote_path)
        .await
        .map_err(|e| format!("SFTP open failed: {}", e))?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .await
        .map_err(|e| format!("Read failed: {}", e))?;

    Ok(content)
}

/// Write/upload a file via SFTP.
#[tauri::command]
pub async fn sftp_write_file(
    state: tauri::State<'_, SshManager>,
    session_id: String,
    remote_path: String,
    content: String,
) -> Result<(), String> {
    let handle = {
        let sessions = state.sessions.lock().unwrap();
        let entry = sessions.get(&session_id).ok_or("Session not found")?;
        entry.handle.clone()
    };

    let handle_guard = handle.lock().await;
    let channel = handle_guard.channel_open_session()
        .await
        .map_err(|e| format!("Channel open failed: {}", e))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP session failed: {}", e))?;

    let mut file = sftp.create(&remote_path)
        .await
        .map_err(|e| format!("SFTP create failed: {}", e))?;

    file.write_all(content.as_bytes())
        .await
        .map_err(|e| format!("Write failed: {}", e))?;

    Ok(())
}

/// Delete a remote file or directory via SFTP.
#[tauri::command]
pub async fn sftp_delete(
    state: tauri::State<'_, SshManager>,
    session_id: String,
    remote_path: String,
    is_dir: bool,
) -> Result<(), String> {
    let handle = {
        let sessions = state.sessions.lock().unwrap();
        let entry = sessions.get(&session_id).ok_or("Session not found")?;
        entry.handle.clone()
    };

    let handle_guard = handle.lock().await;
    let channel = handle_guard.channel_open_session()
        .await
        .map_err(|e| format!("Channel open failed: {}", e))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP session failed: {}", e))?;

    if is_dir {
        sftp.remove_dir(&remote_path)
            .await
            .map_err(|e| format!("rmdir failed: {}", e))?;
    } else {
        sftp.remove_file(&remote_path)
            .await
            .map_err(|e| format!("unlink failed: {}", e))?;
    }
    Ok(())
}

/// Create a remote directory via SFTP.
#[tauri::command]
pub async fn sftp_mkdir(
    state: tauri::State<'_, SshManager>,
    session_id: String,
    remote_path: String,
) -> Result<(), String> {
    let handle = {
        let sessions = state.sessions.lock().unwrap();
        let entry = sessions.get(&session_id).ok_or("Session not found")?;
        entry.handle.clone()
    };

    let handle_guard = handle.lock().await;
    let channel = handle_guard.channel_open_session()
        .await
        .map_err(|e| format!("Channel open failed: {}", e))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP session failed: {}", e))?;

    sftp.create_dir(&remote_path)
        .await
        .map_err(|e| format!("mkdir failed: {}", e))?;
    Ok(())
}

/// Rename/move a remote file or directory via SFTP.
#[tauri::command]
pub async fn sftp_rename(
    state: tauri::State<'_, SshManager>,
    session_id: String,
    from_path: String,
    to_path: String,
) -> Result<(), String> {
    let handle = {
        let sessions = state.sessions.lock().unwrap();
        let entry = sessions.get(&session_id).ok_or("Session not found")?;
        entry.handle.clone()
    };

    let handle_guard = handle.lock().await;
    let channel = handle_guard.channel_open_session()
        .await
        .map_err(|e| format!("Channel open failed: {}", e))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP session failed: {}", e))?;

    sftp.rename(&from_path, &to_path)
        .await
        .map_err(|e| format!("rename failed: {}", e))?;
    Ok(())
}
