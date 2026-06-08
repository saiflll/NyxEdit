use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

#[derive(Clone, Serialize)]
pub struct AgentPersona {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub instructions: &'static str,
}

pub const BUILTIN_PERSONAS: &[AgentPersona] = &[
    AgentPersona {
        id: "coder",
        name: "Coder",
        description: "General-purpose coding assistant. Writes, edits, and runs.",
        icon: "coder",
        instructions: "You are an expert software engineer pair-programming inside the user's terminal.
- Read files before editing them. Match existing patterns and naming.
- Prefer the smallest correct change. Don't refactor adjacent code unprompted.
- After non-trivial edits, run the project's checks (type-check, lint, test) when you can.
- Keep responses tight: short prose, code blocks with language fences.",
    },
    AgentPersona {
        id: "architect",
        name: "Architect",
        description: "Design and tradeoffs. Plans before code.",
        icon: "architect",
        instructions: "You are a senior software architect.
- Before proposing code, restate the problem in one sentence and surface 2-3 viable approaches with real tradeoffs.
- Recommend one with reasoning. Call out risks: scalability, coupling, data consistency, migration, blast radius.
- Reference the actual repo (read key files) before generalizing. No hand-wavy advice.
- Output structure: Problem - Options - Recommendation - Risks - Next steps.",
    },
    AgentPersona {
        id: "reviewer",
        name: "Code Reviewer",
        description: "Reviews diffs for correctness, perf, security.",
        icon: "reviewer",
        instructions: "You are a meticulous code reviewer.
- Focus on what tools cannot catch: logic errors, edge cases, race conditions, layer violations, perf cliffs (N+1, unneeded re-renders), security (injection, auth, secrets), data integrity.
- Skip formatting / naming / inferred-type nits - linters handle those.
- Output: `[MUST/SHOULD/NIT] file:line - issue -> fix`. If nothing real, say 'Looks good.'",
    },
    AgentPersona {
        id: "security",
        name: "Security",
        description: "Threat-models changes and flags vulns.",
        icon: "security",
        instructions: "You are an application-security engineer.
- Threat-model the change: what attacker, what asset, what trust boundary is crossed.
- Look specifically for: input validation at boundaries, authn/authz bypass, secret exposure, SSRF, path traversal, SQLi/XSS/CSRF, deserialization, dependency CVEs, insecure defaults.
- For each finding: severity, exploit sketch, concrete fix. Prefer fixes that close the class of bug, not the one report.
- If the change is benign, say so explicitly - don't fabricate findings.",
    },
    AgentPersona {
        id: "designer",
        name: "Designer",
        description: "UI/UX critique and refinement.",
        icon: "designer",
        instructions: "You are a senior product designer with a strong taste for restrained, modern UI.
- Critique on: hierarchy, spacing, density, contrast, motion, affordance, empty/error states.
- Propose concrete changes, with CSS values when helpful. Keep consistent with the surrounding design system.
- Avoid generic 'make it pop' advice. Be specific about what's wrong and why.",
    },
];

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub capabilities: Vec<String>,
    pub temperature: Option<f32>,
    pub system_prompt: Option<String>,
    pub persona_id: Option<String>,
    pub built_in: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AgentResponse {
    pub agent_id: String,
    pub content: String,
    pub provider: String,
    pub model: String,
}

#[derive(Clone, Serialize)]
pub struct AiStreamChunk {
    pub delta: String,
}

#[derive(Clone, Serialize)]
pub struct AiStreamDone {
    pub content: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
}

#[derive(Clone, Serialize)]
pub struct AiStreamError {
    pub error: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct AgentUsage {
    pub agent_id: String,
    pub agent_name: String,
    pub provider: String,
    pub model: String,
    pub total_requests: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost: f64,
}

fn model_price(model: &str) -> (f64, f64) {
    let m = model.to_lowercase();
    if m.contains("gpt-4o-mini") {
        (0.00015, 0.0006)
    } else if m.contains("gpt-4o") {
        (0.0025, 0.01)
    } else if m.contains("gpt-4") && !m.contains("turbo") {
        (0.03, 0.06)
    } else if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") {
        (0.01, 0.03)
    } else if m.contains("gpt-3.5-turbo") {
        (0.0005, 0.0015)
    } else if m.contains("claude-3-opus") || m.contains("claude-opus") {
        (0.015, 0.075)
    } else if m.contains("claude-3-sonnet") || m.contains("claude-sonnet") {
        (0.003, 0.015)
    } else if m.contains("claude-3-haiku") || m.contains("claude-haiku") {
        (0.00025, 0.00125)
    } else if m.contains("gemini-1.5-pro") || m.contains("gemini-pro") {
        (0.00125, 0.005)
    } else if m.contains("gemini-1.5-flash") || m.contains("gemini-flash") {
        (0.000075, 0.0003)
    } else if m.contains("gemini-2.0") || m.contains("gemini-2") {
        (0.0001, 0.0004)
    } else if m.contains("deepseek-chat") || m.contains("deepseek-v3") {
        (0.00014, 0.00028)
    } else if m.contains("deepseek-reasoner") || m.contains("deepseek-r1") {
        (0.00055, 0.00219)
    } else if m.contains("llama") || m.contains("mistral") || m.contains("mixtral") || m.contains("qwen") || m.contains("deepseek") {
        (0.0005, 0.0015)
    } else {
        (0.001, 0.002)
    }
}

pub struct AiManager {
    pub agents: Arc<Mutex<HashMap<String, AgentConfig>>>,
    pub usage: Arc<Mutex<HashMap<String, AgentUsage>>>,
}

