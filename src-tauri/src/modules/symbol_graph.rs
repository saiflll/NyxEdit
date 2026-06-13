use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Module,
    Namespace,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    Constructor,
    Closure,
    Variable,
    Constant,
    Parameter,
    Field,
    TypeAlias,
    Interface,
    Class,
    Decorator,
    Macro,
    Import,
    Export,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Contains,
    Calls,
    Imports,
    Exports,
    Extends,
    Implements,
    References,
    Defines,
    Inherits,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SymbolNode {
    pub id: String,
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub doc_comment: Option<String>,
    pub parent_id: Option<String>,
}

impl SymbolNode {
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            SymbolKind::Module => "module",
            SymbolKind::Namespace => "namespace",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Constructor => "constructor",
            SymbolKind::Closure => "closure",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Parameter => "parameter",
            SymbolKind::Field => "field",
            SymbolKind::TypeAlias => "type",
            SymbolKind::Interface => "interface",
            SymbolKind::Class => "class",
            SymbolKind::Decorator => "decorator",
            SymbolKind::Macro => "macro",
            SymbolKind::Import => "import",
            SymbolKind::Export => "export",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SymbolEdge {
    pub source_id: String,
    pub target_id: String,
    pub kind: EdgeKind,
    pub file_path: String,
    pub line: usize,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SymbolGraph {
    pub nodes: HashMap<String, SymbolNode>,
    pub edges: Vec<SymbolEdge>,
    #[serde(skip)]
    adjacency: HashMap<String, Vec<(String, EdgeKind)>>,
    #[serde(skip)]
    reverse_adjacency: HashMap<String, Vec<(String, EdgeKind)>>,
}

impl SymbolGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
            reverse_adjacency: HashMap::new(),
        }
    }

    pub fn rebuild_adjacency(&mut self) {
        self.adjacency.clear();
        self.reverse_adjacency.clear();
        for edge in &self.edges {
            self.adjacency.entry(edge.source_id.clone()).or_default().push((edge.target_id.clone(), edge.kind.clone()));
            self.reverse_adjacency.entry(edge.target_id.clone()).or_default().push((edge.source_id.clone(), edge.kind.clone()));
        }
    }

    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }

    pub fn load_from(path: &std::path::Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut graph: Self = serde_json::from_str(&json)?;
        graph.rebuild_adjacency();
        Ok(graph)
    }

    pub fn add_node(&mut self, node: SymbolNode) {
        let id = node.id.clone();
        self.nodes.insert(id, node);
    }

    pub fn add_edge(&mut self, edge: SymbolEdge) {
        let source = edge.source_id.clone();
        let target = edge.target_id.clone();
        let kind = edge.kind.clone();
        self.edges.push(edge);
        self.adjacency.entry(source.clone()).or_default().push((target.clone(), kind.clone()));
        self.reverse_adjacency.entry(target).or_default().push((source, kind));
    }

    pub fn get_node(&self, id: &str) -> Option<&SymbolNode> {
        self.nodes.get(id)
    }

    pub fn find_by_name(&self, name: &str) -> Vec<&SymbolNode> {
        self.nodes.values().filter(|n| n.name == name).collect()
    }

    pub fn find_by_file(&self, path: &str) -> Vec<&SymbolNode> {
        self.nodes.values().filter(|n| n.file_path == path).collect()
    }

    pub fn find_by_kind(&self, kind: SymbolKind) -> Vec<&SymbolNode> {
        self.nodes.values().filter(|n| n.kind == kind).collect()
    }

    pub fn outgoing(&self, id: &str) -> Vec<(&str, &EdgeKind)> {
        self.adjacency.get(id)
            .map(|v| v.iter().map(|(t, k)| (t.as_str(), k)).collect())
            .unwrap_or_default()
    }

    pub fn incoming(&self, id: &str) -> Vec<(&str, &EdgeKind)> {
        self.reverse_adjacency.get(id)
            .map(|v| v.iter().map(|(s, k)| (s.as_str(), k)).collect())
            .unwrap_or_default()
    }

    pub fn references(&self, id: &str) -> Vec<&str> {
        self.incoming(id).into_iter()
            .filter(|(_, k)| *k == &EdgeKind::References)
            .map(|(s, _)| s)
            .collect()
    }

    pub fn definitions(&self, name: &str) -> Vec<&SymbolNode> {
        self.nodes.values().filter(|n| {
            n.name == name && (n.kind == SymbolKind::Function
                || n.kind == SymbolKind::Method
                || n.kind == SymbolKind::Struct
                || n.kind == SymbolKind::Class
                || n.kind == SymbolKind::Enum
                || n.kind == SymbolKind::Trait
                || n.kind == SymbolKind::Interface
                || n.kind == SymbolKind::Variable
                || n.kind == SymbolKind::Constant)
        }).collect()
    }

    pub fn search(&self, query: &str) -> Vec<&SymbolNode> {
        let q = query.to_lowercase();
        self.nodes.values().filter(|n| {
            n.name.to_lowercase().contains(&q)
                || n.file_path.to_lowercase().contains(&q)
        }).collect()
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.adjacency.clear();
        self.reverse_adjacency.clear();
    }

    /// Depth-bounded traversal from a node, following outgoing edges.
    /// Returns reachable node IDs up to `max_depth` hops.
    pub fn traverse(&self, start_id: &str, max_depth: usize) -> Vec<&SymbolNode> {
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut queue = Vec::new();
        queue.push((start_id.to_string(), 0));
        visited.insert(start_id.to_string());

        while let Some((current, depth)) = queue.pop() {
            if let Some(node) = self.nodes.get(&current) {
                result.push(node);
            }
            if depth >= max_depth { continue; }
            if let Some(edges) = self.adjacency.get(&current) {
                for (target, _) in edges {
                    if visited.insert(target.clone()) {
                        queue.push((target.clone(), depth + 1));
                    }
                }
            }
        }
        result
    }

    /// Extract a subgraph of connected nodes within `max_hops` of the given node.
    /// Returns a new SymbolGraph with just the relevant nodes + edges.
    pub fn subgraph(&self, center_id: &str, max_hops: usize) -> SymbolGraph {
        let nodes_in_subgraph: std::collections::HashSet<String> = {
            let mut set = std::collections::HashSet::new();
            let mut queue = Vec::new();
            queue.push((center_id.to_string(), 0));
            set.insert(center_id.to_string());

            while let Some((current, depth)) = queue.pop() {
                if depth >= max_hops { continue; }
                if let Some(edges) = self.adjacency.get(&current) {
                    for (target, _) in edges {
                        if set.insert(target.clone()) {
                            queue.push((target.clone(), depth + 1));
                        }
                    }
                }
                if let Some(edges) = self.reverse_adjacency.get(&current) {
                    for (source, _) in edges {
                        if set.insert(source.clone()) {
                            queue.push((source.clone(), depth + 1));
                        }
                    }
                }
            }
            set
        };

        let mut sg = SymbolGraph::new();
        for id in &nodes_in_subgraph {
            if let Some(node) = self.nodes.get(id) {
                sg.add_node(node.clone());
            }
        }
        for edge in &self.edges {
            if nodes_in_subgraph.contains(&edge.source_id) && nodes_in_subgraph.contains(&edge.target_id) {
                sg.add_edge(edge.clone());
            }
        }
        sg
    }
}
