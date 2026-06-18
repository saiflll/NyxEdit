use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use super::model_registry::{ReasoningTier, Spec};
use super::routing_engine::{Intent, OutputType, RouteDecision};

/// Step type in a chain
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum StepType {
    WideScan,
    Reasoning,
    Coding,
}

/// A single node in the execution chain
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChainNode {
    pub id: String,
    pub step_type: StepType,
    pub model_tier: ReasoningTier,
    pub spec: Spec,
    pub label: String,
    /// How to inject previous step output into this step's context
    pub inject_as: InjectRole,
}

/// Where to inject previous step output
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum InjectRole {
    /// Inject as system message (default)
    System,
    /// Inject as user message
    User,
}

/// A linear execution plan (multi-step chain)
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChainPlan {
    pub nodes: Vec<ChainNode>,
    pub user_prompt: String,
}

impl ChainPlan {
    /// Build a chain plan from a route decision.
    /// Returns None if the task can be handled in a single step (no chaining needed).
    pub fn from_decision(decision: &RouteDecision, user_prompt: &str) -> Option<Self> {
        match (&decision.intent, &decision.output_type) {
            (Intent::RefactorFull, OutputType::CodeDiff) => Some(Self {
                user_prompt: user_prompt.to_string(),
                nodes: vec![
                    ChainNode {
                        id: "scan".into(), step_type: StepType::WideScan, model_tier: ReasoningTier::Medium,
                        spec: Spec::Scan, label: "Scanning project files".into(), inject_as: InjectRole::System,
                    },
                    ChainNode {
                        id: "reason".into(), step_type: StepType::Reasoning, model_tier: ReasoningTier::UltraHigh,
                        spec: Spec::Review, label: "Analyzing & planning refactor".into(), inject_as: InjectRole::System,
                    },
                    ChainNode {
                        id: "code".into(), step_type: StepType::Coding, model_tier: ReasoningTier::High,
                        spec: Spec::Code, label: "Writing refactored code".into(), inject_as: InjectRole::System,
                    },
                ],
            }),
            (Intent::CodeReview, OutputType::Narrative) => Some(Self {
                user_prompt: user_prompt.to_string(),
                nodes: vec![
                    ChainNode {
                        id: "scan".into(), step_type: StepType::WideScan, model_tier: ReasoningTier::Medium,
                        spec: Spec::Scan, label: "Scanning files for review".into(), inject_as: InjectRole::System,
                    },
                    ChainNode {
                        id: "review".into(), step_type: StepType::Reasoning, model_tier: ReasoningTier::UltraHigh,
                        spec: Spec::Review, label: "Reviewing code quality".into(), inject_as: InjectRole::System,
                    },
                ],
            }),
            (Intent::DebugLogic, OutputType::CodeDiff) => Some(Self {
                user_prompt: user_prompt.to_string(),
                nodes: vec![
                    ChainNode {
                        id: "reason".into(), step_type: StepType::Reasoning, model_tier: ReasoningTier::UltraHigh,
                        spec: Spec::Scan, label: "Analyzing the bug".into(), inject_as: InjectRole::System,
                    },
                    ChainNode {
                        id: "code".into(), step_type: StepType::Coding, model_tier: ReasoningTier::High,
                        spec: Spec::Code, label: "Writing the fix".into(), inject_as: InjectRole::System,
                    },
                ],
            }),
            _ => None,
        }
    }

    pub fn len(&self) -> usize { self.nodes.len() }
}

