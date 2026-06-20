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
fn find_python_on_windows() -> Option<String> {
    if cfg!(target_os = "windows") {
        // 1. Check if "python" in PATH works
        if let Ok(output) = Command::new("python").arg("--version").output() {
            if output.status.success() {
                return Some("python".to_string());
            }
        }
        // 2. Look in AppData\Local\Programs\Python\Python3*
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let python_dir = format!("{}\\AppData\\Local\\Programs\\Python", user_profile);
            if let Ok(entries) = std::fs::read_dir(&python_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let exe_path = path.join("python.exe");
                        if exe_path.exists() {
                            return Some(exe_path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        // 3. Look in Program Files
        for program_files in &["C:\\Program Files", "C:\\Program Files (x86)"] {
            if let Ok(entries) = std::fs::read_dir(program_files) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("Python3") {
                        let exe_path = entry.path().join("python.exe");
                        if exe_path.exists() {
                            return Some(exe_path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

fn get_python_cmd() -> String {
    for py in &["python", "python3", "py"] {
        if let Ok(output) = Command::new(py).arg("--version").output() {
            if output.status.success() {
                let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let ver = if !stdout_str.is_empty() { stdout_str } else { stderr_str };
                if !ver.is_empty() {
                    return py.to_string();
                }
            }
        }
    }
    if cfg!(target_os = "windows") {
        if let Some(path) = find_python_on_windows() {
            return path;
        }
    }
    "python".to_string()
}

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

    // 3. Try `python -m platformio` using the detected python cmd
    let py = get_python_cmd();
    if let Ok(output) = Command::new(&py)
        .args(["-m", "platformio", "--version"])
        .output()
    {
        if output.status.success() {
            return Some((py, vec!["-m".to_string(), "platformio".to_string()]));
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
    let py = get_python_cmd();
    if let Ok(output) = Command::new(&py).arg("--version").output() {
        let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let ver = if !stdout_str.is_empty() { stdout_str } else { stderr_str };
        if !ver.is_empty() {
            status.python = Some(ver);
        }
    }

    Ok(status)
}

#[tauri::command]
pub fn pio_install() -> Result<PioResult, String> {
    let py = get_python_cmd();

    let result = Command::new(&py)
        .args(["-m", "pip", "install", "platformio"])
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
