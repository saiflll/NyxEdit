use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn get_initial_cwd() -> Result<String, String> {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fs_search_files(path: String, query: String) -> Result<Vec<FileEntry>, String> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let q = query.to_lowercase();
    let mut results = Vec::new();

    if q.is_empty() {
        return Ok(results);
    }

    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let name_lower = name.to_lowercase();

        // Fuzzy-like matching: query chars appear in order in name
        if fuzzy_match(&q, &name_lower) {
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let modified = metadata
                .modified()
                .ok()
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .unwrap_or_default();

            results.push(FileEntry {
                name,
                path: entry.path().to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
                modified,
            });
        }
    }

    // Directories first, then by name
    results.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    Ok(results)
}

/// Simple fuzzy match: all characters in `query` appear in order within `name`
fn fuzzy_match(query: &str, name: &str) -> bool {
    if query.is_empty() { return true; }
    let qchars: Vec<char> = query.chars().collect();
    let mut qi = 0;
    for c in name.chars() {
        if qi < qchars.len() && c == qchars[qi] {
            qi += 1;
        }
    }
    qi == qchars.len()
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

#[tauri::command]
pub fn fs_list_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let modified = metadata
            .modified()
            .ok()
            .map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_default();

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified,
        });
    }

    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    Ok(entries)
}

