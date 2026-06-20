use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const DEFAULT_MODELS_TOML: &str = include_str!("../../../models.toml");

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReasoningTier {
    UltraHigh,
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum Spec {
    Code,
    Scan,
    Chat,
    Test,
    Review,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_window_limit: u32,      // token count
    pub reasoning_tier: ReasoningTier,  // UltraHigh | High | Medium | Low
    pub specialization: Vec<Spec>,      // [Code, Scan, Chat, Test, Review]
    pub cost_per_1k_tokens: f64,        // USD
    pub avg_latency_p95_ms: u32,        // P95 latency
    pub max_parallel_calls: u8,
    pub supports_streaming: bool,
    pub supports_tool_use: bool,
    pub fallback_priority: u8,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelRegistry {
    pub models: Vec<ModelMetadata>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self { models: Vec::new() }
    }

    pub fn load_default() -> Self {
        let models = vec![
            // Wide-Context Scanners
            ModelMetadata {
                id: "gemini-2.0-flash".into(),
                name: "Gemini 2.0 Flash".into(),
                provider: "gemini".into(),
                context_window_limit: 1048576, // 1M+
                reasoning_tier: ReasoningTier::Medium,
                specialization: vec![Spec::Scan, Spec::Chat],
                cost_per_1k_tokens: 0.000075,
                avg_latency_p95_ms: 2000,
                max_parallel_calls: 5,
                supports_streaming: true,
                supports_tool_use: true,
                fallback_priority: 1,
            },
            // Reasoning Engines
            ModelMetadata {
                id: "deepseek-r1".into(),
                name: "DeepSeek R1".into(),
                provider: "openrouter".into(),
                context_window_limit: 163840,
                reasoning_tier: ReasoningTier::UltraHigh,
                specialization: vec![Spec::Code, Spec::Review],
                cost_per_1k_tokens: 0.002,
                avg_latency_p95_ms: 15000, // Reasoning takes time
                max_parallel_calls: 2,
                supports_streaming: true,
                supports_tool_use: false, // R1 sometimes struggles with direct tool calling format
                fallback_priority: 1,
            },
            // Execution Coders
            ModelMetadata {
                id: "qwen/qwen-2.5-coder-32b-instruct".into(),
                name: "Qwen 2.5 Coder 32B".into(),
                provider: "openrouter".into(),
                context_window_limit: 32768,
                reasoning_tier: ReasoningTier::High,
                specialization: vec![Spec::Code, Spec::Test],
                cost_per_1k_tokens: 0.0003,
                avg_latency_p95_ms: 3000,
                max_parallel_calls: 5,
                supports_streaming: true,
                supports_tool_use: true,
                fallback_priority: 1,
            },
            // Simple Helpers
            ModelMetadata {
                id: "meta-llama/llama-3-8b-instruct".into(),
                name: "Llama 3 8B".into(),
                provider: "openrouter".into(),
                context_window_limit: 8192,
                reasoning_tier: ReasoningTier::Low,
                specialization: vec![Spec::Chat],
                cost_per_1k_tokens: 0.00005,
                avg_latency_p95_ms: 1000,
                max_parallel_calls: 10,
                supports_streaming: true,
                supports_tool_use: true,
                fallback_priority: 1,
            },
        ];

        Self { models }
    }

    /// Load from TOML file. Checks, in order:
    /// 1. Explicit path (if provided)
    /// 2. `{app_data_dir}/contlib/models.toml` (auto-created on first run)
    /// 3. Compiled-in TOML (`models.toml`)
    /// 4. Hardcoded defaults (if all else fails)
    pub fn load<P: AsRef<Path>>(path: Option<P>) -> Self {
        // 1. Try explicit path if provided
        if let Some(path) = path {
            if path.as_ref().exists() {
                if let Ok(registry) = Self::load_from_toml(path.as_ref()) {
                    return registry;
                }
            }
        }
        // 2. Try app data dir external config
        if let Some(data_dir) = dirs::data_dir() {
            let config_path = data_dir.join("contlib").join("models.toml");
            if config_path.exists() {
                if let Ok(registry) = Self::load_from_toml(&config_path) {
                    return registry;
                }
            } else {
                // Create default external config file on first run
                if let Some(parent) = config_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(&config_path, DEFAULT_MODELS_TOML);
            }
        }
        // 3. Try compiled-in TOML
        if let Ok(registry) = toml::from_str::<ModelRegistry>(DEFAULT_MODELS_TOML) {
            return registry;
        }
        // 4. Ultimate fallback: hardcoded defaults
        Self::load_default()
    }

    pub fn load_from_toml<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    }

    /// Select the best model based on reasoning tier, specialization, and context size requirements
    pub fn select_model(&self, tier: ReasoningTier, spec: Spec, context_size: u32) -> Option<&ModelMetadata> {
        // Try exact match first
        if let Some(m) = self.find_model_for_tier(tier, spec, context_size) {
            return Some(m);
        }

        // Fallback: search other tiers in order of preference
        let search_order = match tier {
            ReasoningTier::UltraHigh => vec![ReasoningTier::High, ReasoningTier::Medium, ReasoningTier::Low],
            ReasoningTier::High => vec![ReasoningTier::UltraHigh, ReasoningTier::Medium, ReasoningTier::Low],
            ReasoningTier::Medium => vec![ReasoningTier::High, ReasoningTier::UltraHigh, ReasoningTier::Low],
            ReasoningTier::Low => vec![ReasoningTier::Medium, ReasoningTier::High, ReasoningTier::UltraHigh],
        };

        for fallback_tier in search_order {
            if let Some(m) = self.find_model_for_tier(fallback_tier, spec, context_size) {
                return Some(m);
            }
        }

        None
    }

    fn find_model_for_tier(&self, tier: ReasoningTier, spec: Spec, context_size: u32) -> Option<&ModelMetadata> {
        self.models.iter()
            .filter(|m| m.reasoning_tier == tier && m.specialization.contains(&spec) && m.context_window_limit >= context_size)
            .min_by(|a, b| {
                // Cost-aware routing: choose cheaper first, then fallback_priority
                a.cost_per_1k_tokens.partial_cmp(&b.cost_per_1k_tokens)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.fallback_priority.cmp(&b.fallback_priority))
            })
    }
}
