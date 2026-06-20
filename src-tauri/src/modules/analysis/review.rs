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

// ── Rule 1: TODO/FIXME ──
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

// ── Rule 2: Debug prints ──
struct DebugPrintCheck;
impl ReviewRule for DebugPrintCheck {
    fn id(&self) -> &'static str { "no-debug-print" }
    fn description(&self) -> &'static str { "Detect debug print statements" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let patterns: &[&str] = if file.ends_with(".rs") {
            &["println!", "eprintln!", "dbg!("]
        } else if file.ends_with(".js") || file.ends_with(".ts") || file.ends_with(".jsx") || file.ends_with(".tsx") {
            &["console.log", "console.debug", "debugger"]
        } else if file.ends_with(".py") {
            &["print(", "pprint.pprint"]
        } else if file.ends_with(".go") {
            &["fmt.Println", "fmt.Print(", "log.Println"]
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

// ── Rule 3: Long function ──
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
    } else if file.ends_with(".go") {
        if let Some(idx) = line.find("func ") {
            let rest = &line[idx + 5..];
            let name = rest.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
            if !name.is_empty() { return Some(name.to_string()); }
        }
    } else if file.ends_with(".js") || file.ends_with(".ts") || file.ends_with(".jsx") || file.ends_with(".tsx") {
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

// ── Rule 4: Hardcoded secrets ──
struct HardcodedSecretsCheck;
impl ReviewRule for HardcodedSecretsCheck {
    fn id(&self) -> &'static str { "hardcoded-secret" }
    fn description(&self) -> &'static str { "Detect hardcoded API keys, passwords, tokens" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        let secret_patterns = ["api_key", "api_secret", "apiKey", "apiSecret", "password", "passwd",
            "auth_token", "secret_key", "secretKey", "access_token", "bearer", "private_key"];
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") { continue; }
            let lower = trimmed.to_lowercase();
            for pat in &secret_patterns {
                if lower.contains(pat) && (trimmed.contains('=') || trimmed.contains(':')) {
                    findings.push(ReviewFinding {
                        file: file.to_string(),
                        line: i + 1,
                        severity: ReviewSeverity::Error,
                        rule_id: "hardcoded-secret".into(),
                        message: format!("Possible hardcoded secret: `{}`", pat),
                        suggestion: "Use environment variables or a secrets manager instead.".into(),
                    });
                    break;
                }
            }
        }
        findings
    }
}

// ── Rule 5: Empty catch/except ──
struct EmptyCatchCheck;
impl ReviewRule for EmptyCatchCheck {
    fn id(&self) -> &'static str { "empty-catch" }
    fn description(&self) -> &'static str { "Detect empty catch/except blocks that silently swallow errors" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let t = line.trim();
            if file.ends_with(".rs") {
                if t.contains("Err(_)") && t.contains("=>") && line.contains("{}") {
                    findings.push(ReviewFinding {
                        file: file.to_string(),
                        line: i + 1,
                        severity: ReviewSeverity::Error,
                        rule_id: "empty-catch".into(),
                        message: "Empty error match arm that discards error".into(),
                        suggestion: "Handle the error explicitly or log it.".into(),
                    });
                }
            } else if file.ends_with(".js") || file.ends_with(".ts") || file.ends_with(".jsx") || file.ends_with(".tsx") {
                if t.starts_with("catch") && (t.ends_with("{}") || t.contains("{}")) {
                    findings.push(ReviewFinding {
                        file: file.to_string(),
                        line: i + 1,
                        severity: ReviewSeverity::Error,
                        rule_id: "empty-catch".into(),
                        message: "Empty catch block silently swallows error".into(),
                        suggestion: "At minimum log the error, or handle it properly.".into(),
                    });
                }
            } else if file.ends_with(".py") {
                if t.contains("except") && (t.ends_with(":") || t.contains("pass")) {
                    if let Some(next) = content.lines().nth(i + 1) {
                        if next.trim() == "pass" {
                            findings.push(ReviewFinding {
                                file: file.to_string(),
                                line: i + 1,
                                severity: ReviewSeverity::Error,
                                rule_id: "empty-catch".into(),
                                message: "Empty except block silently swallows error".into(),
                                suggestion: "At minimum log the error, or handle it properly.".into(),
                            });
                        }
                    }
                }
            } else if file.ends_with(".go") {
                if t == "_ = err" || t.contains("err != nil") && line.contains("{}") {
                    findings.push(ReviewFinding {
                        file: file.to_string(),
                        line: i + 1,
                        severity: ReviewSeverity::Warning,
                        rule_id: "empty-catch".into(),
                        message: "Error ignored or handled with empty block".into(),
                        suggestion: "Handle the error explicitly.".into(),
                    });
                }
            }
        }
        findings
    }
}

