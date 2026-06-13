use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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
