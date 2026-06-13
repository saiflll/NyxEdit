use serde::{Deserialize, Serialize};

pub fn run_git(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(args).current_dir(repo_path);
    #[cfg(target_os = "windows")] {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let out = cmd.output().map_err(|e| format!("git error: {}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "git command failed".into() } else { err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn git_init(repo_path: String) -> Result<(), String> {
    run_git(&repo_path, &["init"])?;
    Ok(())
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GitCommitEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

pub fn git_log(repo_path: String, max_count: Option<u32>) -> Result<Vec<GitCommitEntry>, String> {
    let count = max_count.unwrap_or(20).to_string();
    let out = run_git(&repo_path, &[
        "log", &format!("--max-count={}", count),
        "--format=%H||%an||%ai||%s",
    ])?;
    let entries = out.lines().filter(|l| !l.is_empty()).map(|line| {
        let parts: Vec<&str> = line.splitn(4, "||").collect();
        GitCommitEntry {
            hash: parts.get(0).unwrap_or(&"").to_string(),
            author: parts.get(1).unwrap_or(&"").to_string(),
            date: parts.get(2).unwrap_or(&"").to_string(),
            message: parts.get(3).unwrap_or(&"").to_string(),
        }
    }).collect();
    Ok(entries)
}

pub fn git_push(repo_path: String, remote: Option<String>, branch: Option<String>) -> Result<String, String> {
    let remote = remote.unwrap_or_else(|| "origin".into());
    let branch = branch.unwrap_or_else(|| {
        run_git(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|_| "main".into())
    });
    let out = run_git(&repo_path, &["push", "-u", &remote, &branch])?;
    Ok(out)
}

pub fn git_pull(repo_path: String, remote: Option<String>, branch: Option<String>) -> Result<String, String> {
    let remote = remote.unwrap_or_else(|| "origin".into());
    let branch = branch.unwrap_or_else(|| {
        run_git(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|_| "main".into())
    });
    let out = run_git(&repo_path, &["pull", &remote, &branch])?;
    Ok(out)
}

pub fn git_sync(repo_path: String) -> Result<String, String> {
    let branch = run_git(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|_| "main".into());
    let remote = run_git(&repo_path, &["remote", "get-url", "origin"]).unwrap_or_default();
    if remote.is_empty() {
        return Err("No remote configured. Set a remote first.".into());
    }
    run_git(&repo_path, &["fetch", "--all"])?;
    run_git(&repo_path, &["pull", "origin", &branch])?;
    let out = run_git(&repo_path, &["push", "-u", "origin", &branch])?;
    Ok(out)
}

pub fn git_diff_uncommitted(repo_path: String, staged: bool) -> Result<String, String> {
    let args = if staged { &["diff", "--cached"] as &[&str] } else { &["diff"] as &[&str] };
    run_git(&repo_path, args)
}

pub fn git_remote_url(repo_path: String) -> Result<String, String> {
    run_git(&repo_path, &["remote", "get-url", "origin"])
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GitStatusResult {
    pub branch: String,
    pub untracked: Vec<String>,
    pub modified: Vec<String>,
    pub staged: Vec<String>,
}

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
