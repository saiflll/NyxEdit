use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tokio::process::Command;
use tokio::sync::mpsc;

/// ── CLI Discovery ───────────────────────────────────────────────────────────

static DISCOVERED: OnceLock<Mutex<HashMap<String, CliInfo>>> = OnceLock::new();

fn discovered() -> &'static Mutex<HashMap<String, CliInfo>> {
    DISCOVERED.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliInfo {
    pub name: String,
    pub path: String,
    pub version: Option<String>,
    pub has_json_mode: bool,
    pub has_interactive: bool,
}

/// Known CLI agents and their detection config.
const KNOWN_CLIS: &[(&str, &[&str])] = &[
    ("claude",   &["--version"]),
    ("gemini",   &["--version"]),
    ("opencode", &["--version"]),
    ("aider",    &["--version"]),
    ("codex",    &["--version"]),
    ("agy",      &["--version"]),
    ("plandex",  &["--version"]),
    ("crush",    &["--version"]),
    ("goose",    &["--version"]),
    ("qodo",     &["--version"]),
];

/// Returns `true` if the given CLI tool is installed (found in PATH).
pub fn is_installed(name: &str) -> bool {
    discovered().lock().unwrap().contains_key(name)
}

/// Returns the cached CliInfo for a tool, or `None` if not discovered.
pub fn get_info(name: &str) -> Option<CliInfo> {
    discovered().lock().unwrap().get(name).cloned()
}

/// Returns all discovered CLI tools.
pub fn list_discovered() -> Vec<CliInfo> {
    discovered().lock().unwrap().values().cloned().collect()
}

/// Probe PATH for all known CLI tools and cache results.
pub async fn discover_all() {
    for (name, args) in KNOWN_CLIS {
        match probe_cli(name, args).await {
            Some(info) => {
                discovered().lock().unwrap().insert(name.to_string(), info);
            }
            None => {
                discovered().lock().unwrap().remove::<str>(name);
            }
        }
    }
}

/// Probe a single CLI binary.
async fn probe_cli(name: &str, version_args: &[&str]) -> Option<CliInfo> {
    let path = which(name)?;
    let version = get_version(&path, version_args).await;
    let has_json = has_json_flag(name);
    let has_interactive = has_interactive_mode(name);

    Some(CliInfo {
        name: name.to_string(),
        path: path.to_string(),
        version,
        has_json_mode: has_json,
        has_interactive,
    })
}

/// Cross-platform `which` via `std::process::Command` (non-tokio because it's quick).
fn which(name: &str) -> Option<String> {
    let found = if cfg!(target_os = "windows") {
        let output = std::process::Command::new("where").arg(name).output().ok()?;
        if !output.status.success() { return None; }
        String::from_utf8_lossy(&output.stdout).lines().next().map(|s| s.trim().to_string())
    } else {
        let output = std::process::Command::new("which").arg(name).output().ok()?;
        if !output.status.success() { return None; }
        String::from_utf8_lossy(&output.stdout).lines().next().map(|s| s.trim().to_string())
    };
    found
}

async fn get_version(path: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(path)
        .args(args)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next().unwrap_or(&stdout);
    Some(first.trim().to_string())
}

fn has_json_flag(name: &str) -> bool {
    matches!(name, "claude" | "gemini" | "opencode" | "codex")
}

fn has_interactive_mode(name: &str) -> bool {
    matches!(name, "claude" | "gemini" | "opencode" | "aider" | "agy")
}

/// ── CLI Session ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliSessionInfo {
    pub id: String,
    pub cli_name: String,
    pub status: String, // "running" | "idle" | "exited"
    pub created_at: String,
}

pub struct CliSession {
    pub id: String,
    pub cli_name: String,
    pub child: Option<tokio::process::Child>,
    pub stdin: Option<tokio::process::ChildStdin>,
    pub tx: mpsc::Sender<String>,
    pub rx: Option<mpsc::Receiver<String>>,
}

/// ── CliAdapter Trait ────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait CliAdapter: Send + Sync {
    /// Human-readable name (matches CliInfo.name).
    fn name(&self) -> &str;

    /// Generate the CLI argument list for a given prompt.
    fn build_args(&self, prompt: &str, cwd: Option<&str>, json: bool) -> Vec<String>;

    /// Execute once and return the full output as a string.
    async fn execute(&self, prompt: &str, cwd: Option<&str>) -> Result<String, String> {
        let bin = get_info(self.name())
            .ok_or_else(|| format!("{} CLI not installed", self.name()))?;
        let args = self.build_args(prompt, cwd, bin.has_json_mode);

        let mut cmd = Command::new(&bin.path);
        cmd.args(&args);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            cmd.output(),
        )
        .await
        .map_err(|_| format!("{} timed out after 300s", self.name()))?
        .map_err(|e| format!("{} execution error: {}", self.name(), e))?;

        let mut result = String::new();
        if !output.stdout.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        Ok(result)
    }

    /// Spawn a streaming session. Returns a receiver that yields output lines.
    async fn execute_stream(
        &self,
        prompt: &str,
        cwd: Option<&str>,
    ) -> Result<(String, mpsc::Receiver<String>), String> {
        let bin = get_info(self.name())
            .ok_or_else(|| format!("{} CLI not installed", self.name()))?;
        let args = self.build_args(prompt, cwd, false);

        let mut cmd = Command::new(&bin.path);
        cmd.args(&args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd.spawn().map_err(|e| format!("spawn {}: {}", self.name(), e))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| "no stdout".to_string())?;
        let stderr = child.stderr.take()
            .ok_or_else(|| "no stderr".to_string())?;

        let (tx, rx) = mpsc::channel(256);
        let sid = self.name().to_string();

        // Reader thread: stdout
        let tx_out = tx.clone();
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stdout);
            use tokio::io::AsyncBufReadExt;
            let mut line = String::new();
            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                if tx_out.send(line.clone()).await.is_err() {
                    break;
                }
                line.clear();
            }
        });

        // Reader thread: stderr
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            use tokio::io::AsyncBufReadExt;
            let mut line = String::new();
            while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                if tx.send(line.clone()).await.is_err() {
                    break;
                }
                line.clear();
            }
        });

        Ok((sid, rx))
    }
}

