use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use super::local::FileEntry;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SearchMatch {
    pub path: String,
    pub line: usize,
    pub content: String,
}

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