impl AiManager {
    pub fn new() -> Self {
        let agents = HashMap::new();
        Self {
            agents: Arc::new(Mutex::new(agents)),
            usage: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn record_usage(&self, agent_id: &str, input_tokens: u64, output_tokens: u64, cost: f64) {
        let mut usage = self.usage.lock().unwrap();
        let entry = usage.entry(agent_id.to_string()).or_insert_with(|| {
            let agents = self.agents.lock().unwrap();
            let agent = agents.get(agent_id);
            AgentUsage {
                agent_id: agent_id.to_string(),
                agent_name: agent.map(|a| a.name.clone()).unwrap_or_default(),
                provider: agent.map(|a| a.provider.clone()).unwrap_or_default(),
                model: agent.map(|a| a.model.clone()).unwrap_or_default(),
                total_requests: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cost: 0.0,
            }
        });
        entry.total_requests += 1;
        entry.total_input_tokens += input_tokens;
        entry.total_output_tokens += output_tokens;
        entry.total_cost += cost;
    }

    pub fn get_usage(&self) -> Vec<AgentUsage> {
        let usage = self.usage.lock().unwrap();
        let mut list: Vec<AgentUsage> = usage.values().cloned().collect();
        list.sort_by(|a, b| b.total_cost.partial_cmp(&a.total_cost).unwrap_or(std::cmp::Ordering::Equal));
        list
    }

    pub fn reset_usage(&self) {
        let mut usage = self.usage.lock().unwrap();
        usage.clear();
    }

    pub fn get_agent(&self, agent_id: &str) -> Option<AgentConfig> {
        let agents = self.agents.lock().unwrap();
        agents.get(agent_id).cloned()
    }

    pub fn list_agents(&self) -> Vec<AgentConfig> {
        let agents = self.agents.lock().unwrap();
        agents.values().cloned().collect()
    }

    pub fn update_agent(&self, config: AgentConfig) -> Result<(), String> {
        let mut agents = self.agents.lock().unwrap();
        agents.insert(config.id.clone(), config);
        Ok(())
    }

    pub fn remove_agent(&self, agent_id: &str) -> Result<(), String> {
        let mut agents = self.agents.lock().unwrap();
        agents.remove(agent_id);
        Ok(())
    }
}

fn resolve_system_prompt(agent: &AgentConfig) -> String {
    if let Some(pid) = &agent.persona_id {
        if let Some(persona) = BUILTIN_PERSONAS.iter().find(|p| p.id == pid.as_str()) {
            return persona.instructions.to_string();
        }
    }
    agent.system_prompt.clone().unwrap_or_default()
}

fn default_base_url(provider: &str) -> &'static str {
    match provider {
        "vercel" => "https://ai-gateway.vercel.sh/v1",
        "ollama" => "http://localhost:11434/v1",
        _ => "",
    }
}

fn build_request(agent: &AgentConfig, messages: &[ChatMessage]) -> Result<reqwest::blocking::Request, String> {
    let base_url = agent.base_url.clone().unwrap_or_else(|| default_base_url(&agent.provider).to_string());

    let mut chat_messages: Vec<Value> = Vec::new();
    let sp = resolve_system_prompt(agent);
    if !sp.is_empty() {
        chat_messages.push(serde_json::json!({
            "role": "system",
            "content": sp
        }));
    }
    for m in messages {
        chat_messages.push(serde_json::json!({
            "role": m.role,
            "content": m.content
        }));
    }

    let body = serde_json::json!({
        "model": agent.model,
        "messages": chat_messages,
        "temperature": agent.temperature.unwrap_or(0.7),
        "max_tokens": 4096,
    });

    let client = reqwest::blocking::Client::new();
    let mut req_builder = client.post(format!("{}/chat/completions", base_url)).json(&body);

    if let Some(key) = &agent.api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
    }

    req_builder.build().map_err(|e: reqwest::Error| e.to_string())
}

#[tauri::command]
pub async fn ai_chat(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
    agent_id: String,
    messages: Vec<ChatMessage>,
) -> Result<AgentResponse, String> {
    let mut agent = state.get_agent(&agent_id).ok_or("Agent not found")?;

    // Load actual API key dynamically from Keychain
    if let Ok(Some(real_key)) = crate::modules::secrets::get_secret(&app, &secrets_state, "codlib-ai", &agent_id) {
        agent.api_key = Some(real_key);
    } else if let Some(ref key) = agent.api_key {
        if key == "********" {
            return Err("API key is masked but not found in the OS Keychain/Credential Store.".to_string());
        }
    }

    let agent_clone = agent.clone();
    let messages_clone = messages.clone();

    let (content, input_tokens, output_tokens) = tokio::task::spawn_blocking(move || {
        let req = build_request(&agent_clone, &messages_clone)?;
        let client = reqwest::blocking::Client::new();
        let resp = client.execute(req).map_err(|e: reqwest::Error| e.to_string())?;
        let status = resp.status();
        let body: Value = resp.json().map_err(|e: reqwest::Error| e.to_string())?;
        if !status.is_success() {
            return Err(format!("AI request failed ({}): {}", status, body));
        }
        Ok((
            body["choices"][0]["message"]["content"]
                .as_str().unwrap_or("").to_string(),
            body["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            body["usage"]["completion_tokens"].as_u64().unwrap_or(0),
        ))
    }).await.map_err(|e| format!("Task join error: {}", e))??;

    let (price_in, price_out) = model_price(&agent.model);
    let cost = (input_tokens as f64 * price_in + output_tokens as f64 * price_out) / 1000.0;
    state.record_usage(&agent.id, input_tokens, output_tokens, cost);

    Ok(AgentResponse {
        agent_id: agent.id.clone(),
        content,
        provider: agent.provider.clone(),
        model: agent.model.clone(),
    })
}

async fn stream_openai(
    app: &tauri::AppHandle,
    agent: &AgentConfig,
    messages: &[ChatMessage],
    base_url: &str,
) -> Result<(String, u64, u64), String> {
    let mut chat_messages: Vec<Value> = Vec::new();
    let sp = resolve_system_prompt(agent);
    if !sp.is_empty() {
        chat_messages.push(serde_json::json!({
            "role": "system",
            "content": sp
        }));
    }
    for m in messages {
        chat_messages.push(serde_json::json!({"role": m.role, "content": m.content}));
    }

    let mut body = serde_json::json!( {
        "model": agent.model,
        "messages": chat_messages,
        "temperature": agent.temperature.unwrap_or(0.7),
        "max_tokens": 4096,
        "stream": true,
    });
    body["stream_options"] = serde_json::json!({"include_usage": true});

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req_builder = client
        .post(format!("{}/chat/completions", base_url))
        .json(&body);

    if let Some(key) = &agent.api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
    }

    let response = req_builder.send().await.map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("AI request failed ({}): {}", status, text));
    }

    let mut buffer = String::new();
    let mut full_content = String::new();
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;

    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() { continue; }
            if !line.starts_with("data: ") { continue; }

            let data = line[6..].trim().to_string();
            if data == "[DONE]" { continue; }

            if let Ok(json) = serde_json::from_str::<Value>(&data) {
                if let Some(delta) = json["choices"][0]["delta"]["content"].as_str() {
                    if !delta.is_empty() {
                        full_content.push_str(delta);
                        let _ = app.emit("ai:chunk", AiStreamChunk { delta: delta.to_string() });
                    }
                }
                if let Some(usage) = json["usage"].as_object() {
                    if let Some(v) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) { input_tokens = v; }
                    if let Some(v) = usage.get("completion_tokens").and_then(|v| v.as_u64()) { output_tokens = v; }
                }
            }
        }
    }

    Ok((full_content, input_tokens, output_tokens))
}

