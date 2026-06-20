use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProviderStatEntry {
    pub provider: String,
    pub total_requests: u64,
    pub failed_requests: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub avg_latency_ms: f64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
struct ProviderMetrics {
    success_count: u64,
    failure_count: u64,
    consecutive_failures: u64,
    total_tokens: u64,
    total_cost: f64,
    latencies: Vec<u64>,
    last_error: Option<String>,
    circuit_open_until: Option<Instant>,
}

pub struct ProviderStats {
    data: Arc<Mutex<HashMap<String, ProviderMetrics>>>,
}

const CIRCUIT_BREAK_THRESHOLD: u64 = 3;
const CIRCUIT_COOLDOWN_SECS: u64 = 60;
const MAX_LATENCY_SAMPLES: usize = 20;

impl ProviderStats {
    pub fn new() -> Self {
        Self { data: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn record_success(&self, provider: &str, tokens: u64, cost: f64, latency_ms: u64) {
        if let Ok(mut map) = self.data.lock() {
            let m = map.entry(provider.to_string()).or_insert_with(|| ProviderMetrics {
                success_count: 0, failure_count: 0, consecutive_failures: 0,
                total_tokens: 0, total_cost: 0.0, latencies: Vec::new(),
                last_error: None, circuit_open_until: None,
            });
            m.success_count += 1;
            m.consecutive_failures = 0;
            m.total_tokens += tokens;
            m.total_cost += cost;
            m.latencies.push(latency_ms);
            if m.latencies.len() > MAX_LATENCY_SAMPLES {
                m.latencies.remove(0);
            }
            // Re-enable circuit if previously open
            m.circuit_open_until = None;
        }
    }

    pub fn record_failure(&self, provider: &str, error: &str) {
        if let Ok(mut map) = self.data.lock() {
            let m = map.entry(provider.to_string()).or_insert_with(|| ProviderMetrics {
                success_count: 0, failure_count: 0, consecutive_failures: 0,
                total_tokens: 0, total_cost: 0.0, latencies: Vec::new(),
                last_error: None, circuit_open_until: None,
            });
            m.failure_count += 1;
            m.consecutive_failures += 1;
            m.last_error = Some(error.to_string());

            if m.consecutive_failures >= CIRCUIT_BREAK_THRESHOLD {
                m.circuit_open_until = Some(Instant::now() + Duration::from_secs(CIRCUIT_COOLDOWN_SECS));
            }
        }
    }

    /// Check if a provider is available (circuit not open)
    pub fn is_available(&self, provider: &str) -> bool {
        if let Ok(map) = self.data.lock() {
            if let Some(m) = map.get(provider) {
                if let Some(until) = m.circuit_open_until {
                    if Instant::now() < until {
                        return false; // Circuit is open
                    }
                }
            }
        }
        true
    }

    /// Get stats for all providers
    pub fn get_all_stats(&self) -> Result<Vec<ProviderStatEntry>, String> {
        let map = self.data.lock().map_err(|e| format!("Lock: {}", e))?;
        Ok(map.iter().map(|(provider, m)| {
            let avg_lat = if m.latencies.is_empty() { 0.0 }
                else { m.latencies.iter().sum::<u64>() as f64 / m.latencies.len() as f64 };
            ProviderStatEntry {
                provider: provider.clone(),
                total_requests: m.success_count + m.failure_count,
                failed_requests: m.failure_count,
                total_tokens: m.total_tokens,
                total_cost: m.total_cost,
                avg_latency_ms: avg_lat,
                last_error: m.last_error.clone(),
            }
        }).collect())
    }

    /// Reset stats for all providers
    pub fn reset(&self) {
        if let Ok(mut map) = self.data.lock() {
            map.clear();
        }
    }
}

impl Default for ProviderStats {
    fn default() -> Self { Self::new() }
}

#[tauri::command]
pub fn provider_get_stats(
    state: tauri::State<'_, ProviderStats>,
) -> Result<Vec<ProviderStatEntry>, String> {
    state.get_all_stats()
}

#[tauri::command]
pub fn provider_reset_stats(
    state: tauri::State<'_, ProviderStats>,
) -> Result<(), String> {
    state.reset();
    Ok(())
}
