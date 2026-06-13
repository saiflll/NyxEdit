use std::sync::OnceLock;

use super::symbol_graph::{EdgeKind, SymbolGraph, SymbolKind, SymbolNode};

static RUST_LANG: OnceLock<tree_sitter::Language> = OnceLock::new();
static JS_LANG: OnceLock<tree_sitter::Language> = OnceLock::new();
static PY_LANG: OnceLock<tree_sitter::Language> = OnceLock::new();

fn get_rust_lang() -> &'static tree_sitter::Language {
    RUST_LANG.get_or_init(|| tree_sitter_rust::LANGUAGE.into())
}

fn get_js_lang() -> &'static tree_sitter::Language {
    JS_LANG.get_or_init(|| tree_sitter_javascript::LANGUAGE.into())
}

fn get_py_lang() -> &'static tree_sitter::Language {
    PY_LANG.get_or_init(|| tree_sitter_python::LANGUAGE.into())
}

fn lang_for_file(path: &str) -> Option<&'static tree_sitter::Language> {
    if path.ends_with(".rs") { Some(get_rust_lang()) }
    else if path.ends_with(".js") || path.ends_with(".jsx") || path.ends_with(".mjs") { Some(get_js_lang()) }
    else if path.ends_with(".ts") || path.ends_with(".tsx") { Some(get_js_lang()) }
    else if path.ends_with(".py") { Some(get_py_lang()) }
    else { None }
}

fn symbol_name_from_capture(node: &tree_sitter::Node, source: &[u8]) -> String {
    // Try to find an identifier child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "field_identifier"
            || child.kind() == "type_identifier" || child.kind() == "name"
        {
            return child.utf8_text(source).unwrap_or("").to_string();
        }
    }
    // Fallback: first token
    node.utf8_text(source).unwrap_or("").to_string()
}

fn parse_rust_file(graph: &mut SymbolGraph, source: &[u8], file_path: &str) {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(get_rust_lang()).ok();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return,
    };
    let root = tree.root_node();

    fn walk_rust(
        node: tree_sitter::Node,
        graph: &mut SymbolGraph,
        source: &[u8],
        file_path: &str,
        parent_id: Option<String>,
        module_path: &str,
    ) {
        let kind = node.kind();
        let (symbol_kind, is_def) = match kind {
            "struct_item" => (SymbolKind::Struct, true),
            "enum_item" => (SymbolKind::Enum, true),
            "trait_item" => (SymbolKind::Trait, true),
            "function_item" | "function_signature" => (SymbolKind::Function, true),
            "macro_definition" => (SymbolKind::Macro, true),
            "type_item" => (SymbolKind::TypeAlias, true),
            "const_item" => (SymbolKind::Constant, true),
            "static_item" => (SymbolKind::Variable, true),
            "impl_item" => (SymbolKind::Trait, false),
            "impl_trait" | "impl" => (SymbolKind::Trait, false),
            _ => return,
        };

        if is_def {
            let name = symbol_name_from_capture(&node, source);
            if !name.is_empty() {
                let id = format!("{}::{}", module_path, name);
                let (s_line, s_col) = (node.start_position().row + 1, node.start_position().column + 1);
                let (e_line, _) = (node.end_position().row + 1, node.end_position().column + 1);
                let node_id = id.clone();

                graph.add_node(SymbolNode {
                    id: node_id.clone(),
                    name: name.clone(),
                    kind: symbol_kind,
                    file_path: file_path.to_string(),
                    line: s_line,
                    column: s_col,
                    end_line: e_line,
                    doc_comment: None,
                    parent_id: parent_id.clone(),
                });

                if let Some(pid) = &parent_id {
                    graph.add_edge(super::symbol_graph::SymbolEdge {
                        source_id: pid.clone(),
                        target_id: node_id.clone(),
                        kind: EdgeKind::Contains,
                        file_path: file_path.to_string(),
                        line: s_line,
                    });
                }

                // Walk children with new module path
                let child_module = format!("{}::{}", module_path, name);
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    walk_rust(child, graph, source, file_path, Some(node_id.clone()), &child_module);
                }
                return;
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_rust(child, graph, source, file_path, parent_id.clone(), module_path);
        }
    }

    walk_rust(root, graph, source, file_path, None, "");
}

