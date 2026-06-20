use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::modules::routing::model_registry::{ModelMetadata, ModelRegistry, ReasoningTier, Spec};

/// Cost-aware model recommendation
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CostRecommendation {
    pub model_id: String,
    pub provider: String,
    pub estimated_cost_usd: f64,
    pub tier: String,
    pub confidence: f64,
    pub reason: String,
}

/// Per-session cost tracking
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SessionCost {
    pub session_id: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost: f64,
    pub model_used: String,
    pub provider_used: String,
}

/// CostBudget — enforces spending limits
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CostBudget {
    pub max_per_session: f64,
    pub max_per_day: f64,
    pub current_session_cost: f64,
    pub current_day_cost: f64,
}

impl Default for CostBudget {
    fn default() -> Self {
        Self {
            max_per_session: 0.10,
            max_per_day: 1.00,
            current_session_cost: 0.0,
            current_day_cost: 0.0,
        }
    }
}

/// CostRouter — selects the most cost-effective model for a task.
pub struct CostRouter {
    budget: Arc<Mutex<CostBudget>>,
    session_costs: Arc<Mutex<Vec<SessionCost>>>,
    /// Cache: model_id -> avg cost per 1K tokens
    avg_costs: Arc<Mutex<HashMap<String, f64>>>,
}