// ── Tool types for ReAct loop ──────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Build the tools array that gets sent with every LLM request.
fn build_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read_file".into(),
            description: "Read the contents of a file. Use offset/limit to page large files.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the file"},
                    "offset": {"type": "integer", "description": "Line number to start from (0-indexed)", "default": 0},
                    "limit": {"type": "integer", "description": "Max lines to read", "default": 2000}
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "write_file".into(),
            description: "Create a new file or overwrite an existing one with new content.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the file"},
                    "content": {"type": "string", "description": "Full file content"}
                },
                "required": ["path", "content"]
            }),
        },
        ToolDef {
            name: "edit".into(),
            description: "Replace an exact string match in a file with new content. Prefer over write_file for targeted changes.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the file"},
                    "old_string": {"type": "string", "description": "Text to find (must be unique)"},
                    "new_string": {"type": "string", "description": "Replacement text"}
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        ToolDef {
            name: "grep".into(),
            description: "Search file contents using a regex pattern.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regex pattern to search"},
                    "root": {"type": "string", "description": "Root directory to search in"},
                    "glob": {"type": "string", "description": "Optional file glob filter (e.g. *.rs)"}
                },
                "required": ["pattern", "root"]
            }),
        },
        ToolDef {
            name: "glob".into(),
            description: "Find files by glob pattern.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern (e.g. **/*.rs)"},
                    "root": {"type": "string", "description": "Root directory to search in"}
                },
                "required": ["pattern", "root"]
            }),
        },
        ToolDef {
            name: "list_directory".into(),
            description: "List entries in a directory.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the directory"}
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "bash_run".into(),
            description: "Run a shell command and return output.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Shell command to run"},
                    "cwd": {"type": "string", "description": "Working directory (optional)"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 30}
                },
                "required": ["command"]
            }),
        },
    ]
}

/// Execute a tool call and return the result as a string.
async fn execute_tool(tc: &ToolCall, workspace_root: &str) -> Result<String, String> {
    match tc.name.as_str() {
        "read_file" => {
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let offset = tc.arguments["offset"].as_i64().unwrap_or(0) as usize;
            let limit = tc.arguments["limit"].as_i64().unwrap_or(2000) as usize;
            let content = tokio::fs::read_to_string(path).await
                .map_err(|e| format!("read_file error: {}", e))?;
            let lines: Vec<&str> = content.lines().skip(offset).take(limit).collect();
            Ok(lines.join("\n"))
        }
        "write_file" => {
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let content = tc.arguments["content"].as_str().ok_or("missing content")?;
            if let Some(parent) = std::path::Path::new(path).parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| format!("write_file mkdir error: {}", e))?;
            }
            tokio::fs::write(path, content).await.map_err(|e| format!("write_file error: {}", e))?;
            Ok(format!("Written {} bytes to {}", content.len(), path))
        }
        "edit" => {
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let old = tc.arguments["old_string"].as_str().ok_or("missing old_string")?;
            let new = tc.arguments["new_string"].as_str().ok_or("missing new_string")?;
            let content = tokio::fs::read_to_string(path).await
                .map_err(|e| format!("edit read error: {}", e))?;
            if !content.contains(old) {
                return Err("edit: old_string not found in file".into());
            }
            let new_content = content.replace(old, new);
            tokio::fs::write(path, &new_content).await
                .map_err(|e| format!("edit write error: {}", e))?;
            Ok(format!("Replaced one occurrence in {}", path))
        }
        "grep" => {
            let pattern = tc.arguments["pattern"].as_str().ok_or("missing pattern")?;
            let root = tc.arguments["root"].as_str().unwrap_or(workspace_root);
            let re = regex::Regex::new(pattern).map_err(|e| format!("grep regex error: {}", e))?;
            let mut results = Vec::new();
            // Simple recursive grep (limited depth to avoid infinite loops)
            fn grep_dir(re: &regex::Regex, dir: &std::path::Path, results: &mut Vec<String>, depth: usize) -> Result<(), String> {
                if depth > 8 { return Ok(()); }
                let entries = std::fs::read_dir(dir).map_err(|e| format!("grep dir error: {}", e))?;
                for entry in entries {
                    let entry = entry.map_err(|e| format!("grep entry error: {}", e))?;
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if name.starts_with('.') || name == "node_modules" || name == "target" { continue; }
                        grep_dir(re, &path, results, depth + 1)?;
                    } else if path.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            for (i, line) in content.lines().enumerate() {
                                if re.is_match(line) {
                                    results.push(format!("{}:{}: {}", path.display(), i + 1, line));
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            grep_dir(&re, std::path::Path::new(root), &mut results, 0)?;
            if results.len() > 100 {
                results.truncate(100);
                results.push("... (truncated at 100 matches)".into());
            }
            Ok(results.join("\n"))
        }
        "glob" => {
            let pattern = tc.arguments["pattern"].as_str().ok_or("missing pattern")?;
            let root = tc.arguments["root"].as_str().unwrap_or(workspace_root);
            let glob_pattern = format!("{}/{}", root.trim_end_matches('/'), pattern);
            let mut entries = Vec::new();
            let glob_entries = glob::glob(&glob_pattern).map_err(|e| format!("glob error: {}", e))?;
            for entry in glob_entries {
                match entry {
                    Ok(p) => { entries.push(p.display().to_string()); }
                    Err(_) => {}
                }
            }
            entries.sort();
            if entries.len() > 200 {
                entries.truncate(200);
                entries.push("... (truncated at 200 results)".into());
            }
            Ok(entries.join("\n"))
        }
        "list_directory" => {
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let mut entries = tokio::fs::read_dir(path).await
                .map_err(|e| format!("list_directory error: {}", e))?;
            let mut lines = Vec::new();
            while let Some(entry) = entries.next_entry().await.map_err(|e| format!("list_directory entry: {}", e))? {
                let name = entry.file_name().to_string_lossy().to_string();
                let kind = if entry.file_type().await.map_or(false, |t| t.is_dir()) { "dir" } else { "file" };
                lines.push(format!("{}  {}", kind, name));
            }
            lines.sort();
            Ok(lines.join("\n"))
        }
        "bash_run" => {
            let command = tc.arguments["command"].as_str().ok_or("missing command")?;
            let cwd = tc.arguments["cwd"].as_str().unwrap_or("");
            let timeout_secs = tc.arguments["timeout"].as_u64().unwrap_or(30);
            let mut cmd = if cfg!(target_os = "windows") {
                let mut c = tokio::process::Command::new("cmd");
                c.arg("/C").arg(command);
                c
            } else {
                let mut c = tokio::process::Command::new("sh");
                c.arg("-c").arg(command);
                c
            };
            if !cwd.is_empty() {
                cmd.current_dir(cwd);
            }
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                cmd.output(),
            )
            .await
            .map_err(|_| format!("bash_run timed out after {}s", timeout_secs))?
            .map_err(|e| format!("bash_run error: {}", e))?;
            let mut result = String::new();
            if !output.stdout.is_empty() {
                result.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                if !result.is_empty() { result.push('\n'); }
                result.push_str(&String::from_utf8_lossy(&output.stderr));
            }
            if !output.status.success() {
                result.push_str(&format!("\n(exit code: {:?})", output.status.code()));
            }
            Ok(result)
        }
        _ => Err(format!("Unknown tool: {}", tc.name)),
    }
}

