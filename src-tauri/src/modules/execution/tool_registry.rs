use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ToolId {
    Ripgrep,
    TreeSitter,
    Lsp,
    GitLog,
    CargoCheck,
    AstDiffer,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ToolCapability {
    Search,
    Parse,
    Navigate,
    History,
    Compile,
    Diff,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum InputType {
    FilePath,
    Symbol,
    Query,
    Regex,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum OutputType {
    StructuredData,
    Text,
    FileList,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ToolCost {
    Free,
    ComputeOnly,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ToolMetadata {
    pub id: ToolId,
    pub name: String,
    pub capability: Vec<ToolCapability>,
    pub input_type: Vec<InputType>,
    pub output_type: OutputType,
    pub avg_latency_ms: u32,
    pub cost: ToolCost,
    pub requires_index: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ToolRegistry {
    pub tools: Vec<ToolMetadata>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn load_default() -> Self {
        let tools = vec![
            ToolMetadata {
                id: ToolId::Ripgrep,
                name: "ripgrep".into(),
                capability: vec![ToolCapability::Search],
                input_type: vec![InputType::Regex, InputType::Query],
                output_type: OutputType::FileList,
                avg_latency_ms: 50,
                cost: ToolCost::Free,
                requires_index: false,
            },
            ToolMetadata {
                id: ToolId::TreeSitter,
                name: "tree-sitter".into(),
                capability: vec![ToolCapability::Parse, ToolCapability::Navigate],
                input_type: vec![InputType::Symbol, InputType::Query],
                output_type: OutputType::StructuredData,
                avg_latency_ms: 20,
                cost: ToolCost::Free,
                requires_index: true,
            },
            ToolMetadata {
                id: ToolId::Lsp,
                name: "lsp".into(),
                capability: vec![ToolCapability::Navigate],
                input_type: vec![InputType::Symbol],
                output_type: OutputType::StructuredData,
                avg_latency_ms: 100,
                cost: ToolCost::Free,
                requires_index: true,
            },
            ToolMetadata {
                id: ToolId::GitLog,
                name: "git-log".into(),
                capability: vec![ToolCapability::History],
                input_type: vec![InputType::FilePath],
                output_type: OutputType::Text,
                avg_latency_ms: 30,
                cost: ToolCost::Free,
                requires_index: false,
            },
            ToolMetadata {
                id: ToolId::CargoCheck,
                name: "cargo-check".into(),
                capability: vec![ToolCapability::Compile],
                input_type: vec![InputType::FilePath],
                output_type: OutputType::Text,
                avg_latency_ms: 1000,
                cost: ToolCost::ComputeOnly,
                requires_index: false,
            },
        ];

        Self { tools }
    }
}
