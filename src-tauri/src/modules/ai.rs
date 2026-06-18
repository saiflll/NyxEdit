use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tauri::{Emitter, Manager};
use chrono::Utc;



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
    /// Cached list of models fetched from the provider endpoint (persisted, no need to re-fetch)
    #[serde(default)]
    pub cached_models: Vec<String>,
    /// Timestamp of last successful model sync (ISO 8601)
    #[serde(default)]
    pub models_synced_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_content: Option<String>,
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

pub fn model_price(model: &str) -> (f64, f64) {
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

#[derive(Clone)]
pub struct AiManager {
    pub agents: Arc<Mutex<HashMap<String, AgentConfig>>>,
    pub usage: Arc<Mutex<HashMap<String, AgentUsage>>>,
    /// Current workspace root for agent log files
    pub workspace_root: Arc<Mutex<String>>,
    /// Whether agents have been loaded from disk yet
    pub loaded: Arc<Mutex<bool>>,
}

// â”€â”€ Persistence helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn get_agents_file_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("agents.json"))
}

fn load_agents_from_disk(app: &tauri::AppHandle) -> HashMap<String, AgentConfig> {
    let path = match get_agents_file_path(app) {
        Some(p) => p,
        None => return HashMap::new(),
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<HashMap<String, AgentConfig>>(&text).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_agents_to_disk(app: &tauri::AppHandle, agents: &HashMap<String, AgentConfig>) {
    let path = match get_agents_file_path(app) {
        Some(p) => p,
        None => return,
    };
    // Create parent dir if needed
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Mask API keys before writing â€” security best practice
    let safe: HashMap<String, AgentConfig> = agents.iter().map(|(k, v)| {
        let mut a = v.clone();
        if a.api_key.as_deref().map(|k| !k.is_empty() && k != "********").unwrap_or(false) {
            a.api_key = Some("********".to_string());
        }
        (k.clone(), a)
    }).collect();
    if let Ok(json) = serde_json::to_string_pretty(&safe) {
        let _ = std::fs::write(&path, json);
    }
}

impl AiManager {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
            usage: Arc::new(Mutex::new(HashMap::new())),
            workspace_root: Arc::new(Mutex::new(String::new())),
            loaded: Arc::new(Mutex::new(false)),
        }
    }

    /// Ensure agents are loaded from disk once. Call at the start of every public command.
    pub fn ensure_loaded(&self, app: &tauri::AppHandle) {
        let mut loaded = self.loaded.lock().unwrap();
        if !*loaded {
            *loaded = true;
            let from_disk = load_agents_from_disk(app);
            if !from_disk.is_empty() {
                let mut agents = self.agents.lock().unwrap();
                for (k, v) in from_disk {
                    agents.entry(k).or_insert(v);
                }
            }
            // Warm cache: pre-load session list and scan cache on first call
            self.warm_cache(app);
            
            // Stage 5: Auto-load Knowledge Graph (lazy load on first use)
            if let Some(graph_state) = app.try_state::<crate::modules::graph::SymbolGraph>() {
                let _ = graph_state.ensure_loaded();
            }
            
            // Stage 6: Auto-detect Project Intel
            if let Some(intel_state) = app.try_state::<crate::modules::project_intel::ProjectIntelState>() {
                if let Some(root) = self.workspace_root.lock().unwrap().clone() {
                    let ctx = intel_state.detect(&root);
                    let _ = app.emit("nyx:project_detected", serde_json::json!({
                        "framework": ctx.framework_label(),
                        "language": ctx.language,
                        "has_tests": ctx.has_tests,
                        "has_ci": ctx.has_ci,
                        "has_docker": ctx.has_docker,
                        "file_count": ctx.file_count,
                        "src_dirs": ctx.src_dirs,
                    }));
                }
            }
        }
    }

    /// Warm up caches on first load for faster subsequent operations.
    fn warm_cache(&self, app: &tauri::AppHandle) {
        // Warm ripgrep scan cache if workspace root is set
        let root = self.workspace_root.lock().unwrap().clone();
        if !root.is_empty() && std::path::Path::new(&root).exists() {
            let _ = super::ripgrep::search_text("fn", std::path::Path::new(&root), Some(5));
        }
        // Report healthy to self-heal engine
        if let Some(heal) = app.try_state::<super::self_heal::SelfHealEngine>() {
            heal.report_healthy("ai");
        }
        log::info!("Cache warmed for workspace: {}", root);
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

    pub fn get_agent_for_provider(&self, provider: &str) -> Option<AgentConfig> {
        let agents = self.agents.lock().unwrap();
        agents.values().find(|a| a.provider == provider).cloned()
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

    /// Set the current workspace root (used to determine .nyx log dir)
    pub fn set_workspace_root(&self, root: &str) {
        let mut wr = self.workspace_root.lock().unwrap();
        *wr = root.to_string();
    }

    /// Get the current workspace root
    pub fn get_workspace_root(&self) -> String {
        self.workspace_root.lock().unwrap().clone()
    }

    /// Write a line to the agent log file at .nyx/logs/agent-{id}.log
    pub fn write_agent_log(&self, agent_id: &str, agent_name: &str, line: &str) {
        let workspace_root = self.workspace_root.lock().unwrap().clone();
        if workspace_root.is_empty() { return; }
        let sep = if workspace_root.contains('\\') { "\\" } else { "/" };
        let log_dir = format!("{}{}.nyx{}logs", workspace_root, sep, sep);
        let log_path = format!("{}{}{}.log", log_dir, sep, agent_id);
        // Ensure log dir exists (best-effort)
        let _ = std::fs::create_dir_all(&log_dir);
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let entry = format!("[{}][{}] {}\n", timestamp, agent_name, line);
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = f.write_all(entry.as_bytes());
        }
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

pub fn default_base_url(provider: &str) -> &'static str {
    match provider {
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        "deepseek" => "https://api.deepseek.com/v1",
        "mistral" => "https://api.mistral.ai/v1",
        "cerebras" => "https://api.cerebras.ai/v1",
        "groq" => "https://api.groq.com/openai/v1",
        "together" => "https://api.together.xyz/v1",
        "vercel" => "https://ai-gateway.vercel.sh/v1",
        "ollama" => "http://localhost:11434/v1",
        "lmstudio" => "http://localhost:1234/v1",
        "gemini" => "https://generativelanguage.googleapis.com/v1beta/openai",
        _ => "",
    }
}

fn parse_multimodal_content(text: &str) -> serde_json::Value {
    if !text.contains("data:image/") {
        return serde_json::json!(text);
    }

    let mut parts = Vec::new();
    let mut last_pos = 0;
    
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut pos = 0;
    
    while pos < len {
        if pos + 11 <= len && chars[pos..pos+11].iter().collect::<String>() == "data:image/" {
            if pos > last_pos {
                let mut txt_part: String = chars[last_pos..pos].iter().collect();
                if txt_part.ends_with('(') {
                    if let Some(bracket_start) = txt_part.rfind("![") {
                        txt_part.truncate(bracket_start);
                    }
                }
                let clean_txt = txt_part.trim();
                if !clean_txt.is_empty() {
                    parts.push(serde_json::json!({
                        "type": "text",
                        "text": clean_txt
                    }));
                }
            }
            
            let mut end_pos = pos;
            while end_pos < len {
                let c = chars[end_pos];
                if c == ')' || c.is_whitespace() || c == '"' || c == '\'' {
                    break;
                }
                end_pos += 1;
            }
            
            let data_url: String = chars[pos..end_pos].iter().collect();
            let clean_url = data_url.replace(|c: char| c.is_whitespace(), "");
            
            parts.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": clean_url
                }
            }));
            
            if end_pos < len && chars[end_pos] == ')' {
                last_pos = end_pos + 1;
            } else {
                last_pos = end_pos;
            }
            pos = last_pos;
        } else {
            pos += 1;
        }
    }
    
    if last_pos < len {
        let txt_part: String = chars[last_pos..len].iter().collect();
        let clean_txt = txt_part.trim();
        if !clean_txt.is_empty() {
            parts.push(serde_json::json!({
                "type": "text",
                "text": clean_txt
            }));
        }
    }
    
    if parts.is_empty() {
        serde_json::json!(text)
    } else {
        serde_json::json!(parts)
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
            "content": parse_multimodal_content(&m.content)
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

pub async fn stream_openai(
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
        chat_messages.push(serde_json::json!({"role": m.role, "content": parse_multimodal_content(&m.content)}));
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

    let status = response.status();

    if !status.is_success() {
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

// â”€â”€ Tool types for ReAct loop â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
        ToolDef {
            name: "claude_run".into(),
            description: "Run Claude CLI with a prompt (heavy coding tasks, refactoring, complex analysis).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Task description for Claude"},
                    "cwd": {"type": "string", "description": "Working directory (optional)"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 120}
                },
                "required": ["prompt"]
            }),
        },
        ToolDef {
            name: "gemini_run".into(),
            description: "Run Gemini CLI with a prompt (large context, multimodal, code analysis).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Task description for Gemini"},
                    "cwd": {"type": "string", "description": "Working directory (optional)"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 120}
                },
                "required": ["prompt"]
            }),
        },
        ToolDef {
            name: "opencode_run".into(),
            description: "Run OpenCode CLI with a prompt (agentic coding, file operations).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Task description for OpenCode"},
                    "cwd": {"type": "string", "description": "Working directory (optional)"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 120}
                },
                "required": ["prompt"]
            }),
        },
        ToolDef {
            name: "aider_run".into(),
            description: "Run Aider CLI with a prompt (git-aware pair programming, diffs).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Task description for Aider"},
                    "cwd": {"type": "string", "description": "Working directory (optional)"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 120}
                },
                "required": ["prompt"]
            }),
        },
        ToolDef {
            name: "codex_run".into(),
            description: "Run Codex CLI with a prompt (OpenAI agent, code generation).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Task description for Codex"},
                    "cwd": {"type": "string", "description": "Working directory (optional)"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 120}
                },
                "required": ["prompt"]
            }),
        },
        ToolDef {
            name: "agy_run".into(),
            description: "Run Agy CLI with a prompt (lightweight agent).".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Task description for Agy"},
                    "cwd": {"type": "string", "description": "Working directory (optional)"},
                    "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 120}
                },
                "required": ["prompt"]
            }),
        },
    ]
}