#[tauri::command]
pub fn ai_list_personas() -> Vec<AgentPersona> {
    BUILTIN_PERSONAS.to_vec()
}

#[derive(Clone, Serialize)]
pub struct AiToolCallEvent {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Serialize)]
pub struct AiToolResultEvent {
    pub id: String,
    pub name: String,
    pub result: String,
}

async fn run_react_loop(
    app: &tauri::AppHandle,
    agent: &AgentConfig,
    messages: &[ChatMessage],
    base_url: &str,
) -> Result<(String, u64, u64), String> {
    let tools = build_tools();
    let tool_defs: Vec<Value> = tools.iter().map(|t| serde_json::json!({
        "type": "function",
        "function": {
            "name": t.name,
            "description": t.description,
            "parameters": t.parameters
        }
    })).collect();

    let mut conversation: Vec<Value> = Vec::new();
    let sp = resolve_system_prompt(agent);
    if !sp.is_empty() {
        conversation.push(serde_json::json!({"role": "system", "content": sp}));
    }
    for m in messages {
        conversation.push(serde_json::json!({"role": m.role, "content": m.content}));
    }

    let mut full_content = String::new();
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let max_steps = 10;

    for _step in 0..max_steps {
        let body = serde_json::json!({
            "model": agent.model,
            "messages": conversation,
            "temperature": agent.temperature.unwrap_or(0.7),
            "max_tokens": 4096,
            "tools": tool_defs,
            "stream": false,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        let mut req_builder = client
            .post(format!("{}/chat/completions", base_url))
            .json(&body);
        if let Some(key) = &agent.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }

        let response = req_builder.send().await.map_err(|e| format!("Request failed: {}", e))?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("AI request failed ({}): {}", status, text));
        }

        let resp_json: Value = response.json().await.map_err(|e| format!("Parse error: {}", e))?;

        if let Some(usage) = resp_json["usage"].as_object() {
            if let Some(v) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) { total_input += v; }
            if let Some(v) = usage.get("completion_tokens").and_then(|v| v.as_u64()) { total_output += v; }
        }

        let choice = &resp_json["choices"][0]["message"];

        // Parse tool calls
        let tool_calls = choice["tool_calls"].as_array()
            .map(|arr| arr.iter().filter_map(|tc| {
                let id = tc["id"].as_str()?.to_string();
                let name = tc["function"]["name"].as_str()?.to_string();
                let args: Value = serde_json::from_str(tc["function"]["arguments"].as_str()?)
                    .unwrap_or(Value::Null);
                Some(ToolCall { id, name, arguments: args })
            }).collect::<Vec<_>>())
            .unwrap_or_default();

        if !tool_calls.is_empty() {
            // Execute tools and add results to conversation
            let workspace_root = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            // Add assistant message with tool calls
            let assistant_msg = serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": choice["tool_calls"]
            });
            conversation.push(assistant_msg);

            // Emit tool calls to frontend
            for tc in &tool_calls {
                let _ = app.emit("ai:tool_call", AiToolCallEvent {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                });
            }

            // Execute and emit results
            for tc in &tool_calls {
                let result = execute_tool(tc, &workspace_root).await.unwrap_or_else(|e| format!("Error: {}", e));
                let _ = app.emit("ai:tool_result", AiToolResultEvent {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    result: result.clone(),
                });
                conversation.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": result
                }));
            }
        } else {
            // Text response - stream full at once
            if let Some(content) = choice["content"].as_str() {
                full_content = content.to_string();
                let _ = app.emit("ai:chunk", AiStreamChunk { delta: content.to_string() });
            }
            break;
        }
    }

    Ok((full_content, total_input, total_output))
}