// ── Rule 6: Deep nesting ──
struct DeepNestingCheck;
impl ReviewRule for DeepNestingCheck {
    fn id(&self) -> &'static str { "deep-nesting" }
    fn description(&self) -> &'static str { "Detect overly nested code (>4 levels)" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            let level = if line.starts_with('\t') { indent } else { indent / 2 };
            if level > 4 && line.trim().len() > 0 {
                findings.push(ReviewFinding {
                    file: file.to_string(),
                    line: i + 1,
                    severity: ReviewSeverity::Warning,
                    rule_id: "deep-nesting".into(),
                    message: format!("Deep nesting level ({})", level),
                    suggestion: "Extract inner logic into a separate function.".into(),
                });
                break;
            }
        }
        findings
    }
}

// ── Rule 7: Long lines ──
struct LongLineCheck;
impl ReviewRule for LongLineCheck {
    fn id(&self) -> &'static str { "long-line" }
    fn description(&self) -> &'static str { "Detect lines exceeding 120 characters" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let visible = line.len();
            if visible > 120 {
                findings.push(ReviewFinding {
                    file: file.to_string(),
                    line: i + 1,
                    severity: ReviewSeverity::Info,
                    rule_id: "long-line".into(),
                    message: format!("Line is {} characters long", visible),
                    suggestion: "Break the line into multiple lines for readability.".into(),
                });
            }
        }
        findings
    }
}

// ── Rule 8: Unsafe unwrap (Rust) ──
struct UnsafeUnwrapCheck;
impl ReviewRule for UnsafeUnwrapCheck {
    fn id(&self) -> &'static str { "unsafe-unwrap" }
    fn description(&self) -> &'static str { "Detect unsafe .unwrap() calls that can panic" }
    fn check(&self, file: &str, content: &str) -> Vec<ReviewFinding> {
        let mut findings = Vec::new();
        if !file.ends_with(".rs") { return findings; }
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") { continue; }
            if trimmed.contains(".unwrap(") && !trimmed.contains(".unwrap_or") && !trimmed.contains(".unwrap_err") {
                findings.push(ReviewFinding {
                    file: file.to_string(),
                    line: i + 1,
                    severity: ReviewSeverity::Warning,
                    rule_id: "unsafe-unwrap".into(),
                    message: "Unsafe `.unwrap()` call that can panic at runtime".into(),
                    suggestion: "Use `.unwrap_or()`, `.unwrap_or_else()`, or proper error handling with `?`.".into(),
                });
            }
        }
        findings
    }
}

impl ReviewEngine {
    pub fn default_rules() -> Self {
        Self {
            rules: vec![
                Box::new(TodoCheck),
                Box::new(DebugPrintCheck),
                Box::new(LongFunctionCheck),
                Box::new(HardcodedSecretsCheck),
                Box::new(EmptyCatchCheck),
                Box::new(DeepNestingCheck),
                Box::new(LongLineCheck),
                Box::new(UnsafeUnwrapCheck),
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