/// ── Built-in Adapters ───────────────────────────────────────────────────────

pub struct ClaudeAdapter;
pub struct GeminiAdapter;
pub struct OpenCodeAdapter;
pub struct AiderAdapter;
pub struct CodexAdapter;
pub struct AgyAdapter;

#[async_trait::async_trait]
impl CliAdapter for ClaudeAdapter {
    fn name(&self) -> &str { "claude" }
    fn build_args(&self, prompt: &str, _cwd: Option<&str>, json: bool) -> Vec<String> {
        let mut args = vec!["-p".into(), prompt.to_string()];
        if json { args.push("--output-format".into()); args.push("json".into()); }
        args
    }
}

#[async_trait::async_trait]
impl CliAdapter for GeminiAdapter {
    fn name(&self) -> &str { "gemini" }
    fn build_args(&self, prompt: &str, _cwd: Option<&str>, _json: bool) -> Vec<String> {
        vec!["-p".into(), prompt.to_string()]
    }
}

#[async_trait::async_trait]
impl CliAdapter for OpenCodeAdapter {
    fn name(&self) -> &str { "opencode" }
    fn build_args(&self, prompt: &str, _cwd: Option<&str>, json: bool) -> Vec<String> {
        let mut args = vec!["run".into(), prompt.to_string()];
        if json { args.push("--json".into()); }
        args
    }
}

#[async_trait::async_trait]
impl CliAdapter for AiderAdapter {
    fn name(&self) -> &str { "aider" }
    fn build_args(&self, prompt: &str, _cwd: Option<&str>, _json: bool) -> Vec<String> {
        vec!["--message".into(), prompt.to_string(), "--yes".into()]
    }
}

#[async_trait::async_trait]
impl CliAdapter for CodexAdapter {
    fn name(&self) -> &str { "codex" }
    fn build_args(&self, prompt: &str, _cwd: Option<&str>, json: bool) -> Vec<String> {
        let mut args = vec!["-p".into(), prompt.to_string()];
        if json { args.push("--json".into()); }
        args
    }
}

#[async_trait::async_trait]
impl CliAdapter for AgyAdapter {
    fn name(&self) -> &str { "agy" }
    fn build_args(&self, prompt: &str, _cwd: Option<&str>, _json: bool) -> Vec<String> {
        vec![prompt.to_string()]
    }
}

/// ── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cli_discover() -> Result<Vec<CliInfo>, String> {
    discover_all().await;
    Ok(list_discovered())
}

#[tauri::command]
pub async fn cli_list() -> Vec<CliInfo> {
    list_discovered()
}

#[tauri::command]
pub async fn cli_execute(name: String, prompt: String, cwd: Option<String>) -> Result<String, String> {
    let adapter: Box<dyn CliAdapter> = match name.as_str() {
        "claude" => Box::new(ClaudeAdapter),
        "gemini" => Box::new(GeminiAdapter),
        "opencode" => Box::new(OpenCodeAdapter),
        "aider" => Box::new(AiderAdapter),
        "codex" => Box::new(CodexAdapter),
        "agy" => Box::new(AgyAdapter),
        _ => return Err(format!("Unknown CLI: {}", name)),
    };
    adapter.execute(&prompt, cwd.as_deref()).await
}

/// Returns a registry of all CLI adapters (for use in tool registration).
pub fn all_adapters() -> Vec<Box<dyn CliAdapter>> {
    vec![
        Box::new(ClaudeAdapter),
        Box::new(GeminiAdapter),
        Box::new(OpenCodeAdapter),
        Box::new(AiderAdapter),
        Box::new(CodexAdapter),
        Box::new(AgyAdapter),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_cache() {
        discover_all().await;
        let list = list_discovered();
        // Should not crash; may be empty if CI has no CLIs installed.
        assert!(list.iter().all(|c| !c.name.is_empty()));
    }

    #[test]
    fn test_known_cli_list() {
        assert!(KNOWN_CLIS.len() >= 6);
    }

    #[tokio::test]
    async fn test_claude_adapter_args() {
        let a = ClaudeAdapter;
        let args = a.build_args("hello", None, true);
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"hello".to_string()));
        assert!(args.contains(&"--output-format".to_string()));
    }

    #[tokio::test]
    async fn test_aider_adapter_args() {
        let a = AiderAdapter;
        let args = a.build_args("fix bug", None, false);
        assert!(args.contains(&"--yes".to_string()));
        assert!(args.contains(&"--message".to_string()));
    }

    #[tokio::test]
    async fn test_execute_not_installed() {
        // Should return error if CLI not found
        let a = AgyAdapter;
        let result = a.execute("test", None).await;
        // Expected to fail because agy is likely not installed in test env
        assert!(result.is_err());
    }
}
