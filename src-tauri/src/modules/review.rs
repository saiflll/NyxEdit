use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ReviewSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ReviewFinding {
    pub file: String,
    pub line: usize,
    pub severity: ReviewSeverity,
    pub rule_id: String,
    pub message: String,
    pub suggestion: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ReviewResult {
    pub findings: Vec<ReviewFinding>,
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
}

pub struct ReviewEngine {
    pub rules: Vec<Box<dyn ReviewRule + Send + Sync>>,
}

pub trait ReviewRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding>;
}

struct TodoCheck;
impl ReviewRule for TodoCheck {
    fn id(&self) -> &'static str { "no-todo" }
    fn description(&self) -> &'static str { "Detect TODO/FIXME comments" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let lower = line.to_lowercase();
            if lower.contains("todo") || lower.contains("fixme") || lower.contains("xxx") {
                findings.push(ReviewFinding {
                    file: file.to_string(),
                    line: i + 1,
                    severity: ReviewSeverity::Warning,
                    rule_id: "no-todo".into(),
                    message: "Unresolved TODO/FIXME comment".into(),
                    suggestion: "Resolve the task or create a tracking issue.".into(),
                });
            }
        }
        findings
    }
}

struct DebugPrintCheck;
impl ReviewRule for DebugPrintCheck {
    fn id(&self) -> &'static str { "no-debug-print" }
    fn description(&self) -> &'static str { "Detect debug print statements" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let patterns: &[&str] = if file.ends_with(".rs") {
            &["println!", "eprintln!", "dbg!("]
        } else if file.ends_with(".js") || file.ends_with(".ts") {
            &["console.log", "console.debug", "debugger"]
        } else if file.ends_with(".py") {
            &["print(", "pprint.pprint"]
        } else { return findings };

        for (i, line) in content.lines().enumerate() {
            for pat in patterns {
                if line.contains(pat) {
                    findings.push(ReviewFinding {
                        file: file.to_string(),
                        line: i + 1,
                        severity: ReviewSeverity::Warning,
                        rule_id: "no-debug-print".into(),
                        message: format!("Debug print statement: `{}`", pat),
                        suggestion: "Remove before committing, or use a proper logging framework.".into(),
                    });
                }
            }
        }
        findings
    }
}

struct LongFunctionCheck;
impl ReviewRule for LongFunctionCheck {
    fn id(&self) -> &'static str { "long-function" }
    fn description(&self) -> &'static str { "Detect overly long functions (>50 lines)" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let mut brace_depth = 0i32;
        let mut fn_start = 0usize;
        let mut fn_name = String::new();
        let mut in_fn = false;

        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            if !in_fn {
                if let Some(name) = extract_fn_name(line, file) {
                    fn_name = name;
                    fn_start = i;
                    in_fn = true;
                    brace_depth = line.chars().filter(|c| *c == '{').count() as i32
                        - line.chars().filter(|c| *c == '}').count() as i32;
                }
            } else {
                brace_depth += line.chars().filter(|c| *c == '{').count() as i32;
                brace_depth -= line.chars().filter(|c| *c == '}').count() as i32;
                if brace_depth <= 0 {
                    let length = i - fn_start + 1;
                    if length > 50 {
                        findings.push(ReviewFinding {
                            file: file.to_string(),
                            line: fn_start + 1,
                            severity: ReviewSeverity::Warning,
                            rule_id: "long-function".into(),
                            message: format!("Function `{}` is {} lines long", fn_name, length),
                            suggestion: "Consider refactoring into smaller functions.".into(),
                        });
                    }
                    in_fn = false;
                }
            }
        }
        findings
    }
}

fn extract_fn_name(line: &str, file: &str) -> Option<String> {
    if file.ends_with(".rs") {
        let cleaned = line.trim_start_matches("pub ").trim_start_matches("async ").trim_start_matches("unsafe ");
        if let Some(idx) = cleaned.find("fn ") {
            let rest = &cleaned[idx + 3..];
            let name = rest.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
            if !name.is_empty() { return Some(name.to_string()); }
        }
    } else if file.ends_with(".js") || file.ends_with(".ts") {
        if let Some(idx) = line.find("function ") {
            let rest = &line[idx + 9..];
            let name = rest.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
            if !name.is_empty() { return Some(name.to_string()); }
        }
    } else if file.ends_with(".py") {
        if let Some(idx) = line.find("def ") {
            let rest = &line[idx + 4..];
            let name = rest.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
            if !name.is_empty() { return Some(name.to_string()); }
        }
    }
    None
}

impl ReviewEngine {
    pub fn default_rules() -> Self {
        Self {
            rules: vec![
                Box::new(TodoCheck),
                Box::new(DebugPrintCheck),
                Box::new(LongFunctionCheck),
            ],
        }
    }

    pub fn review_file(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        for rule in &self.rules {
            findings.extend(rule.check(file, content));
        }
        findings
    }

    pub fn review_text(&self, text: &str) -> ReviewResult {
        let mut findings = Vec::new();
        for file_chunk in text.split("```") {
            if let Some((header, body)) = file_chunk.split_once('\n') {
                let file = header.trim();
                if !file.is_empty() && !file.contains(' ') {
                    findings.extend(self.review_file(file, body));
                }
            }
        }
        let errors = findings.iter().filter(|f| matches!(f.severity, ReviewSeverity::Error)).count();
        let warnings = findings.len() - errors;
        ReviewResult { total: findings.len(), errors, warnings, findings }
    }
}

pub struct ReviewState {
    pub engine: Arc<Mutex<ReviewEngine>>,
}

impl ReviewState {
    pub fn new() -> Self {
        Self { engine: Arc::new(Mutex::new(ReviewEngine::default_rules())) }
    }
}

impl Default for ReviewState {
    fn default() -> Self { Self::new() }
}

#[tauri::command]
pub fn review_text(
    state: tauri::State<'_, ReviewState>,
    text: String,
) -> Result<ReviewResult, String> {
    let engine = state.engine.lock().map_err(|e| format!("Lock: {}", e))?;
    Ok(engine.review_text(&text))
}

#[tauri::command]
pub fn review_file(
    state: tauri::State<'_, ReviewState>,
    file_path: String,
    content: String,
) -> Result<Vec<ReviewFinding>, String> {
    let engine = state.engine.lock().map_err(|e| format!("Lock: {}", e))?;
    Ok(engine.review_file(&file_path, &content))
}
