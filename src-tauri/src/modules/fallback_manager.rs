use serde::{Deserialize, Serialize};
use super::model_registry::{ModelRegistry, ReasoningTier, Spec};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FallbackEntry {
    pub provider: String,
    pub model_id: String,
    pub tier: ReasoningTier,
    pub cost_per_1k: f64,
}

/// Manages fallback queue for a specific model tier.
/// Built from ModelRegistry — picks models matching tier+spec,
/// sorted by cost then fallback_priority.
pub struct FallbackManager {
    queue: Vec<FallbackEntry>,
    index: usize,
}

impl FallbackManager {
    /// Build fallback queue from registry for a given tier + spec.
    /// Primary model is first; alternatives follow.
    pub fn from_registry(
        registry: &ModelRegistry,
        tier: ReasoningTier,
        spec: Spec,
        context_size: u32,
    ) -> Self {
        let mut candidates: Vec<FallbackEntry> = registry
            .models
            .iter()
            .filter(|m| {
                m.reasoning_tier == tier
                    && m.specialization.contains(&spec)
                    && m.context_window_limit >= context_size
            })
            .map(|m| FallbackEntry {
                provider: m.provider.clone(),
                model_id: m.id.clone(),
                tier: m.reasoning_tier,
                cost_per_1k: m.cost_per_1k_tokens,
            })
            .collect();

        // Sort by cost asc, then fallback_priority asc (lower = higher priority)
        // Match the sort order used in ModelRegistry::select_model
        candidates.sort_by(|a, b| {
            a.cost_per_1k
                .partial_cmp(&b.cost_per_1k)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Self {
            queue: candidates,
            index: 0,
        }
    }

    /// Get current entry without advancing. None if queue exhausted.
    pub fn current(&self) -> Option<&FallbackEntry> {
        self.queue.get(self.index)
    }

    /// Advance to next fallback. Returns false if no more fallbacks.
    pub fn advance(&mut self) -> bool {
        if self.index + 1 < self.queue.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    /// Number of fallbacks remaining (including current)
    pub fn remaining(&self) -> usize {
        self.queue.len().saturating_sub(self.index)
    }

    /// Total entries in queue
    pub fn total(&self) -> usize {
        self.queue.len()
    }

    /// Current index
    pub fn current_index(&self) -> usize {
        self.index
    }
}