fn parse_js_file(graph: &mut SymbolGraph, source: &[u8], file_path: &str) {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(get_js_lang()).ok();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return,
    };
    let root = tree.root_node();

    fn walk_js(
        node: tree_sitter::Node,
        graph: &mut SymbolGraph,
        source: &[u8],
        file_path: &str,
        parent_id: Option<String>,
    ) {
        let kind = node.kind();
        let (symbol_kind, name) = match kind {
            "function_declaration" => {
                let mut c = node.walk();
                let n = node.children(&mut c).find(|ch| ch.kind() == "identifier")
                    .and_then(|n| n.utf8_text(source).ok()).unwrap_or("").to_string();
                (SymbolKind::Function, n)
            }
            "class_declaration" => {
                let mut c = node.walk();
                let n = node.children(&mut c).find(|ch| ch.kind() == "identifier")
                    .and_then(|n| n.utf8_text(source).ok()).unwrap_or("").to_string();
                (SymbolKind::Class, n)
            }
            "method_definition" => {
                let mut c = node.walk();
                let n = node.children(&mut c).find(|ch| ch.kind() == "property_identifier")
                    .and_then(|n| n.utf8_text(source).ok()).unwrap_or("").to_string();
                (SymbolKind::Method, n)
            }
            "arrow_function" | "generator_function" => (SymbolKind::Closure, String::new()),
            "lexical_declaration" | "variable_declaration" => {
                let mut c = node.walk();
                let n = node.children(&mut c).find(|ch| ch.kind() == "identifier")
                    .and_then(|n| n.utf8_text(source).ok()).unwrap_or("").to_string();
                (SymbolKind::Variable, n)
            }
            "export_statement" => (SymbolKind::Export, String::new()),
            "import_statement" => (SymbolKind::Import, String::new()),
            _ => return,
        };

        if !name.is_empty() {
            let id = format!("{}:{}", file_path, name);
            let (s_line, s_col) = (node.start_position().row + 1, node.start_position().column + 1);
            let (e_line, _) = (node.end_position().row + 1, node.end_position().column + 1);

            graph.add_node(SymbolNode {
                id: id.clone(),
                name: name.clone(),
                kind: symbol_kind,
                file_path: file_path.to_string(),
                line: s_line,
                column: s_col,
                end_line: e_line,
                doc_comment: None,
                parent_id: parent_id.clone(),
            });

            if let Some(pid) = &parent_id {
                graph.add_edge(super::symbol_graph::SymbolEdge {
                    source_id: pid.clone(),
                    target_id: id.clone(),
                    kind: EdgeKind::Contains,
                    file_path: file_path.to_string(),
                    line: s_line,
                });
            }

            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children {
                walk_js(child, graph, source, file_path, Some(id.clone()));
            }
        } else {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_js(child, graph, source, file_path, parent_id.clone());
            }
        }
    }

    walk_js(root, graph, source, file_path, None);
}

fn parse_py_file(graph: &mut SymbolGraph, source: &[u8], file_path: &str) {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(get_py_lang()).ok();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return,
    };
    let root = tree.root_node();

    fn walk_py(
        node: tree_sitter::Node,
        graph: &mut SymbolGraph,
        source: &[u8],
        file_path: &str,
        parent_id: Option<String>,
    ) {
        let kind = node.kind();
        let (symbol_kind, name) = match kind {
            "function_definition" => {
                let mut c = node.walk();
                let n = node.children(&mut c).find(|ch| ch.kind() == "identifier")
                    .and_then(|n| n.utf8_text(source).ok()).unwrap_or("").to_string();
                (SymbolKind::Function, n)
            }
            "class_definition" => {
                let mut c = node.walk();
                let n = node.children(&mut c).find(|ch| ch.kind() == "identifier")
                    .and_then(|n| n.utf8_text(source).ok()).unwrap_or("").to_string();
                (SymbolKind::Class, n)
            }
            "assignment" => {
                let mut c = node.walk();
                let n = node.children(&mut c).find(|ch| ch.kind() == "identifier")
                    .and_then(|n| n.utf8_text(source).ok()).unwrap_or("").to_string();
                (SymbolKind::Variable, n)
            }
            "import_statement" | "import_from_statement" => (SymbolKind::Import, String::new()),
            _ => return,
        };

        if !name.is_empty() {
            let id = format!("{}:{}", file_path, name);
            let (s_line, s_col) = (node.start_position().row + 1, node.start_position().column + 1);
            let (e_line, _) = (node.end_position().row + 1, node.end_position().column + 1);

            graph.add_node(SymbolNode {
                id: id.clone(),
                name: name.clone(),
                kind: symbol_kind,
                file_path: file_path.to_string(),
                line: s_line,
                column: s_col,
                end_line: e_line,
                doc_comment: None,
                parent_id: parent_id.clone(),
            });

            if let Some(pid) = &parent_id {
                graph.add_edge(super::symbol_graph::SymbolEdge {
                    source_id: pid.clone(),
                    target_id: id.clone(),
                    kind: EdgeKind::Contains,
                    file_path: file_path.to_string(),
                    line: s_line,
                });
            }

            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            for child in children {
                walk_py(child, graph, source, file_path, Some(id.clone()));
            }
        } else {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_py(child, graph, source, file_path, parent_id.clone());
            }
        }
    }

    walk_py(root, graph, source, file_path, None);
}

