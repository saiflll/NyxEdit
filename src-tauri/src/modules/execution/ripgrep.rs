use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgMatch {
    pub file: String,
    pub line_number: u32,
    pub text: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RgResult {
    pub matches: Vec<RgMatch>,
    pub total: usize,
    pub elapsed_ms: u64,
}

struct CacheEntry {
    root: String,
    pattern: String,
    result: RgResult,
    cached_at: Instant,
}

fn cache() -> &'static Mutex<Vec<CacheEntry>> {
    static CACHE: std::sync::OnceLock<Mutex<Vec<CacheEntry>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

fn cache_ttl() -> u64 { 30 }

fn get_cached(root: &str, pattern: &str) -> Option<RgResult> {
    if let Ok(mut guard) = cache().lock() {
        let now = Instant::now();
        guard.retain(|e| now.duration_since(e.cached_at).as_secs() < cache_ttl());
        guard.iter()
            .find(|e| e.root == root && e.pattern == pattern)
            .map(|e| e.result.clone())
    } else { None }
}

fn set_cached(root: &str, pattern: &str, result: &RgResult) {
    if let Ok(mut guard) = cache().lock() {
        guard.retain(|e| e.root != root || e.pattern != pattern);
        guard.push(CacheEntry {
            root: root.to_string(),
            pattern: pattern.to_string(),
            result: result.clone(),
            cached_at: Instant::now(),
        });
    }
}

/// Clear the in-memory scan cache
pub fn clear_cache() {
    if let Ok(mut guard) = cache().lock() {
        guard.clear();
    }
}

/// Search files using ripgrep. Returns structured matches.
/// Results are cached in-memory for 30s to avoid redundant searches.
pub fn search(pattern: &str, root: &Path, max_results: Option<usize>) -> Result<RgResult, String> {
    let root_str = root.to_string_lossy().to_string();

    if let Some(cached) = get_cached(&root_str, pattern) {
        return Ok(cached);
    }

    let start = Instant::now();

    let mut cmd = Command::new("rg");
    cmd.arg("--json")
        .arg("--line-number")
        .arg("--no-heading")
        .arg("--color")
        .arg("never")
        .arg(pattern)
        .arg(root.to_string_lossy().as_ref());

    if let Some(max) = max_results {
        cmd.arg("-m").arg(max.to_string());
    }

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "ripgrep (rg) is not installed. Install it from https://github.com/BurntSushi/ripgrep".to_string()
        } else {
            format!("ripgrep execution error: {}", e)
        }
    })?;

    let elapsed = start.elapsed().as_millis() as u64;

    if !output.status.success() && !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            return Err(format!("ripgrep error: {}", stderr.trim()));
        }
    }

    let mut matches = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if entry["type"] == "match" {
                let data = &entry["data"];
                matches.push(RgMatch {
                    file: data["path"]["text"].as_str().unwrap_or("").to_string(),
                    line_number: data["line_number"].as_u64().unwrap_or(0) as u32,
                    text: data["lines"]["text"].as_str().unwrap_or("").trim().to_string(),
                });
            }
        }
    }

    let total = matches.len();
    let result = RgResult { matches, total, elapsed_ms: elapsed };
    set_cached(&root_str, pattern, &result);
    Ok(result)
}

/// Search for a specific pattern and return human-readable summary.
pub fn search_text(pattern: &str, root: &Path, max_results: Option<usize>) -> Result<String, String> {
    let result = search(pattern, root, max_results)?;
    if result.matches.is_empty() {
        return Ok(format!("No matches found for pattern '{}' ({}ms)", pattern, result.elapsed_ms));
    }
    let mut out = format!("Found {} matches in {}ms:\n", result.total, result.elapsed_ms);
    for m in result.matches.iter().take(20) {
        out.push_str(&format!("  {}:{}  {}\n", m.file, m.line_number, m.text));
    }
    if result.matches.len() > 20 {
        out.push_str(&format!("  ... and {} more matches\n", result.matches.len() - 20));
    }
    Ok(out)
}
