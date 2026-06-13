use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ProjectFramework {
    RustCargo,
    NodeNpm,
    NodeYarn,
    PythonPoetry,
    PythonPip,
    GoMod,
    PlatformIO,
    Docker,
    Unknown,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ProjectContext {
    pub framework: ProjectFramework,
    pub has_tests: bool,
    pub has_ci: bool,
    pub has_docker: bool,
    pub has_kubernetes: bool,
    pub file_count: usize,
    pub language: String,
    pub src_dirs: Vec<String>,
}

impl ProjectContext {
    pub fn detect(root: &Path) -> Self {
        let framework = detect_framework(root);
        let has_tests = root.join("tests").exists() || root.join("test").exists() || root.join("__tests__").exists();
        let has_ci = root.join(".github").exists() || root.join(".gitlab-ci.yml").exists();
        let has_docker = root.join("Dockerfile").exists() || root.join("docker-compose.yml").exists();
        let has_kubernetes = root.join("k8s").exists() || root.join("kubernetes").exists() || root.join("deploy/k8s").exists() || root.join("deploy/kubernetes").exists();
        let src_dirs = detect_src_dirs(root);
        let file_count = count_source_files(root);
        let language = match framework {
            ProjectFramework::RustCargo => "Rust".to_string(),
            ProjectFramework::NodeNpm | ProjectFramework::NodeYarn => "JavaScript/TypeScript".to_string(),
            ProjectFramework::PythonPoetry | ProjectFramework::PythonPip => "Python".to_string(),
            ProjectFramework::GoMod => "Go".to_string(),
            ProjectFramework::PlatformIO => "C/C++".to_string(),
            _ => "Unknown".to_string(),
        };

        Self { framework, has_tests, has_ci, has_docker, has_kubernetes, file_count, language, src_dirs }
    }

    pub fn framework_label(&self) -> &'static str {
        match self.framework {
            ProjectFramework::RustCargo => "Rust/Cargo",
            ProjectFramework::NodeNpm => "Node.js (npm)",
            ProjectFramework::NodeYarn => "Node.js (yarn)",
            ProjectFramework::PythonPoetry => "Python (Poetry)",
            ProjectFramework::PythonPip => "Python (pip)",
            ProjectFramework::GoMod => "Go",
            ProjectFramework::PlatformIO => "PlatformIO",
            ProjectFramework::Docker => "Docker",
            ProjectFramework::Unknown => "Unknown",
        }
    }
}

fn detect_framework(root: &Path) -> ProjectFramework {
    if root.join("Cargo.toml").exists() { return ProjectFramework::RustCargo; }
    if root.join("yarn.lock").exists() || root.join(".yarnrc.yml").exists() { return ProjectFramework::NodeYarn; }
    if root.join("package-lock.json").exists() || root.join("package.json").exists() { return ProjectFramework::NodeNpm; }
    if root.join("pyproject.toml").exists() && root.join("poetry.lock").exists() { return ProjectFramework::PythonPoetry; }
    if root.join("requirements.txt").exists() || root.join("setup.py").exists() { return ProjectFramework::PythonPip; }
    if root.join("go.mod").exists() { return ProjectFramework::GoMod; }
    if root.join("platformio.ini").exists() { return ProjectFramework::PlatformIO; }
    if root.join("Dockerfile").exists() { return ProjectFramework::Docker; }

    // Scan subdirectories (1 level deep) for monorepos or nested stack
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("package.json").exists() {
                    return ProjectFramework::NodeNpm;
                }
                if path.join("Cargo.toml").exists() {
                    return ProjectFramework::RustCargo;
                }
                if path.join("go.mod").exists() {
                    return ProjectFramework::GoMod;
                }
                if path.join("platformio.ini").exists() {
                    return ProjectFramework::PlatformIO;
                }
            }
        }
    }
    ProjectFramework::Unknown
}

fn detect_src_dirs(root: &Path) -> Vec<String> {
    let candidates = ["src", "lib", "app", "packages", "modules"];
    let mut dirs: Vec<String> = candidates.iter()
        .filter(|d| root.join(d).is_dir())
        .map(|d| d.to_string())
        .collect();

    if dirs.is_empty() {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with('.') && name != "node_modules" && name != "target" && name != "gradle" && name != "Temp_SDK" && name != "dist" {
                        dirs.push(name.to_string());
                    }
                }
            }
        }
    }
    dirs
}

fn count_source_files(root: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "node_modules" && name != "target" {
                    count += count_source_files(&path);
                }
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    match ext {
                        "rs" | "js" | "jsx" | "ts" | "tsx" | "py" | "go" | "c" | "cpp" | "h" | "hpp" => count += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    count
}

pub struct ProjectIntelState {
    pub context: Arc<Mutex<Option<ProjectContext>>>,
}

impl ProjectIntelState {
    pub fn new() -> Self {
        Self { context: Arc::new(Mutex::new(None)) }
    }

    pub fn detect(&self, root: &str) -> ProjectContext {
        let ctx = ProjectContext::detect(Path::new(root));
        if let Ok(mut guard) = self.context.lock() {
            *guard = Some(ctx.clone());
        }
        ctx
    }

    pub fn get(&self) -> Result<ProjectContext, String> {
        let guard = self.context.lock().map_err(|e| format!("Lock: {}", e))?;
        guard.clone().ok_or("No project context detected yet. Run project_detect first.".to_string())
    }
}

impl Default for ProjectIntelState {
    fn default() -> Self { Self::new() }
}

#[tauri::command]
pub fn project_detect(
    state: tauri::State<'_, ProjectIntelState>,
    root: String,
) -> Result<ProjectContext, String> {
    Ok(state.detect(&root))
}

#[tauri::command]
pub fn project_get_context(
    state: tauri::State<'_, ProjectIntelState>,
) -> Result<ProjectContext, String> {
    state.get()
}
