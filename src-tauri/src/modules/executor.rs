use std::sync::{Arc, Mutex};

/// Executor module: contains the ReAct loop entrypoint and tool execution helpers.
///
/// This is a refactor-only skeleton for now. The real implementation will be
/// moved from ai.rs into this module without behaviour changes.

/// Simple result for tool execution.
pub struct ToolExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Permission state for tool executions that require user approval (e.g., bash).
#[derive(Debug, Default)]
pub struct PermissionState {
    pub pending_bash_permissions: bool,
}

impl PermissionState {
    pub fn new() -> Self {
        Self { pending_bash_permissions: false }
    }
}

/// Global permission state container. In the real system this will be part of
/// application state registered with Tauri; kept local here for compilation.
pub fn global_permission_state() -> Arc<Mutex<PermissionState>> {
    static mut SINGLETON: Option<Arc<Mutex<PermissionState>>> = None;
    // Safety: single-threaded init in practice during startup; this is a small
    // convenience for the skeleton — the real app should register state properly.
    unsafe {
        if SINGLETON.is_none() {
            SINGLETON = Some(Arc::new(Mutex::new(PermissionState::new())));
        }
        SINGLETON.as_ref().unwrap().clone()
    }
}

/// Run the ReAct loop. Placeholder that returns an error indicating not implemented.
///
/// Parameters mirror the high-level contract: messages, initial context, etc.
pub async fn run_react_loop(
    _messages: &[(&str, &str)],
    _allow_tools: bool,
) -> Result<String, String> {
    Err("run_react_loop: not yet implemented in executor.rs skeleton".into())
}

/// Execute a tool by name with input. For now this is a simple stub that returns
/// a mocked result. The real function will perform subprocesses, HTTP calls,
/// and permission checks.
pub async fn execute_tool(
    tool_name: &str,
    input: &str,
) -> Result<ToolExecutionResult, String> {
    // Mocked behaviour: echo input
    Ok(ToolExecutionResult {
        stdout: format!("[tool:{}] {}", tool_name, input),
        stderr: String::new(),
        exit_code: 0,
    })
}

/// Compute the number of differing lines between two strings. Used by the
/// executor to create compact diffs for patching files.
pub fn compute_diff_lines(old: &str, new: &str) -> usize {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut diff = 0usize;
    let max = std::cmp::max(old_lines.len(), new_lines.len());
    for i in 0..max {
        let a = old_lines.get(i).map(|s| *s).unwrap_or("");
        let b = new_lines.get(i).map(|s| *s).unwrap_or("");
        if a != b {
            diff += 1;
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_tool_stub() {
        let res = execute_tool("echo", "hello").await.unwrap();
        assert!(res.stdout.contains("hello"));
        assert_eq!(res.exit_code, 0);
    }

    #[test]
    fn test_compute_diff_lines() {
        let a = "line1\nline2\nline3";
        let b = "line1\nLINE2\nline3\nnewline";
        let d = compute_diff_lines(a, b);
        // differences: line2 vs LINE2, plus missing/newline -> 2
        assert_eq!(d, 2);
    }
}