fn parse_ts_file_regex(graph: &mut SymbolGraph, source: &[u8], file_path: &str) {
    let content = std::str::from_utf8(source).unwrap_or("");
    let mut current_class: Option<String> = None;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // 1. Detect Class (e.g. class UserController or export class UserService)
        if let Some(caps) = regex::Regex::new(r"\bclass\s+([a-zA-Z0-9_]+)").ok()
            .and_then(|re| re.captures(trimmed))
        {
            let class_name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            if !class_name.is_empty() {
                let id = format!("{}:{}", file_path, class_name);
                graph.add_node(SymbolNode {
                    id: id.clone(),
                    name: class_name.clone(),
                    kind: SymbolKind::Class,
                    file_path: file_path.to_string(),
                    line: line_num,
                    column: 1,
                    end_line: line_num + 5,
                    doc_comment: None,
                    parent_id: None,
                });
                current_class = Some(id);
                continue;
            }
        }

        // 2. Detect Function (e.g. function bootstrap() or export async function handler())
        if let Some(caps) = regex::Regex::new(r"\bfunction\s+([a-zA-Z0-9_]+)").ok()
            .and_then(|re| re.captures(trimmed))
        {
            let fn_name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            if !fn_name.is_empty() {
                let id = format!("{}:{}", file_path, fn_name);
                graph.add_node(SymbolNode {
                    id: id.clone(),
                    name: fn_name.clone(),
                    kind: SymbolKind::Function,
                    file_path: file_path.to_string(),
                    line: line_num,
                    column: 1,
                    end_line: line_num + 2,
                    doc_comment: None,
                    parent_id: None,
                });
                continue;
            }
        }

        // 3. Detect Methods inside Classes (e.g. async getProfile(...) or @Get() findAll())
        if let Some(ref class_id) = current_class {
            if (trimmed.contains('(') && (trimmed.contains('{') || trimmed.contains(';'))) || trimmed.ends_with('(') {
                if let Some(caps) = regex::Regex::new(r"(?:public|private|protected|async|get|set|\s)+\s*([a-zA-Z0-9_]+)\s*\(").ok()
                    .and_then(|re| re.captures(trimmed))
                {
                    let method_name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                    if !method_name.is_empty() && method_name != "constructor" && method_name != "function" && method_name != "if" && method_name != "switch" {
                        let id = format!("{}:{}", class_id, method_name);
                        graph.add_node(SymbolNode {
                            id: id.clone(),
                            name: method_name.clone(),
                            kind: SymbolKind::Method,
                            file_path: file_path.to_string(),
                            line: line_num,
                            column: 1,
                            end_line: line_num + 3,
                            doc_comment: None,
                            parent_id: Some(class_id.clone()),
                        });

                        graph.add_edge(super::symbol_graph::SymbolEdge {
                            source_id: class_id.clone(),
                            target_id: id.clone(),
                            kind: EdgeKind::Contains,
                            file_path: file_path.to_string(),
                            line: line_num,
                        });
                    }
                }
            }
        }
    }
}

pub fn parse_file(graph: &mut SymbolGraph, source: &[u8], file_path: &str) {
    if lang_for_file(file_path).is_none() {
        return;
    }

    if file_path.ends_with(".rs") {
        parse_rust_file(graph, source, file_path);
    } else if file_path.ends_with(".py") {
        parse_py_file(graph, source, file_path);
    } else if file_path.ends_with(".ts") || file_path.ends_with(".tsx") {
        parse_ts_file_regex(graph, source, file_path);
    } else {
        parse_js_file(graph, source, file_path);
    }

    infer_name_references(graph, file_path, source);
}

fn infer_name_references(graph: &mut SymbolGraph, file_path: &str, source: &[u8]) {
    let content = std::str::from_utf8(source).unwrap_or("");
    let file_node_ids: Vec<String> = graph.nodes.values()
        .filter(|n| n.file_path == file_path)
        .map(|n| n.id.clone())
        .collect();

    for (line_num, line_content) in content.lines().enumerate() {
        let line = line_num + 1;
        for node_id in &file_node_ids {
            if let Some(node) = graph.get_node(node_id) {
                if line_content.contains(&node.name) && node.name.len() > 2 {
                    if node.line == line { continue; }
                    graph.add_edge(super::symbol_graph::SymbolEdge {
                        source_id: format!("{}:{}", file_path, "use"),
                        target_id: node_id.clone(),
                        kind: EdgeKind::References,
                        file_path: file_path.to_string(),
                        line,
                    });
                }
            }
        }
    }
}

pub fn index_workspace(graph: &mut SymbolGraph, root: &std::path::Path) {
    let mut entries = Vec::new();
    collect_source_files(root, &mut entries, root);
    for entry in &entries {
        if let Ok(content) = std::fs::read(entry) {
            let rel_path = entry.strip_prefix(root).unwrap_or(entry)
                .to_string_lossy().to_string();
            parse_file(graph, &content, &rel_path);
        }
    }
}

pub fn collect_source_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>, root: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "node_modules" && name != "target" {
                    collect_source_files(&path, out, root);
                }
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    match ext {
                        "rs" | "js" | "jsx" | "ts" | "tsx" | "mjs" | "py" => {
                            out.push(path);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
