use serde::{Deserialize, Serialize};
use super::model_registry::{ModelRegistry, ReasoningTier, Spec};
use super::tool_registry::{ToolId, ToolRegistry};
use super::project_intel::ProjectContext;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ContextSize {
    Small,   // < 8k
    Medium,  // 8k - 30k
    Large,   // 30k - 500k
    Massive, // > 500k
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Intent {
    ExplainSimple,
    CodeWrite,
    CodeReview,
    DebugLogic,
    RefactorFull,
    ArchDesign,
    TestGenerate,
    ScanOnly,
    SymbolLookup,
    ExternalAgent,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum OutputType {
    Narrative,
    CodeDiff,
    CodeFull,
    PlanOnly,
    ToolOutput,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RouteDecision {
    pub intent: Intent,
    pub context_size: ContextSize,
    pub token_count: u32,
    pub output_type: OutputType,
    pub tool_route: Option<ToolId>,
    pub model_route: Option<String>, // Model ID
    pub reasoning_tier: ReasoningTier,
    pub reason: String,
    pub external_agent: Option<String>, // e.g., "claude", "gemini", "opencode", "aider", "codex", "agy"
}

pub struct RoutingEngine {
    pub model_registry: ModelRegistry,
    pub tool_registry: ToolRegistry,
}

impl RoutingEngine {
    pub fn new(model_registry: ModelRegistry, tool_registry: ToolRegistry) -> Self {
        Self {
            model_registry,
            tool_registry,
        }
    }

    pub fn count_tokens(text: &str) -> u32 {
        if let Ok(bpe) = tiktoken_rs::cl100k_base() {
            bpe.encode_with_special_tokens(text).len() as u32
        } else {
            (text.len() / 4) as u32
        }
    }

    pub fn classify_context_size(token_count: u32) -> ContextSize {
        if token_count < 8000 {
            ContextSize::Small
        } else if token_count <= 30000 {
            ContextSize::Medium
        } else if token_count <= 500000 {
            ContextSize::Large
        } else {
            ContextSize::Massive
        }
    }

    pub fn classify_intent(text: &str) -> Intent {
        let lower = text.to_lowercase();
        
        // Symbol lookup triggers
        if lower.contains("where is function") || lower.contains("dimana fungsi") || lower.contains("find definition") || lower.contains("go to definition") {
            return Intent::SymbolLookup;
        }
        
        // Scan only triggers
        if lower.contains("scan files") || lower.contains("list files") || lower.contains("cari file") || lower.contains("list semua") {
            return Intent::ScanOnly;
        }

        // Test generation triggers
        if lower.contains("unit test") || lower.contains("buat test") || lower.contains("coverage") || lower.contains("test case") {
            return Intent::TestGenerate;
        }

        // Arch design triggers
        if lower.contains("arsitektur") || lower.contains("desain sistem") || lower.contains("architecture") || lower.contains("design system") {
            return Intent::ArchDesign;
        }

        // Refactor full triggers
        if lower.contains("refactor") || lower.contains("optimasi") || lower.contains("restructure") {
            return Intent::RefactorFull;
        }

        // Debug logic triggers
        if lower.contains("bug") || lower.contains("error") || lower.contains("kenapa salah") || lower.contains("memory leak") || lower.contains("race condition") {
            return Intent::DebugLogic;
        }

        // Code review triggers
        if lower.contains("review") || lower.contains("tinjau") || lower.contains("periksa kode") {
            return Intent::CodeReview;
        }

        // Code write triggers
        if lower.contains("tulis fungsi") || lower.contains("buat class") || lower.contains("implementasi") || lower.contains("write a function") || lower.contains("tulis kode") {
            return Intent::CodeWrite;
        }

        // External agent triggers (heavy task delegation to CLI agents)
        if lower.contains("use claude") || lower.contains("pakai claude") || lower.contains("delegate to") || lower.contains("serahkan ke") || lower.contains("call opencode") || lower.contains("panggil opencode") || lower.contains("run aider") || lower.contains("jalankan aider") {
            return Intent::ExternalAgent;
        }

        // Default simple explain
        Intent::ExplainSimple
    }

    pub fn classify_output_type(text: &str, intent: &Intent) -> OutputType {
        let lower = text.to_lowercase();

        if matches!(intent, Intent::SymbolLookup) {
            return OutputType::ToolOutput;
        }

        if lower.contains("explain") || lower.contains("jelaskan") || lower.contains("bedanya") || lower.contains("summary") {
            return OutputType::Narrative;
        }

        if lower.contains("pseudocode") || lower.contains("langkah-langkah") || lower.contains("plan") || lower.contains("rencana") {
            return OutputType::PlanOnly;
        }

        if lower.contains("rewrite complete") || lower.contains("tulis ulang seluruh") || lower.contains("create file") || lower.contains("file baru") {
            return OutputType::CodeFull;
        }

        // Default code diff for code modifications
        if matches!(intent, Intent::CodeWrite | Intent::DebugLogic | Intent::RefactorFull | Intent::TestGenerate) {
            OutputType::CodeDiff
        } else {
            OutputType::Narrative
        }
    }

    pub fn route_request(&self, text: &str) -> RouteDecision {
        let token_count = Self::count_tokens(text);
        let context_size = Self::classify_context_size(token_count);
        let cleaned_text = strip_injected_context(text);
        let intent = Self::classify_intent(&cleaned_text);
        let output_type = Self::classify_output_type(&cleaned_text, &intent);

        // Step 1: Tool-first routing
        let mut tool_route = None;
        let mut external_agent = None;
        
        if intent == Intent::SymbolLookup {
            tool_route = Some(ToolId::TreeSitter);
        } else if intent == Intent::ScanOnly {
            tool_route = Some(ToolId::Ripgrep);
        } else if intent == Intent::ExternalAgent {
            // Determine which external CLI agent to use based on keywords
            let lower = text.to_lowercase();
            if lower.contains("claude") {
                external_agent = Some("claude".to_string());
            } else if lower.contains("gemini") {
                external_agent = Some("gemini".to_string());
            } else if lower.contains("opencode") {
                external_agent = Some("opencode".to_string());
            } else if lower.contains("aider") {
                external_agent = Some("aider".to_string());
            } else if lower.contains("codex") {
                external_agent = Some("codex".to_string());
            } else if lower.contains("agy") {
                external_agent = Some("agy".to_string());
            } else {
                // Default to claude for heavy tasks
                external_agent = Some("claude".to_string());
            }
        }

        // Step 2: Determine reasoning tier
        let reasoning_tier = match intent {
            Intent::ExplainSimple => ReasoningTier::Low,
            Intent::CodeWrite | Intent::TestGenerate => ReasoningTier::High,
            Intent::CodeReview | Intent::DebugLogic | Intent::RefactorFull | Intent::ArchDesign => ReasoningTier::UltraHigh,
            Intent::ScanOnly | Intent::SymbolLookup => ReasoningTier::Medium,
            Intent::ExternalAgent => ReasoningTier::UltraHigh,
        };

        // Step 3: Select specialized model spec
        let spec = match intent {
            Intent::ExplainSimple | Intent::ArchDesign => Spec::Chat,
            Intent::CodeWrite | Intent::RefactorFull => Spec::Code,
            Intent::CodeReview => Spec::Review,
            Intent::TestGenerate => Spec::Test,
            Intent::ScanOnly | Intent::SymbolLookup | Intent::DebugLogic => Spec::Scan,
            Intent::ExternalAgent => Spec::Code,
        };

        // Step 4: Capability-based model routing
        let model = self.model_registry.select_model(reasoning_tier.clone(), spec, token_count);
        let model_route = model.map(|m| m.id.clone());

        let reason = if let Some(agent) = &external_agent {
            format!("Routed to external CLI agent '{}' for heavy task delegation.", agent)
        } else if let Some(tool) = &tool_route {
            format!("Routed to deterministic tool: {:?}.", tool)
        } else if let Some(model_id) = &model_route {
            format!(
                "Routed to model '{}' based on tier {:?}, spec {:?}, and context size ({} tokens).",
                model_id, reasoning_tier, spec, token_count
            )
        } else {
            "No suitable model found in registry, falling back to default agent model.".to_string()
        };

        RouteDecision {
            intent,
            context_size,
            token_count,
            output_type,
            tool_route,
            model_route,
            reasoning_tier,
            reason,
            external_agent,
        }
    }

    /// Route with project context awareness: adjusts model/spec based on detected framework.
    pub fn route_with_context(&self, text: &str, ctx: &ProjectContext) -> RouteDecision {
        let mut decision = self.route_request(text);

        // Adjust tier based on project complexity
        if ctx.file_count > 200 && decision.reasoning_tier == ReasoningTier::Medium {
            decision.reasoning_tier = ReasoningTier::High;
            decision.reason.push_str(&format!(" Project has {} source files, upgraded to High.", ctx.file_count));
        }

        // Prefer code-specialized spec for known frameworks
        match ctx.framework {
            super::project_intel::ProjectFramework::RustCargo => {
                if decision.intent == Intent::CodeReview || decision.intent == Intent::DebugLogic {
                    if let Some(m) = self.model_registry.select_model(decision.reasoning_tier.clone(), Spec::Code, decision.token_count) {
                        decision.model_route = Some(m.id.clone());
                        decision.reason.push_str(" Rust project, using code-specialized model.");
                    }
                }
            }
            super::project_intel::ProjectFramework::NodeNpm | super::project_intel::ProjectFramework::NodeYarn => {
                if decision.intent == Intent::ExplainSimple {
                    if let Some(m) = self.model_registry.select_model(ReasoningTier::Low, Spec::Chat, decision.token_count) {
                        decision.model_route = Some(m.id.clone());
                        decision.reason.push_str(" Node.js project, lightweight chat model sufficient.");
                    }
                }
            }
            _ => {}
        }

        decision
    }
}

pub fn strip_injected_context(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    // State-based skipping of blocks at the start
    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("[Global Custom Instructions]") {
            i += 1;
            while i < lines.len() {
                let next_line = lines[i].trim();
                if next_line.starts_with("[Agent Skills Toggles]") || next_line.starts_with("[Active Editor Context - File:") {
                    break;
                }
                i += 1;
            }
        } else if line.starts_with("[Agent Skills Toggles]") {
            i += 1;
            while i < lines.len() {
                let next_line = lines[i].trim();
                if next_line.starts_with("[Active Editor Context - File:") {
                    break;
                }
                if next_line.is_empty() {
                    i += 1;
                    break;
                }
                i += 1;
            }
        } else if line.starts_with("[Active Editor Context - File:") {
            i += 1;
            let mut in_code = false;
            while i < lines.len() {
                let next_line = lines[i].trim();
                if next_line.starts_with("```") {
                    if in_code {
                        i += 1;
                        while i < lines.len() && lines[i].trim().is_empty() {
                            i += 1;
                        }
                        break;
                    } else {
                        in_code = true;
                    }
                }
                i += 1;
            }
        } else {
            break;
        }
    }

    // Collect prompt lines until the attached files section
    let mut prompt_lines = Vec::new();
    while i < lines.len() {
        let line = lines[i];
        if line.trim() == "---" && i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            if next.starts_with("[Attached File:") || next.starts_with("[Attached File Reference:") {
                break;
            }
        }
        prompt_lines.push(line);
        i += 1;
    }

    prompt_lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_injected_context() {
        let input = "[Global Custom Instructions]\nAlways answer in Indonesian\n\n[Agent Skills Toggles]\n- Reading Files: ENABLED\n- Writing/Editing Files: DISABLED (Do not use write_file, edit. Inform user if requested)\n- Terminal Command Execution: DISABLED (Do not use bash_run. Inform user if requested)\n\n[Active Editor Context - File: c:\\foo.js]\n```\nconst x = 1;\n```\n\nhayy\n\n---\n[Attached File: bar.js]\nconsole.log(1);\n---";
        let cleaned = strip_injected_context(input);
        assert_eq!(cleaned, "hayy");
    }
}