// ─── DAG Orchestration ──────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DagNode {
    pub id: String,
    pub step_type: StepType,
    pub model_tier: ReasoningTier,
    pub spec: Spec,
    pub label: String,
    /// Node IDs this node depends on (must complete first)
    pub depends_on: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DagEdge {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DagPlan {
    pub nodes: Vec<DagNode>,
    pub user_prompt: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DagStepResult {
    pub node_id: String,
    pub output: String,
    pub success: bool,
    pub error: Option<String>,
    /// Real token counts from the LLM call (0 if failed or unavailable)
    pub inp_tokens: u64,
    pub out_tokens: u64,
}

impl DagPlan {
    /// Build a DAG plan from intent — parallel branches are created where possible.
    pub fn from_intent(intent: &Intent, user_prompt: &str) -> Option<Self> {
        match intent {
            Intent::RefactorFull => {
                // Parallel: scan + review → merge → code
                Some(Self {
                    user_prompt: user_prompt.to_string(),
                    nodes: vec![
                        DagNode {
                            id: "scan".into(), step_type: StepType::WideScan,
                            model_tier: ReasoningTier::Medium, spec: Spec::Scan,
                            label: "Scanning project structure".into(), depends_on: vec![],
                        },
                        DagNode {
                            id: "review".into(), step_type: StepType::Reasoning,
                            model_tier: ReasoningTier::UltraHigh, spec: Spec::Review,
                            label: "Reviewing code quality".into(), depends_on: vec!["scan".into()],
                        },
                        DagNode {
                            id: "arch".into(), step_type: StepType::Reasoning,
                            model_tier: ReasoningTier::UltraHigh, spec: Spec::Chat,
                            label: "Architectural analysis".into(), depends_on: vec!["scan".into()],
                        },
                        DagNode {
                            id: "code".into(), step_type: StepType::Coding,
                            model_tier: ReasoningTier::High, spec: Spec::Code,
                            label: "Writing refactored code".into(),
                            depends_on: vec!["review".into(), "arch".into()],
                        },
                    ],
                })
            }
            Intent::CodeReview => {
                // Parallel: scan (security + style + perf) in parallel → merged review
                Some(Self {
                    user_prompt: user_prompt.to_string(),
                    nodes: vec![
                        DagNode {
                            id: "scan".into(), step_type: StepType::WideScan,
                            model_tier: ReasoningTier::Medium, spec: Spec::Scan,
                            label: "Scanning project files".into(), depends_on: vec![],
                        },
                        DagNode {
                            id: "security".into(), step_type: StepType::Reasoning,
                            model_tier: ReasoningTier::UltraHigh, spec: Spec::Review,
                            label: "Security review".into(), depends_on: vec!["scan".into()],
                        },
                        DagNode {
                            id: "style".into(), step_type: StepType::Reasoning,
                            model_tier: ReasoningTier::High, spec: Spec::Review,
                            label: "Code style review".into(), depends_on: vec!["scan".into()],
                        },
                        DagNode {
                            id: "perf".into(), step_type: StepType::Reasoning,
                            model_tier: ReasoningTier::High, spec: Spec::Scan,
                            label: "Performance review".into(), depends_on: vec!["scan".into()],
                        },
                        DagNode {
                            id: "merge".into(), step_type: StepType::Reasoning,
                            model_tier: ReasoningTier::UltraHigh, spec: Spec::Chat,
                            label: "Merging review results".into(),
                            depends_on: vec!["security".into(), "style".into(), "perf".into()],
                        },
                    ],
                })
            }
            _ => None,
        }
    }

    /// Topologically sort nodes for execution order (BFS)
    pub fn execution_order(&self) -> Result<Vec<Vec<String>>, String> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut all_ids = HashSet::new();

        for node in &self.nodes {
            all_ids.insert(node.id.as_str());
            in_degree.entry(node.id.as_str()).or_insert(0);
            adj.entry(node.id.as_str()).or_default();
        }

        for node in &self.nodes {
            for dep in &node.depends_on {
                if !all_ids.contains(dep.as_str()) {
                    return Err(format!("DAG node '{}' depends on unknown node '{}'", node.id, dep));
                }
                adj.entry(dep.as_str()).or_default().push(&node.id);
                *in_degree.entry(&node.id).or_insert(0) += 1;
            }
        }

        let mut levels: Vec<Vec<String>> = Vec::new();
        let mut queue: Vec<String> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.to_string())
            .collect();
        let mut visited = 0usize;

        while !queue.is_empty() {
            levels.push(queue.clone());
            let mut next = Vec::new();
            for id in &queue {
                visited += 1;
                if let Some(children) = adj.get(id.as_str()) {
                    for child in children {
                        if let Some(deg) = in_degree.get_mut(child) {
                            *deg -= 1;
                            if *deg == 0 {
                                next.push(child.to_string());
                            }
                        }
                    }
                }
            }
            queue = next;
        }

        if visited != all_ids.len() {
            return Err(format!("DAG has a cycle: visited {} of {} nodes", visited, all_ids.len()));
        }

        Ok(levels)
    }

    /// Merge parallel branch outputs into a single coherent result.
    /// Handles partial failures by noting them in the output.
    pub fn merge_results(results: &[DagStepResult], user_prompt: &str) -> String {
        let successes: Vec<_> = results.iter().filter(|r| r.success).collect();
        let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();

        let mut out = String::new();
        out.push_str(&format!("# Merged DAG Results\n\n**Original task:** {}\n\n", user_prompt));

        if !successes.is_empty() {
            out.push_str("## Successful Results\n\n");
            for r in &successes {
                out.push_str(&format!("### {}\n{}\n\n", r.node_id, r.output));
            }
        }

        if !failures.is_empty() {
            out.push_str("## Failed Steps\n\n");
            for r in &failures {
                out.push_str(&format!("- **{}**: {}\n", r.node_id, r.error.as_deref().unwrap_or("unknown error")));
            }
        }

        out
    }

    pub fn len(&self) -> usize { self.nodes.len() }
}
