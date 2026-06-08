use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug)]
pub struct PioStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub python: Option<String>,
    pub pio_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PioResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Try to find a working way to execute PlatformIO commands.
/// Returns (executable, base_args) or None if not found.
fn find_pio() -> Option<(String, Vec<String>)> {
    // 1. Try direct `pio` binary via PATH
    if let Ok(output) = Command::new("pio").arg("--version").output() {
        if output.status.success() {
            return Some(("pio".to_string(), vec![]));
        }
    }

    // 2. Try direct `platformio` binary via PATH
    if let Ok(output) = Command::new("platformio").arg("--version").output() {
        if output.status.success() {
            return Some(("platformio".to_string(), vec![]));
        }
    }

    // 3. Try `python -m platformio` (common on Windows after pip install)
    for py in &["python", "python3", "py"] {
        if let Ok(output) = Command::new(py)
            .args(["-m", "platformio", "--version"])
            .output()
        {
            if output.status.success() {
                return Some((py.to_string(), vec!["-m".to_string(), "platformio".to_string()]));
            }
        }
    }

    None
}

fn run_pio(args: &[&str]) -> Result<std::process::Output, String> {
    let (exe, base_args) = find_pio().ok_or_else(|| {
        "PlatformIO not found. Install it with: pip install platformio".to_string()
    })?;

    let mut cmd = Command::new(&exe);
    cmd.args(&base_args);
    cmd.args(args);
    cmd.output().map_err(|e| format!("Failed to run pio: {}", e))
}

#[tauri::command]
pub fn pio_detect() -> Result<PioStatus, String> {
    let mut status = PioStatus {
        installed: false,
        version: None,
        python: None,
        pio_path: None,
    };

    match find_pio() {
        Some((ref exe, ref base_args)) => {
            let mut cmd = Command::new(exe);
            cmd.args(base_args);
            cmd.arg("--version");

            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    status.installed = true;
                    status.pio_path = Some(if base_args.is_empty() { exe.clone() } else { format!("{} {}", exe, base_args.join(" ")) });
                    status.version = Some(
                        String::from_utf8_lossy(&output.stdout).trim().to_string(),
                    );
                }
            }
        }
        None => {}
    }

    // Check python version (needed for pip install)
    for py in &["python", "python3", "py"] {
        if let Ok(output) = Command::new(py).arg("--version").output() {
            if output.status.success() {
                status.python = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
                break;
            }
        }
    }

    Ok(status)
}

#[tauri::command]
pub fn pio_install() -> Result<PioResult, String> {
    let pip_cmd = if cfg!(target_os = "windows") { "pip" } else { "pip3" };

    let result = Command::new(pip_cmd)
        .args(["install", "platformio"])
        .output()
        .map_err(|e| format!("Failed to run pip: {}", e))?;

    Ok(PioResult {
        success: result.status.success(),
        output: String::from_utf8_lossy(&result.stdout).to_string(),
        error: if result.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&result.stderr).to_string())
        },
    })
}

#[tauri::command]
pub fn pio_init(path: String, board: Option<String>) -> Result<PioResult, String> {
    let mut args = vec!["project", "init", "-d", &path];

    if let Some(ref b) = board {
        args.extend_from_slice(&["-b", b]);
    }

    let output = run_pio(&args)?;

    Ok(PioResult {
        success: output.status.success(),
        output: String::from_utf8_lossy(&output.stdout).to_string(),
        error: if output.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        },
    })
}

#[tauri::command]
pub fn pio_run(target: String, directory: String) -> Result<PioResult, String> {
    let args = ["run", "-d", &directory, "-t", &target];
    let output = run_pio(&args)?;

    Ok(PioResult {
        success: output.status.success(),
        output: String::from_utf8_lossy(&output.stdout).to_string(),
        error: if output.status.success() {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        },
    })
}

#[tauri::command]
pub fn pio_list_boards(search: Option<String>) -> Result<Vec<String>, String> {
    let args = ["boards"];
    let output = run_pio(&args)?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let out = String::from_utf8_lossy(&output.stdout);
    let mut boards: Vec<String> = Vec::new();

    for line in out.lines().skip(2) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(ref search_term) = search {
            if line.to_lowercase().contains(&search_term.to_lowercase()) {
                boards.push(line.to_string());
            }
        } else {
            boards.push(line.to_string());
        }
    }

    Ok(boards)
}
