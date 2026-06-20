use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

use super::ai::ChatMessage;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum SubAgentRole {
    CodeReviewer,
    Debugger,
    Tester,
    Refactorer,
    Architect,
    Explainer,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SubAgent {
    pub id: String,
    pub name: String,
    pub role: SubAgentRole,
    pub agent_id: String,
    pub system_prompt: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DelegationTask {
    pub id: String,
    pub role: SubAgentRole,
    pub prompt: String,
    pub context: Vec<ChatMessage>,
    pub result: Option<String>,
    pub status: DelegateStatus,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum DelegateStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DelegationResult {
    pub task_id: String,
    pub role: String,
    pub output: String,
    pub success: bool,
}

#[derive(Clone)]
pub struct AgentOrchestrator {
    pub sub_agents: Arc<Mutex<Vec<SubAgent>>>,
    pub tasks: Arc<Mutex<Vec<DelegationTask>>>,
}

impl AgentOrchestrator {
    pub fn new() -> Self {
        let sub_agents = vec![
            SubAgent {
                id: "review-specialist".into(),
                name: "Review Specialist".into(),
                role: SubAgentRole::CodeReviewer,
                agent_id: String::new(), // auto-resolved in routing
                system_prompt: "You are a code review specialist. Analyze code for bugs, style issues, security vulnerabilities, and performance problems. Provide clear, actionable feedback.".into(),
            },
            SubAgent {
                id: "refactor-specialist".into(),
                name: "Refactor Specialist".into(),
                role: SubAgentRole::Refactorer,
                agent_id: String::new(),
                system_prompt: "You are a code refactoring specialist. Optimize code structure, improve readability, eliminate duplication, and apply design patterns while maintaining functionality.".into(),
            },
            SubAgent {
                id: "test-specialist".into(),
                name: "Test Specialist".into(),
                role: SubAgentRole::Tester,
                agent_id: String::new(),
                system_prompt: "You are a testing specialist. Generate comprehensive tests covering edge cases, error paths, and happy paths. Use the project's existing test framework.".into(),
            },
            SubAgent {
                id: "debug-specialist".into(),
                name: "Debug Specialist".into(),
                role: SubAgentRole::Debugger,
                agent_id: String::new(),
                system_prompt: "You are a debugging specialist. Analyze errors, trace execution paths, and identify root causes. Provide step-by-step debugging guidance.".into(),
            },
        ];

        Self {
            sub_agents: Arc::new(Mutex::new(sub_agents)),
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_sub_agents(&self) -> Result<Vec<SubAgent>, String> {
        let guard = self.sub_agents.lock().map_err(|e| format!("Lock: {}", e))?;
        Ok(guard.clone())
    }

    pub fn add_sub_agent(&self, agent: SubAgent) -> Result<(), String> {
        let mut guard = self.sub_agents.lock().map_err(|e| format!("Lock: {}", e))?;
        guard.push(agent);
        Ok(())
    }

    pub fn remove_sub_agent(&self, id: &str) -> Result<(), String> {
        let mut guard = self.sub_agents.lock().map_err(|e| format!("Lock: {}", e))?;
        guard.retain(|a| a.id != id);
        Ok(())
    }

    /// Delegate a task to the best sub-agent for the given role.
    pub fn delegate(&self, role: SubAgentRole, prompt: &str, _context: Vec<ChatMessage>) -> Result<String, String> {
        let agents = self.sub_agents.lock().map_err(|e| format!("Lock: {}", e))?;
        let _agent = agents.iter().find(|a| a.role == role)
            .ok_or_else(|| format!("No sub-agent found for role {:?}", role))?;

        let task_id = format!("task-{}", uuid::Uuid::new_v4());
        let task = DelegationTask {
            id: task_id.clone(),
            role,
            prompt: prompt.to_string(),
            context: _context,
            result: None,
            status: DelegateStatus::Pending,
        };

        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.push(task);
        }

        Ok(task_id)
    }

    pub fn get_tasks(&self) -> Result<Vec<DelegationTask>, String> {
        let guard = self.tasks.lock().map_err(|e| format!("Lock: {}", e))?;
        Ok(guard.clone())
    }

    pub fn update_task_result(&self, task_id: &str, result: &str, success: bool) -> Result<(), String> {
        let mut guard = self.tasks.lock().map_err(|e| format!("Lock: {}", e))?;
        if let Some(task) = guard.iter_mut().find(|t| t.id == task_id) {
            task.result = Some(result.to_string());
            task.status = if success { DelegateStatus::Completed } else { DelegateStatus::Failed(result.to_string()) };
        }
        Ok(())
    }

    /// Merge multiple delegation results into a cohesive output.
    pub fn merge_results(results: &[DelegationResult]) -> String {
        let mut out = String::new();
        for r in results {
            out.push_str(&format!("\n## {} ({})\n", r.role, if r.success { "OK" } else { "FAILED" }));
            out.push_str(&r.output);
            out.push('\n');
        }
        out
    }
}

impl Default for AgentOrchestrator {
    fn default() -> Self { Self::new() }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RouteDelegationResult {
    pub task_id: String,
    pub role: String,
}

#[tauri::command]
pub fn orch_get_agents(
    state: tauri::State<'_, AgentOrchestrator>,
) -> Result<Vec<SubAgent>, String> {
    state.get_sub_agents()
}

#[tauri::command]
pub fn orch_add_agent(
    state: tauri::State<'_, AgentOrchestrator>,
    agent: SubAgent,
) -> Result<(), String> {
    state.add_sub_agent(agent)
}

#[tauri::command]
pub fn orch_remove_agent(
    state: tauri::State<'_, AgentOrchestrator>,
    id: String,
) -> Result<(), String> {
    state.remove_sub_agent(&id)
}

#[tauri::command]
pub fn orch_get_tasks(
    state: tauri::State<'_, AgentOrchestrator>,
) -> Result<Vec<DelegationTask>, String> {
    state.get_tasks()
}

pub fn delegate_and_run(
    app: &tauri::AppHandle,
    state: &AgentOrchestrator,
    ai_state: &super::ai::AiManager,
    role: SubAgentRole,
    prompt: String,
    messages: Vec<super::ai::ChatMessage>,
) -> Result<String, String> {
    use tauri::Manager;
    let task_id = state.delegate(role.clone(), &prompt, messages.clone())?;
    let task_id_clone = task_id.clone();
    
    let app_clone = app.clone();
    let state_clone = state.clone();
    let ai_state_clone = ai_state.clone();
    
    tokio::spawn(async move {
        // Resolve agent configuration to run the sub-agent
        let sub_agent = {
            let sub_agents = state_clone.sub_agents.lock().unwrap();
            match sub_agents.iter().find(|a| a.role == role).cloned() {
                Some(sa) => sa,
                None => {
                    let err_msg = format!("No sub-agent configured for role {:?}", role);
                    let _ = state_clone.update_task_result(&task_id_clone, &err_msg, false);
                    let _ = app_clone.emit("ai:error", super::ai::AiStreamError { error: err_msg });
                    return;
                }
            }
        };

        // Update task status to Running
        {
            let mut guard = state_clone.tasks.lock().unwrap();
            if let Some(t) = guard.iter_mut().find(|task| task.id == task_id_clone) {
                t.status = DelegateStatus::Running;
            }
        }

        // Get default coder agent as template
        let default_agent = match ai_state_clone.get_agent("coder") {
            Some(a) => a,
            None => {
                let err_msg = "Coder agent template not found".to_string();
                let _ = state_clone.update_task_result(&task_id_clone, &err_msg, false);
                let _ = app_clone.emit("ai:error", super::ai::AiStreamError { error: err_msg });
                return;
            }
        };

        let mut sub_agent_config = if !sub_agent.agent_id.is_empty() {
            ai_state_clone.get_agent(&sub_agent.agent_id).unwrap_or(default_agent)
        } else {
            default_agent
        };

        // Select model based on role requirements if agent_id is default/empty
        if sub_agent.agent_id.is_empty() {
            let mut registry = crate::modules::routing::model_registry::ModelRegistry::load(None::<&std::path::Path>);
            let active_agents = ai_state_clone.list_agents();
            let active_providers: Vec<String> = active_agents.iter().map(|a| a.provider.clone()).collect();
            registry.models.retain(|m| active_providers.contains(&m.provider));

            let spec = match role {
                SubAgentRole::CodeReviewer => crate::modules::routing::model_registry::Spec::Review,
                SubAgentRole::Refactorer => crate::modules::routing::model_registry::Spec::Code,
                _ => crate::modules::routing::model_registry::Spec::Chat,
            };
            if let Some(meta) = registry.select_model(crate::modules::routing::model_registry::ReasoningTier::High, spec, 0) {
                sub_agent_config.provider = meta.provider.clone();
                sub_agent_config.model = meta.id.clone();
            }
        }

        // Ensure API Key is resolved
        let secrets_state = app_clone.state::<super::secrets::SecretsState>();
        if let Ok(Some(real_key)) = crate::modules::secrets::get_secret(&app_clone, &secrets_state, "codlib-ai", &sub_agent_config.id) {
            sub_agent_config.api_key = Some(real_key);
        }

        let base_url = sub_agent_config.base_url.clone().unwrap_or_else(|| {
            super::ai::default_base_url(&sub_agent_config.provider).to_string()
        });

        let mut msgs = vec![
            super::ai::ChatMessage {
                role: "system".to_string(),
                content: sub_agent.system_prompt.clone(),
                display_content: None,
            }
        ];
        msgs.extend(messages);

        let _ = app_clone.emit("ai:route_progress", format!("Executing delegated sub-agent '{}'...", sub_agent.name));

        // Call stream_openai
        match super::ai::stream_openai(&app_clone, &sub_agent_config, &msgs, &base_url).await {
            Ok((content, input_tokens, output_tokens)) => {
                let _ = state_clone.update_task_result(&task_id_clone, &content, true);
                
                // Record usage
                let (price_in, price_out) = super::ai::model_price(&sub_agent_config.model);
                let cost = (input_tokens as f64 * price_in + output_tokens as f64 * price_out) / 1000.0;
                ai_state_clone.record_usage(&sub_agent_config.id, input_tokens, output_tokens, cost);
                if let Some(ps) = app_clone.try_state::<crate::modules::routing::provider_stats::ProviderStats>() {
                    ps.record_success(&sub_agent_config.provider, input_tokens + output_tokens, cost, 0);
                }

                let _ = app_clone.emit("ai:done", super::ai::AiStreamDone {
                    content,
                    provider: sub_agent_config.provider.clone(),
                    model: sub_agent_config.model.clone(),
                    input_tokens,
                    output_tokens,
                    cost,
                });
            }
            Err(err) => {
                let _ = state_clone.update_task_result(&task_id_clone, &err, false);
                if let Some(ps) = app_clone.try_state::<crate::modules::routing::provider_stats::ProviderStats>() {
                    ps.record_failure(&sub_agent_config.provider, &err);
                }
                let _ = app_clone.emit("ai:error", super::ai::AiStreamError { error: err });
            }
        }
    });

    Ok(task_id)
}

#[tauri::command]
pub async fn orch_delegate(
    app: tauri::AppHandle,
    state: tauri::State<'_, AgentOrchestrator>,
    ai_state: tauri::State<'_, super::ai::AiManager>,
    role: SubAgentRole,
    prompt: String,
    messages: Vec<super::ai::ChatMessage>,
) -> Result<String, String> {
    delegate_and_run(&app, &state, &ai_state, role, prompt, messages)
}