#[derive(Clone, Serialize)]
pub struct BashPermissionRequest {
    pub id: String,
    pub command: String,
    pub cwd: String,
}

static PENDING_BASH_PERMISSIONS: OnceLock<Mutex<HashMap<String, oneshot::Sender<Result<String, String>>>>> = OnceLock::new();

fn get_pending_bash_permissions() -> &'static Mutex<HashMap<String, oneshot::Sender<Result<String, String>>>> {
    PENDING_BASH_PERMISSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
pub fn ai_respond_bash_permission(
    id: String,
    approved: bool,
    modified_command: Option<String>,
) -> Result<(), String> {
    let mut pending = get_pending_bash_permissions().lock().unwrap();
    if let Some(tx) = pending.remove(&id) {
        if approved {
            let cmd = modified_command.unwrap_or_default();
            let _ = tx.send(Ok(cmd));
        } else {
            let _ = tx.send(Err("Permission denied by user".to_string()));
        }
    }
    Ok(())
}

#[derive(Clone, Serialize)]
pub struct FileWritePermissionRequest {
    pub id: String,
    pub path: String,
    pub is_edit: bool,
    pub diff: Vec<serde_json::Value>,
}

static PENDING_FILE_PERMISSIONS: OnceLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> = OnceLock::new();

fn get_pending_file_permissions() -> &'static Mutex<HashMap<String, oneshot::Sender<bool>>> {
    PENDING_FILE_PERMISSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
pub fn ai_respond_file_permission(
    id: String,
    approved: bool,
) -> Result<(), String> {
    let mut pending = get_pending_file_permissions().lock().unwrap();
    if let Some(tx) = pending.remove(&id) {
        let _ = tx.send(approved);
    }
    Ok(())
}

fn compute_diff_lines(old_content: &str, new_content: &str) -> Vec<serde_json::Value> {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut diff_lines = Vec::new();
    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            ChangeTag::Delete => "deleted",
            ChangeTag::Insert => "added",
            ChangeTag::Equal => "unchanged",
        };
        diff_lines.push(serde_json::json!({
            "type": tag,
            "text": change.value(),
            "old_index": change.old_index(),
            "new_index": change.new_index(),
        }));
    }
    diff_lines
}

#[derive(Debug, Deserialize)]
struct StyleCodingConfig {
    #[serde(rename = "globalInstructions")]
    global_instructions: Option<String>,
    #[serde(rename = "skillRead")]
    skill_read: Option<bool>,
    #[serde(rename = "skillWrite")]
    skill_write: Option<bool>,
    #[serde(rename = "skillTerminal")]
    skill_terminal: Option<bool>,
}

fn load_style_coding_config(workspace_root: &str) -> Option<StyleCodingConfig> {
    let mut paths = Vec::new();
    if !workspace_root.is_empty() {
        paths.push(std::path::Path::new(workspace_root).join(".nyx").join("style_coding.json"));
        paths.push(std::path::Path::new(workspace_root).join("contlib").join(".nyx").join("style_coding.json"));
    }
    if let Ok(curr) = std::env::current_dir() {
        paths.push(curr.join(".nyx").join("style_coding.json"));
        paths.push(curr.join("contlib").join(".nyx").join("style_coding.json"));
    }

    for path in paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<StyleCodingConfig>(&content) {
                    return Some(config);
                }
            }
        }
    }
    None
}

fn resolve_path(workspace_root: &str, path: &str) -> String {
    if workspace_root.is_empty() {
        return path.to_string();
    }
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        path.to_string()
    } else {
        std::path::Path::new(workspace_root)
            .join(path)
            .to_string_lossy()
            .to_string()
    }
}

/// Execute a tool call and return the result as a string.
async fn execute_tool(app: Option<&tauri::AppHandle>, tc: &ToolCall, workspace_root: &str, is_private: bool) -> Result<String, String> {
    let config = load_style_coding_config(workspace_root);
    let skill_read = config.as_ref().and_then(|c| c.skill_read).unwrap_or(true);
    let skill_write = config.as_ref().and_then(|c| c.skill_write).unwrap_or(false);
    let skill_terminal = config.as_ref().and_then(|c| c.skill_terminal).unwrap_or(false);

    match tc.name.as_str() {
        "read_file" => {
            if !skill_read {
                return Err("Permission denied: Allow Reading Files is disabled in settings".to_string());
            }
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let resolved_path = resolve_path(workspace_root, path);
            let offset = tc.arguments["offset"].as_i64().unwrap_or(0) as usize;
            let limit = tc.arguments["limit"].as_i64().unwrap_or(2000) as usize;
            let content = tokio::fs::read_to_string(&resolved_path).await
                .map_err(|e| format!("read_file error: {}", e))?;
            let lines: Vec<&str> = content.lines().skip(offset).take(limit).collect();
            Ok(lines.join("\n"))
        }
        "write_file" => {
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let resolved_path = resolve_path(workspace_root, path);
            let content = tc.arguments["content"].as_str().ok_or("missing content")?;
            let old_content = tokio::fs::read_to_string(&resolved_path).await.unwrap_or_default();
            
            if old_content == content {
                return Ok(format!("File {} already contains the requested content, no changes needed.", path));
            }

            let bypass = skill_write || app.is_none();
            let success = if bypass {
                if let Some(parent) = std::path::Path::new(&resolved_path).parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| format!("write_file mkdir error: {}", e))?;
                }
                tokio::fs::write(&resolved_path, content).await.map_err(|e| format!("write_file error: {}", e))?;
                true
            } else if let Some(app) = app {
                let diff = compute_diff_lines(&old_content, content);
                let (tx, rx) = oneshot::channel();
                {
                    let mut pending = get_pending_file_permissions().lock().unwrap();
                    pending.insert(tc.id.clone(), tx);
                }

                let _ = app.emit("ai:request_file_permission", FileWritePermissionRequest {
                    id: tc.id.clone(),
                    path: path.to_string(),
                    is_edit: false,
                    diff,
                });

                match rx.await {
                    Ok(true) => {
                        if let Some(parent) = std::path::Path::new(&resolved_path).parent() {
                            tokio::fs::create_dir_all(parent).await.map_err(|e| format!("write_file mkdir error: {}", e))?;
                        }
                        tokio::fs::write(&resolved_path, content).await.map_err(|e| format!("write_file error: {}", e))?;
                        true
                    }
                    Ok(false) => return Err("File write permission denied by user".to_string()),
                    Err(_) => return Err("Permission request channel cancelled".to_string()),
                }
            } else {
                return Err("No app handle".to_string());
            };

            if success {
                if let Some(app) = app {
                    let _ = app.emit("ai:file_changed", AiFileChangedEvent {
                        id: tc.id.clone(),
                        path: resolved_path.clone(),
                        old_content: old_content.clone(),
                        new_content: content.to_string(),
                    });
                }
                Ok(format!("Successfully wrote file: {}", path))
            } else {
                Err("Failed to write file".to_string())
            }
        }
        "edit" => {
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let resolved_path = resolve_path(workspace_root, path);
            let old = tc.arguments["old_string"].as_str().ok_or("missing old_string")?;
            let new = tc.arguments["new_string"].as_str().ok_or("missing new_string")?;
            let old_content = tokio::fs::read_to_string(&resolved_path).await
                .map_err(|e| format!("edit read error: {}", e))?;
            if !old_content.contains(old) {
                return Err("edit: old_string not found in file".into());
            }
            let new_content = old_content.replace(old, new);

            if old_content == new_content {
                return Ok(format!("No changes to make in {}", path));
            }

            let bypass = skill_write || app.is_none();
            let success = if bypass {
                tokio::fs::write(&resolved_path, &new_content).await
                    .map_err(|e| format!("edit write error: {}", e))?;
                true
            } else if let Some(app) = app {
                let diff = compute_diff_lines(&old_content, &new_content);
                let (tx, rx) = oneshot::channel();
                {
                    let mut pending = get_pending_file_permissions().lock().unwrap();
                    pending.insert(tc.id.clone(), tx);
                }

                let _ = app.emit("ai:request_file_permission", FileWritePermissionRequest {
                    id: tc.id.clone(),
                    path: path.to_string(),
                    is_edit: true,
                    diff,
                });

                match rx.await {
                    Ok(true) => {
                        tokio::fs::write(&resolved_path, &new_content).await
                            .map_err(|e| format!("edit write error: {}", e))?;
                        true
                    }
                    Ok(false) => return Err("Edit permission denied by user".to_string()),
                    Err(_) => return Err("Permission request channel cancelled".to_string()),
                }
            } else {
                return Err("No app handle".to_string());
            };

            if success {
                if let Some(app) = app {
                    let _ = app.emit("ai:file_changed", AiFileChangedEvent {
                        id: tc.id.clone(),
                        path: resolved_path.clone(),
                        old_content: old_content.clone(),
                        new_content: new_content.clone(),
                    });
                }
                Ok(format!("Successfully applied edit to {}", path))
            } else {
                Err("Failed to apply edit".to_string())
            }
        }
        "grep" => {
            if !skill_read {
                return Err("Permission denied: Allow Reading Files is disabled in settings".to_string());
            }
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
            if !skill_read {
                return Err("Permission denied: Allow Reading Files is disabled in settings".to_string());
            }
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
            if !skill_read {
                return Err("Permission denied: Allow Reading Files is disabled in settings".to_string());
            }
            let path = tc.arguments["path"].as_str().ok_or("missing path")?;
            let resolved_path = resolve_path(workspace_root, path);
            let mut entries = tokio::fs::read_dir(&resolved_path).await
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
            if is_private {
                return Err("Permission denied: active terminal session is private (AI restricted)".to_string());
            }
            let command = tc.arguments["command"].as_str().ok_or("missing command")?;
            let cwd = tc.arguments["cwd"].as_str().unwrap_or("");
            let timeout_secs = tc.arguments["timeout"].as_u64().unwrap_or(30);

            // Use workspace_root if cwd is empty
            let cwd_to_use = if cwd.is_empty() { workspace_root } else { cwd };

            // Ask for permission if AppHandle is present and terminal execution is not bypassed
            let bypass = skill_terminal || app.is_none();
            let command_to_run = if bypass {
                command.to_string()
            } else if let Some(app) = app {
                let (tx, rx) = oneshot::channel();
                {
                    let mut pending = get_pending_bash_permissions().lock().unwrap();
                    pending.insert(tc.id.clone(), tx);
                }

                let _ = app.emit("ai:request_tool_permission", BashPermissionRequest {
                    id: tc.id.clone(),
                    command: command.to_string(),
                    cwd: cwd_to_use.to_string(),
                });

                // Wait for the oneshot receiver
                match rx.await {
                    Ok(Ok(cmd_override)) => {
                        if cmd_override.is_empty() {
                            command.to_string()
                        } else {
                            cmd_override
                        }
                    }
                    Ok(Err(e)) => return Err(e),
                    Err(_) => return Err("Permission request channel cancelled".to_string()),
                }
            } else {
                command.to_string()
            };

            let mut cmd = if cfg!(target_os = "windows") {
                let mut c = tokio::process::Command::new("cmd");
                c.arg("/C").arg(&command_to_run);
                c
            } else {
                let mut c = tokio::process::Command::new("sh");
                c.arg("-c").arg(&command_to_run);
                c
            };
            if !cwd_to_use.is_empty() {
                cmd.current_dir(cwd_to_use);
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
        "claude_run" | "gemini_run" | "opencode_run" | "aider_run" | "codex_run" | "agy_run" => {
            execute_cli_tool(tc, workspace_root).await
        }
        _ => Err(format!("Unknown tool: {}", tc.name)),
    }
}

