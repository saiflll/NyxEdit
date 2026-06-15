pub fn sys_check_installed(cmd: String) -> bool {
    let mut check_cmd = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("powershell.exe");
        c.args(&["-Command", &format!("Get-Command {} -ErrorAction SilentlyContinue", cmd)]);
        c
    } else {
        let mut c = std::process::Command::new("which");
        c.arg(&cmd);
        c
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        check_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    check_cmd.status().map(|s| s.success()).unwrap_or(false)
}

pub fn sys_run_diagnostics(cmd_type: String, directory: String) -> Result<String, String> {
    let mut command = if cfg!(target_os = "windows") {
        std::process::Command::new("powershell.exe")
    } else {
        std::process::Command::new("sh")
    };

    if cfg!(target_os = "windows") {
        command.arg("-NoProfile");
        command.arg("-Command");
    } else {
        command.arg("-c");
    }

    let cmd_str = match cmd_type.as_str() {
        "rust" | "cargo" | "cargo-check" => "cargo check",
        "node" | "npm" | "npm-check" => "npm run check",
        "platformio" | "pio" => "pio run",
        "go" | "golang" | "go-build" => "go build ./...",
        _ => return Err(format!("Unknown diagnostics command type: {}", cmd_type)),
    };

    let full_cmd = if cfg!(target_os = "windows") {
        format!("cd '{}'; {}", directory, cmd_str)
    } else {
        format!("cd \"{}\" && {}", directory, cmd_str)
    };

    command.arg(&full_cmd);

    // Hide console window on Windows
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = command.output().map_err(|e| format!("Failed to run command: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let combined = format!("{}\n{}", stdout, stderr);
    Ok(combined)
}

