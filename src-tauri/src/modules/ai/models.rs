use serde_json::Value;

const PROVIDER_ENDPOINTS: &[(&str, &str)] = &[
    ("openai", "https://api.openai.com/v1"),
    ("cerebras", "https://api.cerebras.ai/v1"),
    ("mistral", "https://api.mistral.ai/v1"),
    ("alibaba", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
    ("xai", "https://api.x.ai/v1"),
    ("openrouter", "https://openrouter.ai/api/v1"),
    ("gemini", "https://generativelanguage.googleapis.com/v1beta/openai"),
];

pub fn provider_endpoint(provider: &str) -> Option<&'static str> {
    PROVIDER_ENDPOINTS.iter().find(|(p, _)| *p == provider).map(|(_, url)| *url)
}

pub async fn fetch_models_json(url: &str, api_key: &Option<String>) -> Result<(reqwest::StatusCode, String), String> {
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

pub async fn fetch_openai_models(url: &str, api_key: &Option<String>) -> Result<Vec<super::ProviderModel>, String> {
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
        Some(super::ProviderModel { id, name: None, source: "openai".to_string() })
    }).collect();
    Ok(list)
}

pub async fn fetch_gemini_models(api_key: &Option<String>) -> Result<Vec<super::ProviderModel>, String> {
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
        Some(super::ProviderModel { id: id.to_string(), name: None, source: "gemini".to_string() })
    }).collect();
    Ok(list)
}

pub async fn fetch_openrouter_models(api_key: &Option<String>) -> Result<Vec<super::ProviderModel>, String> {
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
        Some(super::ProviderModel { id, name, source: "openrouter".to_string() })
    }).collect();
    Ok(list)
}

pub fn vercel_models() -> Vec<super::ProviderModel> {
    vec![
        super::ProviderModel { id: "openai/gpt-4o".into(), name: Some("GPT-4o (OpenAI)".into()), source: "vercel".into() },
        super::ProviderModel { id: "openai/gpt-4o-mini".into(), name: Some("GPT-4o Mini (OpenAI)".into()), source: "vercel".into() },
        super::ProviderModel { id: "openai/gpt-4-turbo".into(), name: Some("GPT-4 Turbo (OpenAI)".into()), source: "vercel".into() },
        super::ProviderModel { id: "openai/gpt-3.5-turbo".into(), name: Some("GPT-3.5 Turbo (OpenAI)".into()), source: "vercel".into() },
        super::ProviderModel { id: "openai/o3-mini".into(), name: Some("o3 Mini (OpenAI)".into()), source: "vercel".into() },
        super::ProviderModel { id: "anthropic/claude-sonnet-4-20250514".into(), name: Some("Claude Sonnet 4 (Anthropic)".into()), source: "vercel".into() },
        super::ProviderModel { id: "anthropic/claude-3-5-sonnet-latest".into(), name: Some("Claude 3.5 Sonnet (Anthropic)".into()), source: "vercel".into() },
        super::ProviderModel { id: "anthropic/claude-3-5-haiku-latest".into(), name: Some("Claude 3.5 Haiku (Anthropic)".into()), source: "vercel".into() },
        super::ProviderModel { id: "anthropic/claude-opus-4-20250514".into(), name: Some("Claude Opus 4 (Anthropic)".into()), source: "vercel".into() },
        super::ProviderModel { id: "google/gemini-2.0-flash".into(), name: Some("Gemini 2.0 Flash (Google)".into()), source: "vercel".into() },
        super::ProviderModel { id: "google/gemini-2.0-flash-lite".into(), name: Some("Gemini 2.0 Flash Lite (Google)".into()), source: "vercel".into() },
        super::ProviderModel { id: "google/gemini-2.5-pro".into(), name: Some("Gemini 2.5 Pro (Google)".into()), source: "vercel".into() },
        super::ProviderModel { id: "google/gemini-1.5-pro".into(), name: Some("Gemini 1.5 Pro (Google)".into()), source: "vercel".into() },
        super::ProviderModel { id: "google/gemini-1.5-flash".into(), name: Some("Gemini 1.5 Flash (Google)".into()), source: "vercel".into() },
        super::ProviderModel { id: "deepseek/deepseek-chat".into(), name: Some("DeepSeek Chat (DeepSeek)".into()), source: "vercel".into() },
        super::ProviderModel { id: "deepseek/deepseek-reasoner".into(), name: Some("DeepSeek Reasoner (DeepSeek)".into()), source: "vercel".into() },
        super::ProviderModel { id: "cerebras/llama3.1-8b".into(), name: Some("Llama 3.1 8B (Cerebras)".into()), source: "vercel".into() },
        super::ProviderModel { id: "cerebras/llama3.1-70b".into(), name: Some("Llama 3.1 70B (Cerebras)".into()), source: "vercel".into() },
        super::ProviderModel { id: "mistral/mistral-large-latest".into(), name: Some("Mistral Large (Mistral)".into()), source: "vercel".into() },
        super::ProviderModel { id: "xai/grok-2".into(), name: Some("Grok 2 (xAI)".into()), source: "vercel".into() },
        super::ProviderModel { id: "perplexity/sonar-pro".into(), name: Some("Sonar Pro (Perplexity)".into()), source: "vercel".into() },
    ]
}

pub async fn probe_single_model(
    model_id: String,
    api_key: Option<String>,
    base_url: Option<String>,
    provider: String,
) -> super::ProbeResult {
    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => return super::ProbeResult {
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
            None => return super::ProbeResult {
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
                super::default_base_url(&provider).to_string()
            }
        } else {
            super::default_base_url(&provider).to_string()
        };

        if resolved_url.is_empty() {
            return super::ProbeResult {
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
                    super::ProbeResult {
                        id: model_id,
                        status: "quota_error".to_string(),
                        latency_ms: latency,
                        error_hint: Some("Quota exceeded or insufficient balance".to_string()),
                    }
                } else if body_lower.contains("invalid api key") || body_lower.contains("incorrect api key") || body_lower.contains("invalid_api_key") {
                    super::ProbeResult {
                        id: model_id,
                        status: "auth_error".to_string(),
                        latency_ms: latency,
                        error_hint: Some("Invalid API Key".to_string()),
                    }
                } else {
                    super::ProbeResult {
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

                super::ProbeResult {
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
            super::ProbeResult {
                id: model_id,
                status: if is_timeout { "timeout".to_string() } else { "error".to_string() },
                latency_ms: latency,
                error_hint: Some(error_str),
            }
        }
    }
}