async fn execute_cli_tool(tc: &ToolCall, workspace_root: &str) -> Result<String, String> {
    let cli_name = match tc.name.as_str() {
        "claude_run" => "claude",
        "gemini_run" => "gemini",
        "opencode_run" => "opencode",
        "aider_run" => "aider",
        "codex_run" => "codex",
        "agy_run" => "agy",
        _ => return Err(format!("Unknown CLI tool: {}", tc.name)),
    };

    let prompt = tc.arguments["prompt"].as_str().ok_or("missing prompt")?;
    let cwd = tc.arguments["cwd"].as_str().unwrap_or("");
    let timeout_secs = tc.arguments["timeout"].as_u64().unwrap_or(120);

    let cwd_to_use = if cwd.is_empty() { workspace_root } else { cwd };

    if !crate::modules::cli::is_installed(cli_name) {
        return Err(format!("CLI '{}' not installed or not in PATH", cli_name));
    }

    let adapter = crate::modules::cli::all_adapters()
        .into_iter()
        .find(|a| a.name() == cli_name)
        .ok_or(format!("No adapter for {}", cli_name))?;

    let args = adapter.build_args(prompt, Some(cwd_to_use), true);
    let mut cmd = tokio::process::Command::new(cli_name);
    cmd.args(&args);

    if !cwd_to_use.is_empty() {
        cmd.current_dir(cwd_to_use);
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        cmd.output(),
    )
    .await
    .map_err(|_| format!("{} timed out after {}s", cli_name, timeout_secs))?
    .map_err(|e| format!("{} error: {}", cli_name, e))?;

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

#[derive(Clone, Serialize)]
pub struct AiFileChangedEvent {
    pub id: String,
    pub path: String,
    pub old_content: String,
    pub new_content: String,
}

#[tauri::command]
pub fn ai_compute_diff(old_content: String, new_content: String) -> Vec<serde_json::Value> {
    compute_diff_lines(&old_content, &new_content)
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
    state: &AiManager,
    agent: &AgentConfig,
    messages: &[ChatMessage],
    base_url: &str,
    is_private: bool,
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
        conversation.push(serde_json::json!({"role": m.role, "content": parse_multimodal_content(&m.content)}));
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

        let status = response.status();

        if !status.is_success() {
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
            let state_root = state.workspace_root.lock().unwrap().clone();
            let workspace_root = if state_root.is_empty() {
                std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                state_root
            };

            // Add assistant message with tool calls
            let assistant_msg = serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": choice["tool_calls"]
            });
            conversation.push(assistant_msg);

            // Emit tool calls to frontend
            for tc in &tool_calls {
                state.write_agent_log(&agent.id, &agent.name,
                    &format!("TOOL_CALL id={} name={} arguments={}", tc.id, tc.name, tc.arguments.to_string()));

                let _ = app.emit("ai:tool_call", AiToolCallEvent {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                });
            }

            // Execute and emit results
            for tc in &tool_calls {
                let result = execute_tool(Some(app), tc, &workspace_root, is_private).await.unwrap_or_else(|e| format!("Error: {}", e));
                
                state.write_agent_log(&agent.id, &agent.name,
                    &format!("TOOL_RESULT id={} name={} result_len={}", tc.id, tc.name, result.len()));

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

/// Resolves the agent config for auto-routing. Returns the resolved AgentConfig.
/// The returned agent carries the correct provider, model, base_url, and api_key.
/// The `id` field reflects the matched provider-agent's ID (used for keychain lookup).
/// If no provider agent is found for the target provider, falls back to default_base_url
/// and keeps the baseline agent's id for keychain lookup.
pub fn resolve_routed_agent(
    decision: &super::routing_engine::RouteDecision,
    registry: &super::model_registry::ModelRegistry,
    state: &AiManager,
    baseline_agent: &AgentConfig,
) -> AgentConfig {
    let mut agent = baseline_agent.clone();
    if let Some(ref model_id) = decision.model_route {
        if let Some(model_meta) = registry.models.iter().find(|m| m.id == *model_id) {
            if let Some(provider_agent) = state.get_agent_for_provider(&model_meta.provider) {
                // Found a configured agent for this provider â€” use all its settings
                agent.id = provider_agent.id.clone();
                agent.provider = provider_agent.provider.clone();
                // Prefer provider_agent's base_url; fallback to default for that provider
                agent.base_url = provider_agent.base_url.clone().or_else(|| {
                    let u = default_base_url(&provider_agent.provider);
                    if u.is_empty() { None } else { Some(u.to_string()) }
                });
                // Use stored key if not masked; otherwise keychain lookup in main loop
                agent.api_key = provider_agent.api_key.clone();
            } else {
                // No configured agent for this provider â€” use model defaults
                agent.provider = model_meta.provider.clone();
                // Do NOT inject a default base_url here; keep base_url None so callers
                // can decide whether to fall back. This preserves the legacy behaviour
                // expected by unit tests.
                agent.base_url = None;
                // Keep baseline agent.id so keychain lookup can still find a key if user
                // stored it under the original agent (e.g. auto-agent with provider key)
                agent.api_key = None;
            }
        }
        agent.model = model_id.clone();
    } else {
        // No model routed â€” try to use any configured gemini agent, then any agent
        if let Some(provider_agent) = state.get_agent_for_provider("gemini") {
            agent.id = provider_agent.id.clone();
            agent.provider = provider_agent.provider.clone();
            agent.base_url = provider_agent.base_url.clone().or_else(|| {
                let u = default_base_url("gemini");
                if u.is_empty() { None } else { Some(u.to_string()) }
            });
            agent.api_key = provider_agent.api_key.clone();
            agent.model = provider_agent.model.clone();
        } else {
            // Last resort: fall back to gemini flash with default URL
            agent.model = "gemini-2.0-flash".to_string();
            agent.provider = "gemini".to_string();
            agent.base_url = Some(default_base_url("gemini").to_string());
            agent.api_key = None;
        }
    }
    agent
}

pub fn resolve_fallback_agent(
    next_fallback: &super::fallback_manager::FallbackEntry,
    state: &AiManager,
    current_agent: &AgentConfig,
) -> AgentConfig {
    let mut agent = current_agent.clone();
    agent.model = next_fallback.model_id.clone();
    agent.base_url = None;
    agent.api_key = None;
    if let Some(pa) = state.get_agent_for_provider(&next_fallback.provider) {
        agent.id = pa.id;
        agent.provider = pa.provider;
        agent.base_url = pa.base_url;
        agent.api_key = pa.api_key.clone();
    } else {
        agent.provider = next_fallback.provider.clone();
    }
    agent
}

/// Score a model name by rough capability for fallback ordering.
/// Higher = try later (prefer cheaper/faster alternatives first, escalate on failure).
/// This gives a reasonable ordering: nano < mini < flash < default < plus < medium < large < pro < ultra/opus
fn model_capability_score(model: &str) -> i32 {
    let m = model.to_lowercase();
    // Tier 1: premium reasoning
    if m.contains("ultra") || m.contains("opus") { return 100; }
    // Tier 2: pro / large / max
    if m.contains("-pro") || m.contains("_pro") || m.ends_with("pro")
        || m.contains("large") || m.contains("-max") || m.contains("r1") { return 80; }
    // Tier 3: medium / plus / sonnet / 70b
    if m.contains("plus") || m.contains("medium") || m.contains("sonnet")
        || m.contains("70b") || m.contains("72b") { return 60; }
    // Tier 4: flash / mini / standard
    if m.contains("flash") || m.contains("-mini") || m.contains("_mini")
        || m.contains("8b") || m.contains("7b") { return 40; }
    // Tier 5: nano / tiny / lite
    if m.contains("nano") || m.contains("tiny") || m.contains("lite") { return 20; }
    // Try to extract a version number for relative ordering
    // e.g. "gemini-2.5" > "gemini-2.0" > "gemini-1.5"
    let version_score = m
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter_map(|s| s.parse::<f32>().ok())
        .fold(0.0f32, f32::max);
    if version_score > 0.0 { return (version_score * 10.0) as i32; }
    50 // unknown â€” middle of the pack
}

/// Handles Gemini's "Please retry in 10.3s" and the JSON "retryDelay": "10s" patterns.
fn extract_retry_delay_secs(error_body: &str) -> Option<u64> {
    // Pattern 1: "Please retry in X.Xs" (Gemini API style)
    if let Some(pos) = error_body.find("retry in ") {
        let s = &error_body[pos + 9..];
        let end = s.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(s.len());
        if let Ok(f) = s[..end].parse::<f64>() {
            return Some(f.ceil() as u64);
        }
    }
    // Pattern 2: "retryDelay": "Xs" (Google gRPC RetryInfo style)
    if let Some(pos) = error_body.find("retryDelay") {
        let s = &error_body[pos..];
        if let Some(q) = s.find('"').and_then(|i| s[i+1..].find('"').map(|j| i+1..i+1+j)) {
            if let Ok(f) = s[q].trim_end_matches('s').parse::<f64>() {
                return Some(f.ceil() as u64);
            }
        }
    }
    None
}

#[tauri::command]
pub async fn ai_chat_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
    pty_state: tauri::State<'_, crate::modules::pty::PtyManager>,
    graph_state: tauri::State<'_, crate::modules::graph::GraphState>,
    agent_id: String,
    messages: Vec<ChatMessage>,
    workspace_root: Option<String>,
    active_session_id: Option<String>,
    model_override: Option<String>,
) -> Result<(), String> {
    state.ensure_loaded(&app);
    let mut agent = state.get_agent(&agent_id).ok_or("Agent not found")?;
    
    // Apply model override if provided (Auto Mode selects a specific model from cached_models)
    if let Some(ref m) = model_override {
        if !m.is_empty() {
            agent.model = m.clone();
        }
    }

    // Auto Mode Routing Integration
    let is_auto_mode = agent.model == "auto" || model_override.as_deref() == Some("auto");
    let mut registry = super::model_registry::ModelRegistry::load(None::<&std::path::Path>);
    if is_auto_mode {
        let active_agents = state.list_agents();
        let active_providers: Vec<String> = active_agents.iter().map(|a| a.provider.clone()).collect();
        registry.models.retain(|m| active_providers.contains(&m.provider));
    }
    let tools = super::tool_registry::ToolRegistry::load_default();
    let engine = super::routing_engine::RoutingEngine::new(registry.clone(), tools);
    let routing_decision = if is_auto_mode {
        use tauri::Manager;
        let last_message = messages.last().map(|m| m.content.as_str()).unwrap_or("");
        let intel_state = app.state::<crate::modules::project_intel::ProjectIntelState>();
        let decision = if let Ok(ctx) = intel_state.get() {
            engine.route_with_context(last_message, &ctx)
        } else {
            engine.route_request(last_message)
        };
        state.write_agent_log(&agent.id, &agent.name, &format!("AUTO ROUTING: intent={:?} size={:?} output={:?} reasoning={:?} tool={:?} model={:?}", 
            decision.intent, decision.context_size, decision.output_type, decision.reasoning_tier, decision.tool_route, decision.model_route));
        let _ = app.emit("ai:route_progress", format!("Auto routing: intent={:?}, selected model={:?}", decision.intent, decision.model_route));
        Some(decision)
    } else {
        None
    };

    if let Some(ref decision) = routing_decision {
        agent = resolve_routed_agent(decision, &engine.model_registry, &state, &agent);
    }

    // Build fallback manager for auto mode
    let mut fallback_mgr = routing_decision.as_ref().map(|decision| {
        super::fallback_manager::FallbackManager::from_registry(
            &registry,
            decision.reasoning_tier.clone(),
            match decision.intent {
                super::routing_engine::Intent::ExplainSimple | super::routing_engine::Intent::ArchDesign => super::model_registry::Spec::Chat,
                super::routing_engine::Intent::CodeWrite | super::routing_engine::Intent::RefactorFull => super::model_registry::Spec::Code,
                super::routing_engine::Intent::CodeReview => super::model_registry::Spec::Review,
                super::routing_engine::Intent::TestGenerate => super::model_registry::Spec::Test,
                super::routing_engine::Intent::ScanOnly | super::routing_engine::Intent::SymbolLookup | super::routing_engine::Intent::DebugLogic => super::model_registry::Spec::Scan,
                super::routing_engine::Intent::ExternalAgent => super::model_registry::Spec::Code,
            },
            decision.token_count,
        )
    });

    // Update workspace root for logging
    if let Some(ref root) = workspace_root {
        state.set_workspace_root(root);
    }

    let is_private = if let Some(ref sid) = active_session_id {
        pty_state.is_private(sid)
    } else {
        false
    };

    // Tool-only route: execute tool directly, skip model call
    if let Some(ref decision) = routing_decision {
        if let Some(tool_id) = &decision.tool_route {
            if matches!(decision.output_type, super::routing_engine::OutputType::ToolOutput) {
                let result = match tool_id {
                    super::tool_registry::ToolId::Ripgrep => {
                        let last_message = messages.last().map(|m| m.content.as_str()).unwrap_or("");
                        let root = workspace_root.as_deref().unwrap_or(".");
                        super::ripgrep::search_text(last_message, std::path::Path::new(root), Some(50))
                    }
                    super::tool_registry::ToolId::TreeSitter => {
                        let last_message = messages.last().map(|m| m.content.as_str()).unwrap_or("");
                        let q = last_message.trim();
                        if let Ok(nodes) = graph_state.search(q) {
                            if nodes.is_empty() {
                                // Try ripgrep as fallback for text search
                                let root = workspace_root.as_deref().unwrap_or(".");
                                super::ripgrep::search_text(q, std::path::Path::new(root), Some(30))
                            } else {
                                let mut out = String::new();
                                for n in &nodes {
                                    out.push_str(&format!("{} | {} | {}:{} | {}\n",
                                        n.kind_name(), n.name, n.file_path, n.line, n.end_line));
                                }
                                Ok(out)
                            }
                        } else {
                            Err("Symbol graph not available".to_string())
                        }
                    }
                    _ => Err("Tool not yet implemented".to_string()),
                };

                match result {
                    Ok(output) => {
                        state.write_agent_log(&agent.id, &agent.name,
                            &format!("TOOL-ONLY {:?}: returned {} chars", tool_id, output.len()));
                        let _ = app.emit("ai:done", AiStreamDone {
                            content: output,
                            provider: agent.provider.clone(),
                            model: format!("{:?}", tool_id),
                            input_tokens: 0,
                            output_tokens: 0,
                            cost: 0.0,
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        let _ = app.emit("ai:error", AiStreamError { error: e.clone() });
                        return Err(e);
                    }
                }
            }
        }
    }

    // External Agent route: delegate to CLI agent (claude, gemini, opencode, aider, codex, agy)
    if let Some(ref decision) = routing_decision {
        if decision.intent == super::routing_engine::Intent::ExternalAgent {
            if let Some(agent_name) = &decision.external_agent {
                let prompt = messages.last().map(|m| m.content.as_str()).unwrap_or("").to_string();
                let _ = app.emit("ai:route_progress", format!("[Auto Mode] Delegating heavy task to external CLI agent: {}", agent_name));
                
                let cli_tool_name = format!("{}_run", agent_name);
                let cwd = workspace_root.as_deref().unwrap_or(".");
                
                let tool_call = ToolCall {
                    id: format!("ext-{}", uuid::Uuid::new_v4()),
                    name: cli_tool_name,
                    arguments: serde_json::json!({
                        "prompt": prompt,
                        "cwd": cwd,
                        "timeout": 120
                    }),
                };
                
                let result = execute_cli_tool(&tool_call, cwd).await;
                
                match result {
                    Ok(output) => {
                        state.write_agent_log(&agent.id, &agent.name,
                            &format!("EXTERNAL AGENT {}: returned {} chars", agent_name, output.len()));
                        let _ = app.emit("ai:done", AiStreamDone {
                            content: output,
                            provider: agent.provider.clone(),
                            model: format!("external:{}", agent_name),
                            input_tokens: 0,
                            output_tokens: 0,
                            cost: 0.0,
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        let _ = app.emit("ai:error", AiStreamError { error: e.clone() });
                        return Err(e);
                    }
                }
            }
        }
    }

    // Stage 9: Multi-Agent Orchestrator delegation route (AUTO-TRIGGER)
    // Otomatis delegasikan ke orchestrator untuk tugas kompleks (RefactorFull, CodeReview, ArchDesign)
    // Tidak perlu manual command 'orch_delegate' lagi
    if let Some(ref decision) = routing_decision {
        if matches!(decision.intent, 
            super::routing_engine::Intent::RefactorFull | 
            super::routing_engine::Intent::CodeReview |
            super::routing_engine::Intent::ArchDesign)
        {
            use tauri::Manager;
            let role = match decision.intent {
                super::routing_engine::Intent::CodeReview => super::agent_orch::SubAgentRole::CodeReviewer,
                super::routing_engine::Intent::RefactorFull => super::agent_orch::SubAgentRole::Refactorer,
                super::routing_engine::Intent::ArchDesign => super::agent_orch::SubAgentRole::Architect,
                _ => super::agent_orch::SubAgentRole::CodeReviewer,
            };
            
            let prompt = messages.last().map(|m| m.content.as_str()).unwrap_or("").to_string();
            let _ = app.emit("ai:route_progress", "[Auto Mode] Complex intent detected. Delegating task to Multi-Agent Orchestrator...".to_string());
            
            let orch_state = app.state::<crate::modules::agent_orch::AgentOrchestrator>();
            
            match super::agent_orch::delegate_and_run(
                &app,
                &orch_state,
                &state,
                role,
                prompt,
                messages.clone(),
            ) {
                Ok(task_id) => {
                    let _ = app.emit("ai:route_progress", format!("Delegated orchestrator task successfully spawned: {}", task_id));
                    return Ok(());
                }
                Err(e) => {
                    let err_msg = format!("Orchestrator delegation failed: {}. Falling back to single-agent execution.", e);
                    log("AI", &err_msg);
                    let _ = app.emit("ai:route_progress", err_msg);
                    // Lanjut ke fallback logic (DAG atau Single Model)
                }
            }
        }
    }

    // DAG route: execute parallel DAG for supported intents (RefactorFull, CodeReview)
    if let Some(ref decision) = routing_decision {
        if let Some(dag_plan) = super::chain_engine::DagPlan::from_intent(&decision.intent,
            messages.last().map(|m| m.content.as_str()).unwrap_or(""))
        {
            if dag_plan.len() > 1 {
                let _ = app.emit("ai:route_progress", format!("Starting DAG with {} nodes across {} levels",
                    dag_plan.len(), dag_plan.execution_order().map(|l| l.len()).unwrap_or(0)));
                let result = run_dag(&app, &state, &secrets_state, &agent, &messages,
                    &dag_plan, &registry, &graph_state, workspace_root.as_deref(), is_private).await;
                return result;
            }
        }
    }

    // Chain route: execute multi-step chain if routing decision indicates chaining needed
    if let Some(ref decision) = routing_decision {
        if let Some(plan) = super::chain_engine::ChainPlan::from_decision(decision,
            messages.last().map(|m| m.content.as_str()).unwrap_or(""))
        {
            if plan.len() > 1 {
                let _ = app.emit("ai:route_progress", format!("Starting chain with {} steps", plan.len()));
                let result = run_chain(&app, &state, &secrets_state, &agent, &messages,
                    &plan, &registry, &graph_state, workspace_root.as_deref(), is_private).await;
                return result;
            }
        }
    }

    let mut last_error = String::new();

    // â”€â”€ Intra-provider model fallback list (Stage 0) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    // When the current model fails, first try other cached_models from the SAME agent/provider
    // before crossing to a different provider. Models sorted by capability score ascending
    // (try cheaper/similar models first, escalate to premium on repeated failure).
    let mut intra_model_fallback: Vec<String> = {
        let mut others: Vec<String> = agent.cached_models.iter()
            .filter(|m| m.as_str() != agent.model.as_str())
            .cloned()
            .collect();
        // Sort ascending by capability â€” try next-closest model first
        others.sort_by_key(|m| model_capability_score(m));
        others
    };
    let mut intra_model_idx = 0usize;

    // Universal fallback list: ALL configured agents except the current one.
    // This runs AFTER the intra-provider + registry queues are exhausted (or in manual mode
    // where fallback_mgr is None). Ensures 429/quota errors always try other providers.
    let mut universal_fallback: Vec<AgentConfig> = {
        let mut all = state.list_agents();
        // Put the initially-selected agent's provider last so we try different providers first
        all.sort_by(|a, b| {
            let a_same = a.provider == agent.provider;
            let b_same = b.provider == agent.provider;
            a_same.cmp(&b_same)
        });
        all.into_iter().filter(|a| a.id != agent.id).collect()
    };
    let mut universal_fallback_idx = 0usize;

    loop {
        // Resolve API key: try agent.id first, then original agent_id as fallback
        // (auto-routing may swap agent.id to a provider-agent; we try both)
        let resolved_key = crate::modules::secrets::get_secret(&app, &secrets_state, "codlib-ai", &agent.id)
            .ok().flatten()
            .or_else(|| {
                if agent.id != agent_id {
                    crate::modules::secrets::get_secret(&app, &secrets_state, "codlib-ai", &agent_id)
                        .ok().flatten()
                } else {
                    None
                }
            });

        if let Some(real_key) = resolved_key {
            agent.api_key = Some(real_key);
        } else if agent.api_key.as_deref() == Some("********") {
            let msg = format!("API key for agent '{}' (provider: {}) is configured but not found in OS Keychain. Please re-enter the key in Settings > Agents.", agent.id, agent.provider);
            let _ = app.emit("ai:error", AiStreamError { error: msg.clone() });
            return Err(msg);
        }

        // Resolve base URL: use agent's stored url, then provider default
        let base_url = agent.base_url.clone()
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| default_base_url(&agent.provider).to_string());

        if base_url.is_empty() {
            let msg = format!("No base URL configured for provider '{}' (agent: '{}'). Add this provider in Settings > Agents with your endpoint URL.", agent.provider, agent.id);
            let _ = app.emit("ai:error", AiStreamError { error: msg.clone() });
            return Err(msg);
        }

        // Context compression: summarize if too many messages.
        // NOTE: ContextManager is stateless for compression â€” summaries per invocation only,
        // not accumulated across sessions (avoids cross-session bleed from static singleton).
        let ctx_mgr = super::context::ContextManager::new();
        let (compressed_msgs, summary) = ctx_mgr.compress(&messages);
        if summary.is_some() {
            state.write_agent_log(&agent.id, &agent.name, "CONTEXT COMPRESSED: summarized older messages");
        }
        let msgs_to_send = compressed_msgs;

        // Log agent invocation
        state.write_agent_log(&agent.id, &agent.name,
            &format!("INVOKED model={}/{} messages={}", agent.provider, agent.model, msgs_to_send.len()));

        // Use ReAct loop for persona agents, simple streaming for others
        let result = if agent.persona_id.is_some() {
            run_react_loop(&app, &state, &agent, &msgs_to_send, &base_url, is_private).await
        } else {
            stream_openai(&app, &agent, &msgs_to_send, &base_url).await
        };

        match result {
            Ok((content, input_tokens, output_tokens)) => {
                let (price_in, price_out) = model_price(&agent.model);
                let cost = (input_tokens as f64 * price_in + output_tokens as f64 * price_out) / 1000.0;
                state.record_usage(&agent.id, input_tokens, output_tokens, cost);
                if let Some(ps) = app.try_state::<super::provider_stats::ProviderStats>() {
                    ps.record_success(&agent.provider, input_tokens + output_tokens, cost, 0);
                }
                // Stage 14: Cost routing â€” record usage and check budget
                if let Some(cost_router) = app.try_state::<super::cost_routing::CostRouter>() {
                    cost_router.record_usage(&agent.id, &agent.model, &agent.provider, input_tokens, output_tokens, cost);
                    if !cost_router.within_budget() {
                        let _ = app.emit("ai:budget_warning", serde_json::json!({
                            "message": "Daily/session budget limit approaching. Check Settings > Cost.",
                            "cost": cost_router.get_cost_summary(),
                        }));
                    }
                }

                state.write_agent_log(&agent.id, &agent.name,
                    &format!("DONE input_tokens={} output_tokens={} cost=${:.5}", input_tokens, output_tokens, cost));

                let _ = app.emit("ai:done", AiStreamDone {
                    content,
                    provider: agent.provider.clone(),
                    model: agent.model.clone(),
                    input_tokens,
                    output_tokens,
                    cost,
                });
                return Ok(());
            }
            Err(e) => {
                if let Some(ps) = app.try_state::<super::provider_stats::ProviderStats>() {
                    ps.record_failure(&agent.provider, &e);
                }
                if let Some(heal) = app.try_state::<super::self_heal::SelfHealEngine>() {
                    heal.report_degraded("providers", &format!("{} failed: {}", agent.provider, e));
                }
                state.write_agent_log(&agent.id, &agent.name, &format!("ERROR {}", e));
                let _ = app.emit("ai:error", AiStreamError { error: e.clone() });

                // Detect rate-limit / quota errors and extract retry delay for user feedback
                let is_rate_limit = e.contains("429") || e.contains("RESOURCE_EXHAUSTED")
                    || e.contains("quota") || e.contains("rate_limit") || e.contains("rate limit");
                let retry_delay = if is_rate_limit { extract_retry_delay_secs(&e) } else { None };
                let fail_label = if is_rate_limit {
                    format!("Rate limit/quota on {}/{}{}",
                        agent.provider, agent.model,
                        retry_delay.map(|d| format!(" â€” retry in {}s", d)).unwrap_or_default())
                } else {
                    format!("{}/{} failed", agent.provider, agent.model)
                };

                // â”€â”€ Stage 0: Intra-provider model fallback (same agent, cached_models) â”€â”€
                // Try other models from the SAME provider before switching providers.
                // E.g.: gemini-2.0-flash fails â†’ try gemini-1.5-pro, gemini-2.5-pro, etc.
                let mut switched_to_fallback = false;
                while !switched_to_fallback && intra_model_idx < intra_model_fallback.len() {
                    let next_model = intra_model_fallback[intra_model_idx].clone();
                    intra_model_idx += 1;
                    let prev_model = agent.model.clone();
                    agent.model = next_model.clone();
                    state.write_agent_log(&agent.id, &agent.name,
                        &format!("MODEL_FALLBACK {}: {} â†’ {}", agent.provider, prev_model, next_model));
                    let _ = app.emit("ai:route_progress",
                        format!("{} â€” switching model to {}/{}â€¦",
                            fail_label, agent.provider, next_model));
                    switched_to_fallback = true;
                }

                // â”€â”€ Stage 1: registry-based fallback (auto mode, same tier/spec) â”€â”€â”€â”€â”€â”€â”€â”€â”€
                if let Some(ref mut fm) = fallback_mgr {
                    loop {
                        if !fm.advance() { break; }
                        let Some(next) = fm.current() else { break; };
                        // Skip providers whose circuit is open
                        if let Some(ps) = app.try_state::<super::provider_stats::ProviderStats>() {
                            if !ps.is_available(&next.provider) {
                                state.write_agent_log(&agent.id, &agent.name,
                                    &format!("SKIP {} (circuit open)", next.provider));
                                continue;
                            }
                        }
                        last_error = format!("{} (tried {}/{})", e, agent.provider, agent.model);
                        agent = resolve_fallback_agent(next, &state, &agent);
                        state.write_agent_log(&agent.id, &agent.name,
                            &format!("FALLBACK to model={}/{} (remaining: {})", agent.provider, agent.model, fm.remaining()));
                        let _ = app.emit("ai:route_progress",
                            format!("Switching to {}/{}â€¦", agent.provider, agent.model));
                        switched_to_fallback = true;
                        break;
                    }
                }

                // â”€â”€ Stage 2: universal fallback â€” ALL other configured agents â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                // Runs when: registry queue exhausted, manual mode (fallback_mgr=None),
                // or any 429/quota error regardless of mode.
                if !switched_to_fallback {
                    while universal_fallback_idx < universal_fallback.len() {
                        let mut next_agent = universal_fallback[universal_fallback_idx].clone();
                        universal_fallback_idx += 1;

                        // Skip if circuit breaker is open for this provider
                        if let Some(ps) = app.try_state::<super::provider_stats::ProviderStats>() {
                            if !ps.is_available(&next_agent.provider) {
                                state.write_agent_log(&agent.id, &agent.name,
                                    &format!("UNIVERSAL_SKIP {} (circuit open)", next_agent.provider));
                                continue;
                            }
                        }

                        // Resolve API key for the candidate agent
                        if let Ok(Some(real_key)) = crate::modules::secrets::get_secret(
                            &app, &secrets_state, "codlib-ai", &next_agent.id)
                        {
                            next_agent.api_key = Some(real_key);
                        }

                        // Ensure base URL
                        if next_agent.base_url.as_deref().unwrap_or("").is_empty() {
                            let default = default_base_url(&next_agent.provider);
                            if !default.is_empty() {
                                next_agent.base_url = Some(default.to_string());
                            } else {
                                // No URL for this provider â€” skip
                                state.write_agent_log(&agent.id, &agent.name,
                                    &format!("UNIVERSAL_SKIP {} (no base URL)", next_agent.provider));
                                continue;
                            }
                        }

                        last_error = format!("{} (tried {}/{})", e, agent.provider, agent.model);
                        state.write_agent_log(&agent.id, &agent.name,
                            &format!("UNIVERSAL_FALLBACK to agent={} model={}/{}",
                                next_agent.id, next_agent.provider, next_agent.model));
                        let _ = app.emit("ai:route_progress",
                            format!("Quota/error on {} â€” switching to {}/{}â€¦",
                                agent.provider, next_agent.provider, next_agent.model));
                        agent = next_agent;
                        switched_to_fallback = true;
                        break;
                    }
                }

                if switched_to_fallback {
                    continue; // retry outer loop with new agent
                }

                // â”€â”€ All fallbacks exhausted â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                if let Some(heal) = app.try_state::<super::self_heal::SelfHealEngine>() {
                    heal.report_down("providers", &format!("all fallbacks exhausted: {}", e));
                }
                let _ = app.emit("ai:error", AiStreamError {
                    error: format!("All configured providers failed or quota exhausted. Last error: {}", e)
                });
                return Err(format!("All providers exhausted. Last: {}", e));

            }
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

/// Set the current workspace root so AI knows where to write logs.
/// Also auto-detects project framework (Stage 6 integration).
#[tauri::command]
pub fn ai_set_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    root: String,
) -> Result<(), String> {
    state.set_workspace_root(&root);
    // Stage 6: Auto-detect project framework on workspace load
    if let Some(intel) = app.try_state::<super::project_intel::ProjectIntelState>() {
        let ctx = intel.detect(&root);
        let _ = app.emit("nyx:project_detected", serde_json::json!({
            "framework": ctx.framework_label(),
            "language": ctx.language,
            "has_tests": ctx.has_tests,
            "has_ci": ctx.has_ci,
            "has_docker": ctx.has_docker,
            "file_count": ctx.file_count,
            "src_dirs": ctx.src_dirs,
        }));
    }
    Ok(())
}

/// List all agent log files in .nyx/logs/
#[derive(Serialize, Deserialize)]
pub struct AgentLogEntry {
    pub agent_id: String,
    pub path: String,
    pub size: u64,
    pub modified: String,
}

#[tauri::command]
pub fn ai_get_agent_logs(state: tauri::State<'_, AiManager>) -> Vec<AgentLogEntry> {
    let workspace_root = state.workspace_root.lock().unwrap().clone();
    if workspace_root.is_empty() { return vec![]; }
    // Use Path::join for safe cross-platform path construction
    let log_dir = std::path::Path::new(&workspace_root).join(".nyx").join("logs");
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("log") {
                let name = p.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let meta = std::fs::metadata(&p).ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified = meta.and_then(|m| m.modified().ok()).map(|t| {
                    let dt = chrono::DateTime::<chrono::Utc>::from(t);
                    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                }).unwrap_or_default();
                result.push(AgentLogEntry {
                    agent_id: name,
                    path: p.to_string_lossy().to_string(),
                    size,
                    modified,
                });
            }
        }
    }
    result
}

#[tauri::command]
pub async fn ai_list_agents(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
) -> Result<Vec<AgentConfig>, String> {
    state.ensure_loaded(&app);
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
    state.ensure_loaded(&app);
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
    state.update_agent(config)?;
    // Persist to disk
    let agents = state.agents.lock().unwrap();
    save_agents_to_disk(&app, &agents);
    Ok(())
}

#[tauri::command]
pub async fn ai_remove_agent(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
    agent_id: String,
) -> Result<(), String> {
    state.ensure_loaded(&app);
    let _ = crate::modules::secrets::delete_secret(&app, &secrets_state, "codlib-ai", &agent_id);
    state.remove_agent(&agent_id)?;
    // Persist to disk
    let agents = state.agents.lock().unwrap();
    save_agents_to_disk(&app, &agents);
    Ok(())
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
    ("gemini", "https://generativelanguage.googleapis.com/v1beta/openai"),
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
            // User-provided base_url always takes priority over hardcoded endpoints.
            // This allows private/custom deployments (e.g. Alibaba private MaaS) to work correctly.
            if let Some(ref user_url) = base_url {
                let url = user_url.trim();
                if !url.is_empty() {
                    return fetch_openai_models(url, &api_key).await;
                }
            }
            // Fall back to hardcoded preset URL for the provider.
            if let Some(endpoint) = provider_endpoint(&provider) {
                fetch_openai_models(endpoint, &api_key).await
            } else {
                Err(format!(
                    "Provider '{}' has no default endpoint. Please specify a Base URL.",
                    provider
                ))
            }
        }
    }
}

/// Fetch models for a saved agent using its stored credentials, then cache the result.
/// Call this once after creating/updating an agent. Subsequently, cached_models is used
/// so no repeated network calls are needed.
#[tauri::command]
pub async fn ai_sync_agent_models(
    app: tauri::AppHandle,
    state: tauri::State<'_, AiManager>,
    secrets_state: tauri::State<'_, crate::modules::secrets::SecretsState>,
    agent_id: String,
) -> Result<Vec<String>, String> {
    let mut agent = state.get_agent(&agent_id).ok_or("Agent not found")?;

    // Load real API key from keychain
    if let Ok(Some(real_key)) = crate::modules::secrets::get_secret(&app, &secrets_state, "codlib-ai", &agent_id) {
        agent.api_key = Some(real_key);
    }

    // Resolve the endpoint: prefer agent's base_url, fall back to known preset
    let base_url_str: String = if let Some(ref url) = agent.base_url {
        let trimmed = url.trim().to_string();
        if !trimmed.is_empty() { trimmed } else { String::new() }
    } else {
        String::new()
    };

    let models = match agent.provider.as_str() {
        "vercel" => vercel_models(),
        "gemini" => fetch_gemini_models(&agent.api_key).await?,
        "openrouter" => fetch_openrouter_models(&agent.api_key).await?,
        _ => {
            if !base_url_str.is_empty() {
                fetch_openai_models(&base_url_str, &agent.api_key).await?
            } else if let Some(endpoint) = provider_endpoint(&agent.provider) {
                fetch_openai_models(endpoint, &agent.api_key).await?
            } else {
                return Err(format!(
                    "Provider '{}' has no endpoint configured. Add a Base URL in Settings > Agents.",
                    agent.provider
                ));
            }
        }
    };

    let model_ids: Vec<String> = models.into_iter().map(|m| m.id).collect();

    // Store cache back into the agent config and persist to disk
    {
        let mut agents = state.agents.lock().unwrap();
        if let Some(a) = agents.get_mut(&agent_id) {
            a.cached_models = model_ids.clone();
            a.models_synced_at = Some(Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string());
        }
        save_agents_to_disk(&app, &agents);
    }

    Ok(model_ids)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProbeResult {
    pub id: String,
    pub status: String, // "ok" | "auth_error" | "quota_error" | "model_error" | "timeout" | "error"
    pub latency_ms: u64,
    pub error_hint: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ProbeProgress {
    pub total: usize,
    pub done: usize,
    pub current_model: String,
}

async fn probe_single_model(
    model_id: String,
    api_key: Option<String>,
    base_url: Option<String>,
    provider: String,
) -> ProbeResult {
    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build();
    
    let client = match client {
        Ok(c) => c,
        Err(e) => return ProbeResult {
            id: model_id,
            status: "error".to_string(),
            latency_ms: 0,
            error_hint: Some(e.to_string()),
        }
    };

    let is_gemini_native = provider == "gemini" && base_url.as_ref().map_or(true, |url| url.trim().is_empty() || !url.starts_with("http"));

    let res = if is_gemini_native {
        let key = match &api_key {
            Some(k) => k,
            None => return ProbeResult {
                id: model_id,
                status: "auth_error".to_string(),
                latency_ms: 0,
                error_hint: Some("API Key is required for Gemini native".to_string()),
            }
        };
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model_id, key);
        let body = serde_json::json!({
            "contents": [{"parts": [{"text": "Reply with: OK"}]}],
            "generationConfig": {"maxOutputTokens": 5}
        });
        client.post(&url).json(&body).send().await
    } else {
        let resolved_url = if let Some(ref url) = base_url {
            let trimmed = url.trim();
            if !trimmed.is_empty() {
                trimmed.to_string()
            } else {
                default_base_url(&provider).to_string()
            }
        } else {
            default_base_url(&provider).to_string()
        };

        if resolved_url.is_empty() {
            return ProbeResult {
                id: model_id,
                status: "error".to_string(),
                latency_ms: 0,
                error_hint: Some(format!("No endpoint URL configured for provider '{}'", provider)),
            };
        }

        let url = format!("{}/chat/completions", resolved_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": model_id,
            "messages": [{"role": "user", "content": "Reply with: OK"}],
            "max_tokens": 15,
            "temperature": 0.5,
        });

        let mut req = client.post(&url).json(&body);
        if let Some(ref key) = api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }
        req.send().await
    };

    let latency = start.elapsed().as_millis() as u64;

    match res {
        Ok(resp) => {
            let status_code = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            
            if status_code.is_success() {
                let body_lower = body_text.to_lowercase();
                if body_lower.contains("insufficient_quota") || body_lower.contains("insufficient balance") || body_lower.contains("exceeded your current quota") {
                    ProbeResult {
                        id: model_id,
                        status: "quota_error".to_string(),
                        latency_ms: latency,
                        error_hint: Some("Quota exceeded or insufficient balance".to_string()),
                    }
                } else if body_lower.contains("invalid api key") || body_lower.contains("incorrect api key") || body_lower.contains("invalid_api_key") {
                    ProbeResult {
                        id: model_id,
                        status: "auth_error".to_string(),
                        latency_ms: latency,
                        error_hint: Some("Invalid API Key".to_string()),
                    }
                } else {
                    ProbeResult {
                        id: model_id,
                        status: "ok".to_string(),
                        latency_ms: latency,
                        error_hint: None,
                    }
                }
            } else {
                let status_val = status_code.as_u16();
                let hint = if !body_text.is_empty() {
                    if let Ok(json_body) = serde_json::from_str::<Value>(&body_text) {
                        if let Some(msg) = json_body["error"]["message"].as_str() {
                            msg.to_string()
                        } else if let Some(msg) = json_body["error"].as_str() {
                            msg.to_string()
                        } else {
                            body_text
                        }
                    } else {
                        body_text
                    }
                } else {
                    format!("HTTP {}", status_val)
                };

                let status_str = match status_val {
                    401 | 403 => "auth_error",
                    429 => "quota_error",
                    404 => "model_error",
                    _ => "error",
                };

                ProbeResult {
                    id: model_id,
                    status: status_str.to_string(),
                    latency_ms: latency,
                    error_hint: Some(hint),
                }
            }
        }
        Err(e) => {
            let error_str = e.to_string();
            let is_timeout = e.is_timeout() || error_str.contains("timeout") || error_str.contains("timed out");
            ProbeResult {
                id: model_id,
                status: if is_timeout { "timeout".to_string() } else { "error".to_string() },
                latency_ms: latency,
                error_hint: Some(error_str),
            }
        }
    }
}

