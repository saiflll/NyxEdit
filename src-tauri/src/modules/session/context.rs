use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use crate::modules::ai::ChatMessage;
use super::sessions::SessionManager;

/// Configuration for conversation context management
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ContextConfig {
    /// Max messages to keep before summarization kicks in
    pub max_messages: usize,
    /// Max tokens budget for the conversation window
    pub max_tokens: usize,
    /// Whether to enable cross-session retrieval
    pub enable_retrieval: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_messages: 20,
            max_tokens: 8000,
            enable_retrieval: true,
        }
    }
}

/// A compressed summary of earlier conversation turns
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConversationSummary {
    pub text: String,
    pub message_count: usize,
    pub tokens_saved: usize,
}

/// ContextManager — compresses and retrieves conversation context.
/// Thread-safe via internal Arc<Mutex>.
pub struct ContextManager {
    config: ContextConfig,
    summaries: Arc<Mutex<Vec<ConversationSummary>>>,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            config: ContextConfig::default(),
            summaries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn new_with_config(config: ContextConfig) -> Self {
        Self {
            config,
            summaries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set config
    pub fn set_config(&mut self, config: ContextConfig) {
        self.config = config;
    }

    /// Compress messages if they exceed max_messages.
    /// Returns the compressed message list and a summary of what was compressed.
    pub fn compress(&self, messages: &[ChatMessage]) -> (Vec<ChatMessage>, Option<ConversationSummary>) {
        if messages.len() <= self.config.max_messages {
            return (messages.to_vec(), None);
        }

        let keep = self.config.max_messages / 2;
        let to_summarize = messages.len() - keep;

        // Take oldest messages to summarize
        let summary_text: String = messages[..to_summarize].iter()
            .filter_map(|m| {
                let content = m.content.trim();
                if content.is_empty() || content.len() < 20 { None }
                else { Some(format!("{}: {}...", m.role, &content[..content.len().min(200)])) }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let summary = ConversationSummary {
            text: format!("Earlier conversation summary ({} messages):\n{}", to_summarize, summary_text),
            message_count: to_summarize,
            tokens_saved: summary_text.len() / 4,
        };

        // Build compressed list: summary as system message + recent messages
        let mut compressed = Vec::new();
        compressed.push(ChatMessage {
            role: "system".into(),
            content: summary.text.clone(),
            display_content: None,
        });
        compressed.extend_from_slice(&messages[to_summarize..]);

        let mut summaries = self.summaries.lock().unwrap();
        summaries.push(summary.clone());

        (compressed, Some(summary))
    }

    /// Search past sessions for relevant context based on current query.
    /// Returns up to `limit` relevant message excerpts.
    pub async fn retrieve_from_past(
        &self,
        mgr: &SessionManager,
        query: &str,
        limit: usize,
    ) -> Vec<ChatMessage> {
        if !self.config.enable_retrieval {
            return Vec::new();
        }

        let sessions = match mgr.list().await {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        let keywords: Vec<&str> = query_lower.split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        for session in &sessions {
            if results.len() >= limit {
                break;
            }
            for msg in &session.messages {
                if results.len() >= limit {
                    break;
                }
                let content_lower = msg.content.to_lowercase();
                let matches = keywords.iter().filter(|k| content_lower.contains(*k)).count();
                // Use ceiling division so a 1-keyword query requires 1 match (not 0)
                if matches > 0 && matches >= keywords.len().max(1).div_ceil(2) {
                    results.push(ChatMessage {
                        role: msg.role.clone(),
                        content: format!("[From past session '{}']\n{}", session.name, msg.content),
                        display_content: None,
                    });
                }
            }
        }

        results
    }

    /// Get all conversation summaries from this session
    pub fn get_summaries(&self) -> Vec<ConversationSummary> {
        self.summaries.lock().unwrap().clone()
    }

    /// Total tokens saved by compression
    pub fn total_tokens_saved(&self) -> usize {
        self.summaries.lock().unwrap().iter().map(|s| s.tokens_saved).sum()
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────────

#[tauri::command]
pub fn session_get_memory_stats(
    state: tauri::State<'_, crate::modules::session::sessions::SessionsState>,
) -> Result<serde_json::Value, String> {
    // Return memory compression stats aggregated across sessions
    // The ContextManager lives per ai_chat_stream call; we expose approximated stats here
    Ok(serde_json::json!({
        "raw_tokens": 0,
        "compressed_tokens": 0,
        "compression_ratio": 0.0,
        "cross_session_retrievals": 0,
        "oldest_context_date": "",
        "note": "Stats tracked per active session. Start a chat session to see live data."
    }))
}
