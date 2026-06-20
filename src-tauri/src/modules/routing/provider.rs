use async_trait::async_trait;

/// Minimal chat message type for provider boundaries. Later this can be
/// replaced by the crate-wide ChatMessage used by ai.rs to avoid duplication.
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Stream a chat completion. Returns (text, input_tokens, output_tokens)
    async fn stream_chat(
        &self,
        model: &str,
        messages: &[ChatMessage],
        system_prompt: &str,
    ) -> Result<(String, u64, u64), String>;

    fn name(&self) -> &str;

    /// Whether this provider supports tool-calling / ReAct style tool execution.
    fn supports_tools(&self) -> bool {
        false
    }
}

/// LiteLLM provider that proxies to a local/open endpoint implementing an
/// OpenAI-compatible API. This is a lightweight skeleton — networking
/// integration will be added in a subsequent iteration.
#[derive(Clone, Debug)]
pub struct LiteLLMProvider {
    pub endpoint: String,
    pub api_key: Option<String>,
}

impl LiteLLMProvider {
    pub fn new(endpoint: String, api_key: Option<String>) -> Self {
        Self { endpoint, api_key }
    }
}

#[async_trait]
impl AiProvider for LiteLLMProvider {
    async fn stream_chat(
        &self,
        _model: &str,
        _messages: &[ChatMessage],
        _system_prompt: &str,
    ) -> Result<(String, u64, u64), String> {
        // Placeholder implementation for initial integration.
        // Real implementation will perform an HTTP request to the LiteLLM
        // endpoint and stream tokens back via an async channel.
        Err("LiteLLMProvider: not implemented".into())
    }

    fn name(&self) -> &str {
        "litellm"
    }

    fn supports_tools(&self) -> bool {
        // Local models may or may not support tool-calls depending on deployment.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lite_llm_provider_basics() {
        let p = LiteLLMProvider::new("http://localhost:4000".into(), None);
        assert_eq!(p.name(), "litellm");
        let res = p.stream_chat("gpt-like", &[], "").await;
        assert!(res.is_err());
    }
}