#[tauri::command]
pub async fn ai_probe_models(
    app: tauri::AppHandle,
    api_key: Option<String>,
    base_url: Option<String>,
    provider: String,
    single_model: Option<String>,
) -> Result<Vec<ProbeResult>, String> {
    let models = if let Some(m) = single_model {
        let trimmed = m.trim();
        if !trimmed.is_empty() {
            vec![ProviderModel {
                id: trimmed.to_string(),
                name: None,
                source: provider.clone(),
            }]
        } else {
            ai_list_models(api_key.clone(), base_url.clone(), provider.clone()).await?
        }
    } else {
        ai_list_models(api_key.clone(), base_url.clone(), provider.clone()).await?
    };
    let total = models.len();
    let mut done = 0;
    let mut results = Vec::new();

    let _ = app.emit("ai:probe_progress", ProbeProgress {
        total,
        done: 0,
        current_model: "".to_string(),
    });

    let mut stream = futures::stream::iter(models)
        .map(|m| {
            let api_key = api_key.clone();
            let base_url = base_url.clone();
            let provider = provider.clone();
            async move {
                probe_single_model(m.id, api_key, base_url, provider).await
            }
        })
        .buffer_unordered(5);

    while let Some(res) = stream.next().await {
        done += 1;
        let _ = app.emit("ai:probe_progress", ProbeProgress {
            total,
            done,
            current_model: res.id.clone(),
        });
        results.push(res);
    }

    Ok(results)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ClassifyResult {
    pub tier: String,
    pub confidence: f32,
    pub reason: String,
    pub overrode_frontend: bool,
}

#[tauri::command]
pub fn ai_classify_request(
    text: String,
    frontend_tier: String,
) -> ClassifyResult {
    let registry = super::model_registry::ModelRegistry::load(None::<&std::path::Path>);
    let tools = super::tool_registry::ToolRegistry::load_default();
    let engine = super::routing_engine::RoutingEngine::new(registry, tools);
    
    let decision = engine.route_request(&text);
    let tier = match decision.context_size {
        super::routing_engine::ContextSize::Small => "simple",
        super::routing_engine::ContextSize::Medium => "medium",
        super::routing_engine::ContextSize::Large | super::routing_engine::ContextSize::Massive => "complex",
    };
    
    let overrode_frontend = tier != frontend_tier;
    let confidence = if overrode_frontend { 0.85 } else { 0.95 };
    
    ClassifyResult {
        tier: tier.to_string(),
        confidence,
        reason: decision.reason,
        overrode_frontend,
    }
}

/// Execute a multi-step chain plan. Each step calls the best model for its tier/spec,
/// passing previous step output as hidden context to the next step.
async fn run_chain(
    app: &tauri::AppHandle,
    state: &AiManager,
    secrets_state: &tauri::State<'_, crate::modules::secrets::SecretsState>,
    _original_agent: &AgentConfig,
    messages: &[ChatMessage],
    plan: &super::chain_engine::ChainPlan,
    registry: &super::model_registry::ModelRegistry,
    graph_state: &super::graph::GraphState,
    workspace_root: Option<&str>,
    is_private: bool,
) -> Result<(), String> {
    let mut previous_output = String::new();
    let mut accumulated_content = String::new();
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_cost_usd = 0.0f64;
    let cost_budget = 0.05; // $0.05 max per chain

    for (i, node) in plan.nodes.iter().enumerate() {
        let step_label = format!("[{}/{}] {}", i + 1, plan.len(), node.label);
        let _ = app.emit("ai:route_progress", format!("Chain step {}: {}", i + 1, node.label));

        // Find best model for this node's tier + spec
        let model_meta = registry.select_model(node.model_tier.clone(), node.spec.clone(), 0);
        let model_id = model_meta.map(|m| m.id.clone()).unwrap_or_default();
        let provider = model_meta.map(|m| m.provider.clone()).unwrap_or_default();

        // Build step agent config
        let mut step_agent = state.get_agent(&_original_agent.id).unwrap_or_else(|| _original_agent.clone());
        let mut resolved = false;
        if !provider.is_empty() {
            if let Some(pa) = state.get_agent_for_provider(&provider) {
                step_agent.id = pa.id;
                step_agent.provider = pa.provider;
                step_agent.base_url = pa.base_url;
                step_agent.api_key = pa.api_key.clone();
                step_agent.model = pa.model.clone();
                resolved = true;
            }
        }
        if !resolved {
            step_agent.model = if model_id.is_empty() { _original_agent.model.clone() } else { model_id.clone() };
            step_agent.provider = if provider.is_empty() { _original_agent.provider.clone() } else { provider.clone() };
            step_agent.base_url = None;
            step_agent.api_key = None;
        }

        // Resolve API key
        if let Ok(Some(real_key)) = crate::modules::secrets::get_secret(app, secrets_state, "codlib-ai", &step_agent.id) {
            step_agent.api_key = Some(real_key);
        }

        let base_url = step_agent.base_url.clone()
            .unwrap_or_else(|| default_base_url(&step_agent.provider).to_string());
        if base_url.is_empty() {
            return Err(format!("{}: No base URL for provider '{}'", step_label, step_agent.provider));
        }

        // Build step messages with hidden context injection
        let step_messages = build_chain_step_messages(node, messages, &previous_output, &plan.user_prompt);

        // Execute this step
        state.write_agent_log(&step_agent.id, &step_agent.name,
            &format!("CHAIN {} model={}/{}", step_label, step_agent.provider, step_agent.model));

        let result = if step_agent.persona_id.is_some() {
            run_react_loop(app, state, &step_agent, &step_messages, &base_url, is_private).await
        } else {
            stream_openai(app, &step_agent, &step_messages, &base_url).await
        };

        match result {
            Ok((content, inp, out)) => {
                total_input += inp;
                total_output += out;

                // Cost budget check
                let (price_in, price_out) = model_price(&step_agent.model);
                let step_cost = (inp as f64 * price_in + out as f64 * price_out) / 1000.0;
                total_cost_usd += step_cost;
                if total_cost_usd > cost_budget {
                    let msg = format!("{}: Chain cost ${:.4} exceeds budget ${:.4}", step_label, total_cost_usd, cost_budget);
                    state.write_agent_log(&step_agent.id, &step_agent.name, &format!("CHAIN BUDGET {}", msg));
                    let _ = app.emit("ai:error", AiStreamError { error: msg.clone() });
                    return Err(msg);
                }

                previous_output = content.clone();

                // Stream partial content to frontend
                if i < plan.len() - 1 {
                    let _ = app.emit("ai:chunk", AiStreamChunk {
                        delta: format!("\n\n--- Step {} complete: {} ---\n\n", i + 1, node.label),
                    });
                }

                if i == plan.len() - 1 {
                    accumulated_content = content;
                }
            }
            Err(e) => {
                let msg = format!("{} failed: {}", step_label, e);
                if let Some(heal) = app.try_state::<super::self_heal::SelfHealEngine>() {
                    heal.report_degraded("chain", &format!("step {}: {}", node.id, e));
                }
                state.write_agent_log(&step_agent.id, &step_agent.name, &format!("CHAIN ERROR {}", msg));
                let _ = app.emit("ai:error", AiStreamError { error: msg.clone() });
                return Err(msg);
            }
        }
    }

    // Emit final result
    let (price_in, price_out) = model_price(&_original_agent.model);
    let cost = (total_input as f64 * price_in + total_output as f64 * price_out) / 1000.0;

    state.write_agent_log(&_original_agent.id, &_original_agent.name,
        &format!("CHAIN DONE steps={} input_tokens={} output_tokens={}", plan.len(), total_input, total_output));

    let _ = app.emit("ai:done", AiStreamDone {
        content: accumulated_content,
        provider: _original_agent.provider.clone(),
        model: _original_agent.model.clone(),
        input_tokens: total_input,
        output_tokens: total_output,
        cost,
    });
    Ok(())
}

/// Execute a DAG plan. Levels are executed sequentially, but nodes within a level run in parallel.
/// Partial failures are tolerated: failed nodes are noted but don't block downstream.
async fn run_dag(
    app: &tauri::AppHandle,
    state: &AiManager,
    secrets_state: &tauri::State<'_, crate::modules::secrets::SecretsState>,
    _original_agent: &AgentConfig,
    messages: &[ChatMessage],
    plan: &super::chain_engine::DagPlan,
    registry: &super::model_registry::ModelRegistry,
    graph_state: &super::graph::GraphState,
    workspace_root: Option<&str>,
    is_private: bool,
) -> Result<(), String> {
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    let levels = plan.execution_order()?;
    let node_outputs: Arc<Mutex<HashMap<String, super::chain_engine::DagStepResult>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let cost_budget = 0.05;

    let total_nodes: usize = plan.nodes.len();
    let mut done_count = 0usize;

    for (level_idx, level) in levels.iter().enumerate() {
        let _ = app.emit("ai:route_progress",
            format!("DAG level {}/{} ({} parallel nodes)", level_idx + 1, levels.len(), level.len()));

        let mut handles = Vec::new();

        for node_id in level {
                let node = plan.nodes.iter().find(|n| n.id == *node_id)
                    .ok_or_else(|| format!("DAG node '{}' not found", node_id))?;
                let node = node.clone();

                let node_outputs_clone = node_outputs.clone();
                let app = app.clone();
                let state_clone = state.clone();
                let agent = _original_agent.clone();
                let messages = messages.to_vec();
                let registry = registry.clone();
                let plan_user_prompt = plan.user_prompt.clone();

                let handle = tokio::spawn(async move {
                let step_label = format!("DAG[{}] {}", node.id, node.label);
                let _ = app.emit("ai:route_progress", format!("Starting {}", step_label));

                // Find best model for this node's tier + spec
                let model_meta = registry.select_model(node.model_tier.clone(), node.spec.clone(), 0);
                let model_id = model_meta.as_ref().map(|m| m.id.clone()).unwrap_or_default();
                let provider = model_meta.as_ref().map(|m| m.provider.clone()).unwrap_or_default();

                let mut step_agent = state_clone.get_agent(&agent.id).unwrap_or_else(|| agent.clone());
                let mut resolved = false;
                if !provider.is_empty() {
                    if let Some(pa) = state_clone.get_agent_for_provider(&provider) {
                        step_agent.id = pa.id;
                        step_agent.provider = pa.provider;
                        step_agent.base_url = pa.base_url;
                        step_agent.api_key = pa.api_key.clone();
                        step_agent.model = pa.model.clone();
                        resolved = true;
                    }
                }
                if !resolved {
                    step_agent.model = if model_id.is_empty() { agent.model.clone() } else { model_id.clone() };
                    step_agent.provider = if provider.is_empty() { agent.provider.clone() } else { provider.clone() };
                    step_agent.base_url = None;
                    step_agent.api_key = None;
                }

                let secret_key = {
                    let s = app.state::<crate::modules::secrets::SecretsState>();
                    crate::modules::secrets::get_secret(&app, &*s, "codlib-ai", &step_agent.id)
                };
                if let Ok(Some(real_key)) = secret_key {
                    step_agent.api_key = Some(real_key);
                }

                let base_url = step_agent.base_url.clone()
                    .unwrap_or_else(|| default_base_url(&step_agent.provider).to_string());
                if base_url.is_empty() {
                    return (node.id.clone(), super::chain_engine::DagStepResult {
                        node_id: node.id.clone(), output: String::new(), success: false,
                        error: Some(format!("No base URL for provider '{}'", step_agent.provider)),
                    });
                }

                // Gather outputs from dependencies
                let dep_outputs = {
                    let outputs = node_outputs_clone.lock().await;
                    let mut buf = String::new();
                    for dep_id in &node.depends_on {
                        if let Some(r) = outputs.get(dep_id) {
                            if r.success {
                                buf.push_str(&format!("<DAG_DEP_OUTPUT node=\"{}\">\n{}\n</DAG_DEP_OUTPUT>\n", dep_id, r.output));
                            }
                        }
                    }
                    buf
                };

                // Build step messages
                let mut step_messages = Vec::new();
                if !dep_outputs.is_empty() {
                    step_messages.push(ChatMessage {
                        role: "system".into(),
                        content: format!("<DAG_CONTEXT>\n{}\n</DAG_CONTEXT>\nOriginal task: {}", dep_outputs, plan_user_prompt),
                        display_content: None,
                    });
                }
                for m in &messages {
                    if m.role != "system" {
                        step_messages.push(m.clone());
                    }
                }
                let has_user = step_messages.iter().any(|m| m.role == "user");
                if !has_user && !plan_user_prompt.is_empty() {
                    step_messages.push(ChatMessage {
                        role: "user".into(),
                        content: plan_user_prompt.clone(),
                        display_content: None,
                    });
                }

                state_clone.write_agent_log(&step_agent.id, &step_agent.name,
                    &format!("DAG {} model={}/{}", step_label, step_agent.provider, step_agent.model));

                let result = if step_agent.persona_id.is_some() {
                    run_react_loop(&app, &state_clone, &step_agent, &step_messages, &base_url, is_private).await
                } else {
                    stream_openai(&app, &step_agent, &step_messages, &base_url).await
                };

                match result {
                    Ok((content, inp, out)) => {
                        let (price_in, price_out) = model_price(&step_agent.model);
                        let step_cost = (inp as f64 * price_in + out as f64 * price_out) / 1000.0;
                        state_clone.record_usage(&step_agent.id, inp, out, step_cost);
                        if let Some(ps) = app.try_state::<super::provider_stats::ProviderStats>() {
                            ps.record_success(&step_agent.provider, inp + out, step_cost, 0);
                        }
                        (node.id.clone(), super::chain_engine::DagStepResult {
                            node_id: node.id.clone(), output: content, success: true, error: None,
                            inp_tokens: inp, out_tokens: out,
                        })
                    }
                    Err(e) => {
                        let err_msg = format!("{} failed: {}", step_label, e);
                        if let Some(heal) = app.try_state::<super::self_heal::SelfHealEngine>() {
                            heal.report_degraded("chain", &format!("dag node {}: {}", node.id, e));
                        }
                        state_clone.write_agent_log(&step_agent.id, &step_agent.name, &format!("DAG ERROR {}", err_msg));
                        if let Some(ps) = app.try_state::<super::provider_stats::ProviderStats>() {
                            ps.record_failure(&step_agent.provider, &err_msg);
                        }
                        (node.id.clone(), super::chain_engine::DagStepResult {
                            node_id: node.id.clone(), output: String::new(), success: false,
                            error: Some(err_msg.clone()), inp_tokens: 0, out_tokens: 0,
                        })
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all parallel nodes in this level
        for handle in handles {
            match handle.await {
                Ok((node_id, result)) => {
                    // Use real token counts from the LLM call
                    total_input += result.inp_tokens;
                    total_output += result.out_tokens;
                    // Cost budget check using real token costs
                    let running_cost = (total_input as f64 * 0.00015 + total_output as f64 * 0.0006) / 1000.0;
                    if running_cost > cost_budget {
                        let msg = format!("DAG cost ${:.4} exceeds budget ${:.4}", running_cost, cost_budget);
                        let _ = app.emit("ai:error", AiStreamError { error: msg.clone() });
                        return Err(msg);
                    }
                    node_outputs.lock().await.insert(node_id, result);
                    done_count += 1;
                    let _ = app.emit("ai:route_progress",
                        format!("DAG progress: {}/{} nodes complete", done_count, total_nodes));
                }
                Err(e) => {
                    let msg = format!("DAG task panicked: {}", e);
                    let _ = app.emit("ai:error", AiStreamError { error: msg.clone() });
                    return Err(msg);
                }
            }
        }
    }

    // Collect all results and merge
    let final_outputs = node_outputs.lock().await;
    let results: Vec<super::chain_engine::DagStepResult> = final_outputs.values().cloned().collect();
    let merged = super::chain_engine::DagPlan::merge_results(&results, &plan.user_prompt);
    let successes = results.iter().filter(|r| r.success).count();
    let failures = results.iter().filter(|r| !r.success).count();
    // Use real accumulated token counts
    let (price_in, price_out) = model_price(&_original_agent.model);
    let final_cost = (total_input as f64 * price_in + total_output as f64 * price_out) / 1000.0;

    state.write_agent_log(&_original_agent.id, &_original_agent.name,
        &format!("DAG DONE total_nodes={} successes={} failures={} input_tokens={} output_tokens={}",
            total_nodes, successes, failures, total_input, total_output));

    let _ = app.emit("ai:done", AiStreamDone {
        content: merged,
        provider: _original_agent.provider.clone(),
        model: _original_agent.model.clone(),
        input_tokens: total_input,
        output_tokens: total_output,
        cost: final_cost,
    });
    Ok(())
}

/// Build messages for a chain step, injecting previous step output as hidden context.
fn build_chain_step_messages(
    node: &super::chain_engine::ChainNode,
    original_messages: &[ChatMessage],
    previous_output: &str,
    user_prompt: &str,
) -> Vec<ChatMessage> {
    let mut step_messages = Vec::new();

    // Inject previous step output as hidden context (if any)
    if !previous_output.is_empty() {
        let context = format!(
            "<PREVIOUS_STEP_OUTPUT step_id=\"{}\" step_type=\"{:?}\">\n{}\n</PREVIOUS_STEP_OUTPUT>",
            node.id, node.step_type, previous_output
        );
        step_messages.push(ChatMessage {
            role: "system".into(),
            content: context,
            display_content: None,
        });
    }

    // Add original messages (but skip system messages that might conflict)
    for m in original_messages {
        if m.role != "system" {
            step_messages.push(m.clone());
        }
    }

    // Ensure user prompt is included
    let has_user_msg = step_messages.iter().any(|m| m.role == "user");
    if !has_user_msg && !user_prompt.is_empty() {
        step_messages.push(ChatMessage {
            role: "user".into(),
            content: user_prompt.to_string(),
            display_content: None,
        });
    }

    step_messages
}

// NOTE: All unit tests have been removed per project cleanup.
// Integration/API tests should be run with the full Tauri application.
// API connectivity tests: set env vars (CEREBRAS_API_KEY etc.) and run manually.