#[tauri::command]
pub async fn ai_chat_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
    agent_id: String,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let mut agent = state.get_agent(&agent_id).ok_or("Agent not found")?;

    if let Ok(Some(real_key)) = crate::modules::secrets::get_secret(&app, &secrets_state, "codlib-ai", &agent_id) {
        agent.api_key = Some(real_key);
    } else if let Some(ref key) = agent.api_key {
        if key == "********" {
            let _ = app.emit("ai:error", AiStreamError { error: "API key is configured but not found in OS Keychain. Please re-enter the key in Settings > Agents.".to_string() });
            return Err("API key not found in keychain".to_string());
        }
    } else {
        let _ = app.emit("ai:error", AiStreamError { error: format!("No API key configured for '{}'. Add one in Settings > Agents.", agent_id) });
        return Err("API key not configured".to_string());
    }

    let base_url = agent.base_url.clone().unwrap_or_else(|| default_base_url(&agent.provider).to_string());

    if base_url.is_empty() {
        let msg = format!("Base URL is required for provider '{}'. Enter your endpoint URL in Settings > Agents.", agent.provider);
        let _ = app.emit("ai:error", AiStreamError { error: msg.clone() });
        return Err(msg);
    }

    // Use ReAct loop for persona agents, simple streaming for others
    let result = if agent.persona_id.is_some() {
        run_react_loop(&app, &agent, &messages, &base_url).await
    } else {
        stream_openai(&app, &agent, &messages, &base_url).await
    };

    match result {
        Ok((content, input_tokens, output_tokens)) => {
            let (price_in, price_out) = model_price(&agent.model);
            let cost = (input_tokens as f64 * price_in + output_tokens as f64 * price_out) / 1000.0;
            state.record_usage(&agent.id, input_tokens, output_tokens, cost);

            let _ = app.emit("ai:done", AiStreamDone {
                content,
                provider: agent.provider.clone(),
                model: agent.model.clone(),
                input_tokens,
                output_tokens,
                cost,
            });
            Ok(())
        }
        Err(e) => {
            let _ = app.emit("ai:error", AiStreamError { error: e.clone() });
            Err(e)
        }
    }
}

#[tauri::command]
pub fn ai_get_usage(state: tauri::State<'_, AiManager>) -> Vec<AgentUsage> {
    state.get_usage()
}

#[tauri::command]
pub fn ai_reset_usage(state: tauri::State<'_, AiManager>) -> Result<(), String> {
    state.reset_usage();
    Ok(())
}

#[tauri::command]
pub async fn ai_list_agents(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
) -> Result<Vec<AgentConfig>, String> {
    let mut list = state.list_agents();
    for agent in &mut list {
        let has_key = if let Ok(Some(_)) = crate::modules::secrets::get_secret(&app, &secrets_state, "codlib-ai", &agent.id) {
            true
        } else if let Some(ref key) = agent.api_key {
            !key.is_empty()
        } else {
            false
        };

        if has_key {
            agent.api_key = Some("********".to_string());
        } else {
            agent.api_key = None;
        }
    }
    Ok(list)
}

#[tauri::command]
pub async fn ai_update_agent(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
    mut config: AgentConfig,
) -> Result<(), String> {
    if let Some(ref key) = config.api_key {
        if key == "********" {
            // Keep existing key in Keychain, do nothing
        } else if key.is_empty() {
            let _ = crate::modules::secrets::delete_secret(&app, &secrets_state, "codlib-ai", &config.id);
            config.api_key = None;
        } else {
            crate::modules::secrets::set_secret(&app, &secrets_state, "codlib-ai", &config.id, key)?;
            config.api_key = Some("********".to_string());
        }
    } else {
        let _ = crate::modules::secrets::delete_secret(&app, &secrets_state, "codlib-ai", &config.id);
    }
    state.update_agent(config)
}

#[tauri::command]
pub async fn ai_remove_agent(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
    agent_id: String,
) -> Result<(), String> {
    let _ = crate::modules::secrets::delete_secret(&app, &secrets_state, "codlib-ai", &agent_id);
    state.remove_agent(&agent_id)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProviderModel {
    pub id: String,
    pub name: Option<String>,
    pub source: String,
}

const PROVIDER_ENDPOINTS: &[(&str, &str)] = &[
    ("openai", "https://api.openai.com/v1"),
    ("cerebras", "https://api.cerebras.ai/v1"),
    ("mistral", "https://api.mistral.ai/v1"),
    ("alibaba", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
    ("xai", "https://api.x.ai/v1"),
    ("openrouter", "https://openrouter.ai/api/v1"),
];

fn provider_endpoint(provider: &str) -> Option<&'static str> {
    PROVIDER_ENDPOINTS.iter().find(|(p, _)| *p == provider).map(|(_, url)| *url)
}

async fn fetch_models_json(url: &str, api_key: &Option<String>) -> Result<(reqwest::StatusCode, String), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.get(url);
    if let Some(ref key) = api_key {
        req = req.header("Authorization", format!("Bearer {key}"));
    }
    let resp = req.send().await.map_err(|e| format!("HTTP error: {e}"))?;
    let status = resp.status();
    let body_text = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body_text))
}