impl CostRouter {
    pub fn new() -> Self {
        Self {
            budget: Arc::new(Mutex::new(CostBudget::default())),
            session_costs: Arc::new(Mutex::new(Vec::new())),
            avg_costs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Select the cheapest model that meets the required tier and spec.
    /// Falls back to the primary model if no cheaper alternative is acceptable.
    pub fn cheapest_for_tier(
        &self,
        registry: &ModelRegistry,
        tier: ReasoningTier,
        spec: Spec,
        primary_model: &str,
    ) -> CostRecommendation {
        // Score models: filter by tier + spec, sorted by cost
        let mut scored: Vec<&ModelMetadata> = registry.models.iter()
            .filter(|m| m.reasoning_tier == tier && m.specialization.contains(&spec))
            .collect();

        scored.sort_by(|a, b| {
            a.cost_per_1k_tokens.partial_cmp(&b.cost_per_1k_tokens)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.fallback_priority.cmp(&b.fallback_priority))
        });

        if let Some(best) = scored.first() {
            let estimator = if best.cost_per_1k_tokens > 0.0 {
                best.cost_per_1k_tokens / 1000.0
            } else {
                0.000015
            };

            CostRecommendation {
                model_id: best.id.clone(),
                provider: best.provider.clone(),
                estimated_cost_usd: estimator * 500.0,
                tier: format!("{:?}", tier),
                confidence: if best.id == primary_model { 0.95 } else { 0.75 },
                reason: format!("Cheapest for {:?}/{:?} (${:.6}/tok)", tier, spec, estimator),
            }
        } else {
            CostRecommendation {
                model_id: primary_model.to_string(),
                provider: String::new(),
                estimated_cost_usd: 0.0,
                tier: format!("{:?}", tier),
                confidence: 0.5,
                reason: "No matching model, using primary".into(),
            }
        }
    }

    /// Record a model invocation for cost tracking
    pub fn record_usage(&self, session_id: &str, model: &str, provider: &str, input_tokens: u64, output_tokens: u64, cost: f64) {
        let mut costs = self.session_costs.lock().unwrap();
        costs.push(SessionCost {
            session_id: session_id.to_string(),
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            total_cost: cost,
            model_used: model.to_string(),
            provider_used: provider.to_string(),
        });

        let mut budget = self.budget.lock().unwrap();
        budget.current_session_cost += cost;
        budget.current_day_cost += cost;

        // Update average cost for this model
        let mut avg = self.avg_costs.lock().unwrap();
        let entry = avg.entry(model.to_string()).or_insert(0.0);
        *entry = (*entry * 0.9) + (cost * 0.1); // exponential moving average
    }

    /// Check if the current session is within budget
    pub fn within_budget(&self) -> bool {
        let budget = self.budget.lock().unwrap();
        budget.current_session_cost < budget.max_per_session
            && budget.current_day_cost < budget.max_per_day
    }

    /// Check if using a specific model would exceed budget
    pub fn would_exceed_budget(&self, estimated_cost: f64) -> bool {
        let budget = self.budget.lock().unwrap();
        (budget.current_session_cost + estimated_cost) > budget.max_per_session
            || (budget.current_day_cost + estimated_cost) > budget.max_per_day
    }

    /// Get cost summary for the frontend
    pub fn get_cost_summary(&self) -> serde_json::Value {
        let costs = self.session_costs.lock().unwrap();
        let budget = self.budget.lock().unwrap();
        serde_json::json!({
            "budget": {
                "max_per_session": budget.max_per_session,
                "max_per_day": budget.max_per_day,
                "current_session": budget.current_session_cost,
                "current_day": budget.current_day_cost,
            },
            "total_calls": costs.len(),
            "total_cost": costs.iter().map(|c| c.total_cost).sum::<f64>(),
            "avg_costs": *self.avg_costs.lock().unwrap(),
        })
    }

    /// Set budget limits
    pub fn set_budget(&self, max_per_session: f64, max_per_day: f64) {
        let mut budget = self.budget.lock().unwrap();
        budget.max_per_session = max_per_session;
        budget.max_per_day = max_per_day;
    }

    /// Reset session cost counter
    pub fn reset_session(&self) {
        let mut budget = self.budget.lock().unwrap();
        budget.current_session_cost = 0.0;
    }

    /// Get spending breakdown per model
    pub fn get_breakdown(&self) -> serde_json::Value {
        let costs = self.session_costs.lock().unwrap();
        let budget = self.budget.lock().unwrap();

        let mut by_model: HashMap<String, (u64, f64)> = HashMap::new();
        let total_cost_sum: f64 = costs.iter().map(|c| c.total_cost).sum();

        for sc in costs.iter() {
            let entry = by_model.entry(sc.model_used.clone()).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += sc.total_cost;
        }

        let by_model_json: serde_json::Value = by_model.iter()
            .map(|(model, (calls, cost))| {
                let percent = if total_cost_sum > 0.0 { cost / total_cost_sum * 100.0 } else { 0.0 };
                serde_json::json!({
                    "model": model,
                    "calls": calls,
                    "cost": cost,
                    "percent": percent
                })
            })
            .collect::<Vec<_>>()
            .into();

        serde_json::json!({
            "by_model": by_model_json,
            "total_cost": total_cost_sum,
            "total_calls": costs.len(),
            "budget_limit": budget.max_per_day,
            "budget_used_percent": if budget.max_per_day > 0.0 { budget.current_day_cost / budget.max_per_day * 100.0 } else { 0.0 },
            "recommendations_ignored": 0u64,
            "budget_alerts_triggered": if budget.current_day_cost >= budget.max_per_day * 0.8 { 1u64 } else { 0u64 }
        })
    }

    /// Get recent recommendation log entries
    pub fn get_recommendation_log(&self) -> Vec<serde_json::Value> {
        let costs = self.session_costs.lock().unwrap();
        costs.iter().rev().take(20).map(|sc| {
            serde_json::json!({
                "session_id": sc.session_id,
                "model": sc.model_used,
                "provider": sc.provider_used,
                "input_tokens": sc.total_input_tokens,
                "output_tokens": sc.total_output_tokens,
                "cost": sc.total_cost,
            })
        }).collect()
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────────

#[tauri::command]
pub fn cost_get_summary(state: tauri::State<'_, CostRouter>) -> Result<serde_json::Value, String> {
    Ok(state.get_cost_summary())
}

#[tauri::command]
pub fn cost_set_budget(
    state: tauri::State<'_, CostRouter>,
    max_per_session: f64,
    max_per_day: f64,
) -> Result<(), String> {
    state.set_budget(max_per_session, max_per_day);
    Ok(())
}

#[tauri::command]
pub fn cost_recommend(
    state: tauri::State<'_, CostRouter>,
    registry: tauri::State<'_, ModelRegistry>,
    tier: String,
    spec: String,
    primary_model: String,
) -> Result<CostRecommendation, String> {
    let rt: ReasoningTier = serde_json::from_str(&format!("\"{}\"", tier))
        .map_err(|e| format!("Invalid tier: {}", e))?;
    let sp: Spec = serde_json::from_str(&format!("\"{}\"", spec))
        .map_err(|e| format!("Invalid spec: {}", e))?;
    Ok(state.cheapest_for_tier(&registry, rt, sp, &primary_model))
}

#[tauri::command]
pub fn cost_get_breakdown(state: tauri::State<'_, CostRouter>) -> Result<serde_json::Value, String> {
    Ok(state.get_breakdown())
}

#[tauri::command]
pub fn cost_get_recommendation_log(state: tauri::State<'_, CostRouter>) -> Result<Vec<serde_json::Value>, String> {
    Ok(state.get_recommendation_log())
}