#[tauri::command]
pub fn fs_read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fs_write_file(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fs_create_dir(path: String) -> Result<(), String> {
    fs::create_dir_all(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fs_delete(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| e.to_string())
    } else {
        fs::remove_file(p).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn fs_rename(from: String, to: String) -> Result<(), String> {
    fs::rename(&from, &to).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fs_exists(path: String) -> bool {
    Path::new(&path).exists()
}

#[tauri::command]
pub fn fs_stat(path: String) -> Result<FileEntry, String> {
    let p = Path::new(&path);
    let metadata = fs::metadata(p).map_err(|e| e.to_string())?;
    let modified = metadata
        .modified()
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        })
        .unwrap_or_default();

    Ok(FileEntry {
        name: p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: p.to_string_lossy().to_string(),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        modified,
    })
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SearchMatch {
    pub path: String,
    pub line: usize,
    pub content: String,
}

#[tauri::command]
pub fn fs_search_contents(path: String, query: String, max_results: Option<usize>) -> Result<Vec<SearchMatch>, String> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let q = query.to_lowercase();
    let max = max_results.unwrap_or(100);
    let mut results = Vec::new();
    let mut dirs: Vec<std::path::PathBuf> = vec![dir.to_path_buf()];
    let skip_dirs: Vec<&str> = vec![".git", "node_modules", "target", "build", "dist", ".svelte-kit", "__pycache__", ".svn", ".hg"];

    while let Some(current) = dirs.pop() {
        if results.len() >= max { break; }
        let entries = match fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            if results.len() >= max { break; }
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();

            if entry_path.is_dir() {
                if !skip_dirs.iter().any(|d| name == *d) {
                    dirs.push(entry_path);
                }
                continue;
            }

            // Skip binary-like extensions
            let ext = entry_path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let binary_exts = ["png","jpg","jpeg","gif","webp","bmp","ico","tiff","avif",
                "mp4","webm","ogg","mkv","mov","avi","mp3","wav","flac","aac","m4a","opus",
                "woff","woff2","ttf","eot","otf","pdf","zip","gz","tar","7z","rar","exe","dll","so","dylib","bin"];
            if binary_exts.contains(&ext.as_str()) {
                continue;
            }

            // Try to read as text
            let content = match fs::read_to_string(&entry_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (i, line) in content.lines().enumerate() {
                if results.len() >= max { break; }
                if line.to_lowercase().contains(&q) {
                    results.push(SearchMatch {
                        path: entry_path.to_string_lossy().to_string(),
                        line: i + 1,
                        content: line.to_string(),
                    });
                }
            }
        }
    }

    // Sort by path then line
    results.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    Ok(results)
}

#[tauri::command]
pub fn sys_check_installed(cmd: String) -> bool {
    let mut check_cmd = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("powershell.exe");
        c.args(&["-Command", &format!("Get-Command {} -ErrorAction SilentlyContinue", cmd)]);
        c
    } else {
        let mut c = std::process::Command::new("which");
        c.arg(&cmd);
        c
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        check_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    check_cmd.status().map(|s| s.success()).unwrap_or(false)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GitStatusResult {
    pub branch: String,
    pub untracked: Vec<String>,
    pub modified: Vec<String>,
    pub staged: Vec<String>,
}

#[tauri::command]
pub fn git_get_status(repo_path: String) -> Result<GitStatusResult, String> {
    let mut branch_cmd = std::process::Command::new("git");
    branch_cmd.args(&["rev-parse", "--abbrev-ref", "HEAD"]).current_dir(&repo_path);
    
    let mut status_cmd = std::process::Command::new("git");
    status_cmd.args(&["status", "--porcelain"]).current_dir(&repo_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        branch_cmd.creation_flags(0x08000000);
        status_cmd.creation_flags(0x08000000);
    }

    let branch_output = branch_cmd.output().map_err(|e| e.to_string())?;
    let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

    let status_output = status_cmd.output().map_err(|e| e.to_string())?;
    let status_str = String::from_utf8_lossy(&status_output.stdout);
    
    let mut untracked = Vec::new();
    let mut modified = Vec::new();
    let mut staged = Vec::new();

    for line in status_str.lines() {
        if line.len() < 3 { continue; }
        let code = &line[0..2];
        let file = line[3..].trim().to_string();
        
        match code {
            "??" => untracked.push(file),
            " M" | " D" => modified.push(file),
            "M " | "A " | "D " => staged.push(file),
            _ => modified.push(file),
        }
    }

    Ok(GitStatusResult {
        branch: if branch.is_empty() { "detached".to_string() } else { branch },
        untracked,
        modified,
        staged,
    })
}

#[tauri::command]
pub fn git_commit(repo_path: String, message: String) -> Result<String, String> {
    let mut add_cmd = std::process::Command::new("git");
    add_cmd.args(&["add", "."]).current_dir(&repo_path);

    let mut commit_cmd = std::process::Command::new("git");
    commit_cmd.args(&["commit", "-m", &message]).current_dir(&repo_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        add_cmd.creation_flags(0x08000000);
        commit_cmd.creation_flags(0x08000000);
    }

    let add_status = add_cmd.status().map_err(|e| e.to_string())?;
    if !add_status.success() {
        return Err("Failed to stage changes with git add".to_string());
    }

    let commit_output = commit_cmd.output().map_err(|e| e.to_string())?;
    if commit_output.status.success() {
        Ok(String::from_utf8_lossy(&commit_output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&commit_output.stderr).trim().to_string())
    }
}

#[tauri::command]
pub fn git_stage_file(repo_path: String, file_path: String) -> Result<(), String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(&["add", &file_path]).current_dir(&repo_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to stage file: {}", file_path))
    }
}

#[tauri::command]
pub fn git_unstage_file(repo_path: String, file_path: String) -> Result<(), String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(&["restore", "--staged", &file_path]).current_dir(&repo_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to unstage file: {}", file_path))
    }
}

#[tauri::command]
pub fn git_discard_file(repo_path: String, file_path: String) -> Result<(), String> {
    let path = std::path::Path::new(&repo_path).join(&file_path);
    
    let mut restore_cmd = std::process::Command::new("git");
    restore_cmd.args(&["restore", &file_path]).current_dir(&repo_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        restore_cmd.creation_flags(0x08000000);
    }

    let restore_status = restore_cmd.status().map_err(|e| e.to_string())?;
    
    if !restore_status.success() {
        if path.exists() {
            if path.is_dir() {
                std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
            } else {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn git_stage_all(repo_path: String) -> Result<(), String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(&["add", "."]).current_dir(&repo_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Failed to stage all files".to_string())
    }
}

#[tauri::command]
pub fn git_unstage_all(repo_path: String) -> Result<(), String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(&["reset"]).current_dir(&repo_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Failed to unstage all changes".to_string())
    }
}