async fn fetch_openai_models(url: &str, api_key: &Option<String>) -> Result<Vec<ProviderModel>, String> {
    let models_url = format!("{}/models", url.trim_end_matches('/'));
    let (status, body_text) = fetch_models_json(&models_url, api_key).await?;
    let body: Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Parse error: {e}\nResponse body:\n{body_text}"))?;
    if !status.is_success() {
        let err_msg = body["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(format!("API error ({}): {err_msg}", status.as_u16()));
    }
    let arr = body["data"].as_array()
        .ok_or_else(|| format!("No 'data' array in response.\nResponse body:\n{body_text}"))?;
    let list = arr.iter().filter_map(|m| {
        let id = m["id"].as_str()?.to_string();
        Some(ProviderModel { id, name: None, source: "openai".to_string() })
    }).collect();
    Ok(list)
}

async fn fetch_gemini_models(api_key: &Option<String>) -> Result<Vec<ProviderModel>, String> {
    let key = api_key.as_ref().ok_or("API key required for Gemini")?;
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={key}");
    let (status, body_text) = fetch_models_json(&url, &None).await?;
    let body: Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Parse error: {e}\nResponse body:\n{body_text}"))?;
    if !status.is_success() {
        let err_msg = body["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(format!("Gemini API error ({}): {err_msg}", status.as_u16()));
    }
    let arr = body["models"].as_array()
        .ok_or_else(|| format!("No 'models' array in Gemini response.\nResponse body:\n{body_text}"))?;
    let list = arr.iter().filter_map(|m| {
        let name = m["name"].as_str()?;
        let id = name.split('/').last().unwrap_or(name);
        Some(ProviderModel { id: id.to_string(), name: None, source: "gemini".to_string() })
    }).collect();
    Ok(list)
}

async fn fetch_openrouter_models(api_key: &Option<String>) -> Result<Vec<ProviderModel>, String> {
    let key = api_key.as_ref().ok_or("API key required for OpenRouter")?;
    let (status, body_text) = fetch_models_json("https://openrouter.ai/api/v1/models", &Some(key.clone())).await?;
    let body: Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("Parse error: {e}\nResponse body:\n{body_text}"))?;
    if !status.is_success() {
        let err_msg = body["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(format!("OpenRouter error ({}): {err_msg}", status.as_u16()));
    }
    let arr = body["data"].as_array()
        .ok_or_else(|| format!("No 'data' array.\nResponse body:\n{body_text}"))?;
    let list = arr.iter().filter_map(|m| {
        let id = m["id"].as_str()?.to_string();
        let name = m["name"].as_str().map(|s| s.to_string());
        Some(ProviderModel { id, name, source: "openrouter".to_string() })
    }).collect();
    Ok(list)
}

fn vercel_models() -> Vec<ProviderModel> {
    vec![
        ProviderModel { id: "openai/gpt-4o".into(), name: Some("GPT-4o (OpenAI)".into()), source: "vercel".into() },
        ProviderModel { id: "openai/gpt-4o-mini".into(), name: Some("GPT-4o Mini (OpenAI)".into()), source: "vercel".into() },
        ProviderModel { id: "openai/gpt-4-turbo".into(), name: Some("GPT-4 Turbo (OpenAI)".into()), source: "vercel".into() },
        ProviderModel { id: "openai/gpt-3.5-turbo".into(), name: Some("GPT-3.5 Turbo (OpenAI)".into()), source: "vercel".into() },
        ProviderModel { id: "openai/o3-mini".into(), name: Some("o3 Mini (OpenAI)".into()), source: "vercel".into() },
        ProviderModel { id: "anthropic/claude-sonnet-4-20250514".into(), name: Some("Claude Sonnet 4 (Anthropic)".into()), source: "vercel".into() },
        ProviderModel { id: "anthropic/claude-3-5-sonnet-latest".into(), name: Some("Claude 3.5 Sonnet (Anthropic)".into()), source: "vercel".into() },
        ProviderModel { id: "anthropic/claude-3-5-haiku-latest".into(), name: Some("Claude 3.5 Haiku (Anthropic)".into()), source: "vercel".into() },
        ProviderModel { id: "anthropic/claude-opus-4-20250514".into(), name: Some("Claude Opus 4 (Anthropic)".into()), source: "vercel".into() },
        ProviderModel { id: "google/gemini-2.0-flash".into(), name: Some("Gemini 2.0 Flash (Google)".into()), source: "vercel".into() },
        ProviderModel { id: "google/gemini-2.0-flash-lite".into(), name: Some("Gemini 2.0 Flash Lite (Google)".into()), source: "vercel".into() },
        ProviderModel { id: "google/gemini-2.5-pro".into(), name: Some("Gemini 2.5 Pro (Google)".into()), source: "vercel".into() },
        ProviderModel { id: "google/gemini-1.5-pro".into(), name: Some("Gemini 1.5 Pro (Google)".into()), source: "vercel".into() },
        ProviderModel { id: "google/gemini-1.5-flash".into(), name: Some("Gemini 1.5 Flash (Google)".into()), source: "vercel".into() },
        ProviderModel { id: "deepseek/deepseek-chat".into(), name: Some("DeepSeek Chat (DeepSeek)".into()), source: "vercel".into() },
        ProviderModel { id: "deepseek/deepseek-reasoner".into(), name: Some("DeepSeek Reasoner (DeepSeek)".into()), source: "vercel".into() },
        ProviderModel { id: "cerebras/llama3.1-8b".into(), name: Some("Llama 3.1 8B (Cerebras)".into()), source: "vercel".into() },
        ProviderModel { id: "cerebras/llama3.1-70b".into(), name: Some("Llama 3.1 70B (Cerebras)".into()), source: "vercel".into() },
        ProviderModel { id: "mistral/mistral-large-latest".into(), name: Some("Mistral Large (Mistral)".into()), source: "vercel".into() },
        ProviderModel { id: "xai/grok-2".into(), name: Some("Grok 2 (xAI)".into()), source: "vercel".into() },
        ProviderModel { id: "perplexity/sonar-pro".into(), name: Some("Sonar Pro (Perplexity)".into()), source: "vercel".into() },
    ]
}

#[tauri::command]
pub async fn ai_list_models(
    api_key: Option<String>,
    base_url: Option<String>,
    provider: String,
) -> Result<Vec<ProviderModel>, String> {
    match provider.as_str() {
        "vercel" => Ok(vercel_models()),
        "gemini" => fetch_gemini_models(&api_key).await,
        "openrouter" => fetch_openrouter_models(&api_key).await,
        _ => {
            if let Some(endpoint) = provider_endpoint(&provider) {
                fetch_openai_models(endpoint, &api_key).await
            } else {
                let base = base_url.unwrap_or_else(|| default_base_url(&provider).to_string());
                if base.is_empty() {
                    return Err(format!("Unknown provider '{}'. No base URL configured.", provider));
                }
                fetch_openai_models(&base, &api_key).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    fn tc(name: &str, args: Value) -> ToolCall {
        ToolCall { id: "t1".into(), name: name.into(), arguments: args }
    }

    // ── Tool execution tests ──

    #[tokio::test]
    async fn test_read_file() {
        let r = execute_tool(&tc("read_file", json!({"path": "Cargo.toml"})), ".").await.unwrap();
        assert!(r.contains("[package]"));
    }

    #[tokio::test]
    async fn test_read_file_offset() {
        let r = execute_tool(&tc("read_file", json!({"path": "Cargo.toml", "offset": 0, "limit": 3})), ".").await.unwrap();
        assert!(r.lines().count() <= 3);
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let r = execute_tool(&tc("read_file", json!({"path": "_no_such_file_69"})), ".").await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn test_write_edit_cycle() {
        let p = "_test_we.txt";
        let _ = std::fs::remove_file(p);
        execute_tool(&tc("write_file", json!({"path": p, "content": "foo\nbar\n"})), ".").await.unwrap();
        assert!(Path::new(p).exists());
        let r = execute_tool(&tc("edit", json!({"path": p, "old_string": "foo", "new_string": "baz"})), ".").await.unwrap();
        assert!(r.contains("Replaced"));
        let c = std::fs::read_to_string(p).unwrap();
        assert!(c.contains("baz"));
        assert!(!c.contains("foo"));
        let _ = std::fs::remove_file(p);
    }

    #[tokio::test]
    async fn test_write_creates_dirs() {
        let p = "_test_dir_nested/f/a.txt";
        let _ = std::fs::remove_dir_all("_test_dir_nested");
        execute_tool(&tc("write_file", json!({"path": p, "content": "x"})), ".").await.unwrap();
        assert!(Path::new(p).exists());
        let _ = std::fs::remove_dir_all("_test_dir_nested");
    }

    #[tokio::test]
    async fn test_edit_not_found_errs() {
        let p = "_test_ef.txt";
        std::fs::write(p, "hello").unwrap();
        let r = execute_tool(&tc("edit", json!({"path": p, "old_string": "nope", "new_string": "x"})), ".").await;
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("old_string not found"));
        let _ = std::fs::remove_file(p);
    }

    #[tokio::test]
    async fn test_grep() {
        let r = execute_tool(&tc("grep", json!({"pattern": "tauri", "root": "."})), ".").await.unwrap();
        assert!(r.contains("tauri"));
    }

    #[tokio::test]
    async fn test_glob() {
        let r = execute_tool(&tc("glob", json!({"pattern": "Cargo.toml", "root": "."})), ".").await.unwrap();
        assert!(r.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_list_directory() {
        let r = execute_tool(&tc("list_directory", json!({"path": "."})), ".").await.unwrap();
        assert!(r.contains("src"));
    }

    #[tokio::test]
    async fn test_bash_run() {
        let r = execute_tool(&tc("bash_run", json!({"command": "echo hello_test_42", "timeout": 10})), ".").await.unwrap();
        assert!(r.contains("hello_test_42"));
    }

    #[tokio::test]
    async fn test_bash_run_timeout() {
        let r = execute_tool(&tc("bash_run", json!({"command": "ping -n 10 127.0.0.1", "timeout": 2})), ".").await;
        assert!(r.is_err() || r.unwrap().contains("timed out"));
    }

    #[tokio::test]
    async fn test_bash_run_failure() {
        let r = execute_tool(&tc("bash_run", json!({"command": "exit 1", "timeout": 5})), ".").await.unwrap();
        assert!(r.contains("exit code"));
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let r = execute_tool(&tc("foobar", json!({})), ".").await;
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_resolve_system_prompt_persona() {
        let agent = AgentConfig {
            id: "t".into(), name: "t".into(), provider: "test".into(), model: "m".into(),
            base_url: None, api_key: None, capabilities: vec![],
            temperature: None, system_prompt: Some("custom".into()),
            persona_id: Some("coder".into()), built_in: true,
        };
        let sp = resolve_system_prompt(&agent);
        assert!(sp.contains("expert software engineer"));
    }

    #[tokio::test]
    async fn test_resolve_system_prompt_custom() {
        let agent = AgentConfig {
            id: "t".into(), name: "t".into(), provider: "test".into(), model: "m".into(),
            base_url: None, api_key: None, capabilities: vec![],
            temperature: None, system_prompt: Some("custom prompt".into()),
            persona_id: None, built_in: false,
        };
        let sp = resolve_system_prompt(&agent);
        assert_eq!(sp, "custom prompt");
    }

    #[tokio::test]
    async fn test_resolve_system_prompt_default() {
        let agent = AgentConfig {
            id: "t".into(), name: "t".into(), provider: "test".into(), model: "m".into(),
            base_url: None, api_key: None, capabilities: vec![],
            temperature: None, system_prompt: None,
            persona_id: None, built_in: false,
        };
        let sp = resolve_system_prompt(&agent);
        assert_eq!(sp, "");
    }

    #[tokio::test]
    async fn test_model_price_known() {
        let (inp, out) = model_price("gpt-4o-mini");
        assert!((inp - 0.00015).abs() < 1e-10);
        assert!((out - 0.0006).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_model_price_fallback() {
        let (inp, out) = model_price("unknown-model-123");
        assert!((inp - 0.001).abs() < 1e-10);
        assert!((out - 0.002).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_build_tools() {
        let tools = build_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"edit"));
        assert!(names.contains(&"grep"));
        assert!(names.contains(&"glob"));
        assert!(names.contains(&"list_directory"));
        assert!(names.contains(&"bash_run"));
    }

    // ── API connectivity tests (set env vars, run with `cargo test -- --ignored`) ──

    async fn simple_chat(api_key: &str, base_url: &str, model: &str, msg: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": msg}],
            "temperature": 0.5,
            "max_tokens": 80,
        });
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .post(format!("{}/chat/completions", base_url.trim_end_matches('/')))
            .json(&body)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| format!("HTTP: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("Body: {e}"))?;
        if !status.is_success() {
            return Err(format!("{}: {}", status, text));
        }
        Ok(text)
    }

    #[ignore]
    #[tokio::test]
    async fn test_api_cerebras() {
        let key = std::env::var("CEREBRAS_API_KEY").expect("Set CEREBRAS_API_KEY");
        let r = simple_chat(&key, "https://api.cerebras.ai/v1", "gpt-oss-120b", "Reply just: OK").await.unwrap();
        assert!(r.contains("OK") || r.contains("ok") || r.contains("Ok"));
    }

    #[ignore]
    #[tokio::test]
    async fn test_api_mistral() {
        let key = std::env::var("MISTRAL_API_KEY").expect("Set MISTRAL_API_KEY");
        let r = simple_chat(&key, "https://api.mistral.ai/v1", "mistral-large-latest", "Reply just: OK").await.unwrap();
        assert!(r.contains("OK") || r.contains("ok") || r.contains("Ok"));
    }

    #[ignore]
    #[tokio::test]
    async fn test_api_vercel() {
        let key = std::env::var("VERCEL_API_KEY").expect("Set VERCEL_API_KEY");
        let r = simple_chat(&key, "https://ai-gateway.vercel.sh/v1", "openai/gpt-4o-mini", "Reply just: OK").await.unwrap();
        assert!(r.contains("OK") || r.contains("ok") || r.contains("Ok"));
    }

    #[ignore]
    #[tokio::test]
    async fn test_api_gemini() {
        let key = std::env::var("GEMINI_API_KEY").expect("Set GEMINI_API_KEY");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build().unwrap();
        let body = serde_json::json!({
            "contents": [{"parts": [{"text": "Reply just: OK"}]}],
            "generationConfig": {"maxOutputTokens": 20}
        });
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={key}");
        let resp = client.post(&url).json(&body).send().await.unwrap();
        assert!(resp.status().is_success(), "Gemini: {}", resp.status());
    }

    // ── ReAct loop test with tool calling ──

    #[ignore]
    #[tokio::test]
    async fn test_react_loop_coder_read_file() {
        let key = std::env::var("CEREBRAS_API_KEY").expect("Set CEREBRAS_API_KEY");
        let agent = AgentConfig {
            id: "coder-test".into(), name: "Coder Test".into(),
            provider: "cerebras".into(), model: "gpt-oss-120b".into(),
            base_url: Some("https://api.cerebras.ai/v1".into()),
            api_key: Some(key),
            capabilities: vec!["tools".into()],
            temperature: Some(0.5),
            system_prompt: None,
            persona_id: Some("coder".into()),
            built_in: true,
        };
        let messages = vec![
            ChatMessage { role: "user".into(), content: "Read Cargo.toml and tell me the package name.".into() },
        ];
        let (content, _, _) = run_react_loop_inner(&agent, &messages, "https://api.cerebras.ai/v1").await.unwrap();
        assert!(content.contains("codlib") || content.contains("package") || content.contains("name"));
    }

    // Test helper that calls run_react_loop without the app handle
    async fn run_react_loop_inner(
        agent: &AgentConfig,
        messages: &[ChatMessage],
        base_url: &str,
    ) -> Result<(String, u64, u64), String> {
        let tools = build_tools();
        let tool_defs: Vec<Value> = tools.iter().map(|t| serde_json::json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters
            }
        })).collect();

        let mut conversation: Vec<Value> = Vec::new();
        let sp = resolve_system_prompt(agent);
        if !sp.is_empty() {
            conversation.push(serde_json::json!({"role": "system", "content": sp}));
        }
        for m in messages {
            conversation.push(serde_json::json!({"role": m.role, "content": m.content}));
        }

        let mut full_content = String::new();
        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let max_steps = 10;

        for _step in 0..max_steps {
            let body = serde_json::json!({
                "model": agent.model,
                "messages": conversation,
                "temperature": agent.temperature.unwrap_or(0.7),
                "max_tokens": 4096,
                "tools": tool_defs,
                "stream": false,
            });

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .map_err(|e| e.to_string())?;
            let mut req_builder = client
                .post(format!("{}/chat/completions", base_url))
                .json(&body);
            if let Some(key) = &agent.api_key {
                req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
            }

            let response = req_builder.send().await.map_err(|e| format!("Request failed: {}", e))?;
            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                return Err(format!("AI request failed ({}): {}", status, text));
            }

            let resp_json: Value = response.json().await.map_err(|e| format!("Parse error: {}", e))?;

            if let Some(usage) = resp_json["usage"].as_object() {
                if let Some(v) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) { total_input += v; }
                if let Some(v) = usage.get("completion_tokens").and_then(|v| v.as_u64()) { total_output += v; }
            }

            let choice = &resp_json["choices"][0]["message"];

            let tool_calls = choice["tool_calls"].as_array()
                .map(|arr| arr.iter().filter_map(|tc| {
                    let id = tc["id"].as_str()?.to_string();
                    let name = tc["function"]["name"].as_str()?.to_string();
                    let args: Value = serde_json::from_str(tc["function"]["arguments"].as_str()?)
                        .unwrap_or(Value::Null);
                    Some(ToolCall { id, name, arguments: args })
                }).collect::<Vec<_>>())
                .unwrap_or_default();

            if !tool_calls.is_empty() {
                let workspace_root = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let assistant_msg = serde_json::json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": choice["tool_calls"]
                });
                conversation.push(assistant_msg);

                for tc in &tool_calls {
                    let result = execute_tool(tc, &workspace_root).await.unwrap_or_else(|e| format!("Error: {}", e));
                    conversation.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": result
                    }));
                }
            } else {
                if let Some(content) = choice["content"].as_str() {
                    full_content = content.to_string();
                }
                break;
            }
        }

        Ok((full_content, total_input, total_output))
    }
}
