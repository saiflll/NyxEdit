pub async fn ssh_list_dir(username: String, host: String, port: u16) -> Result<String, String> {
    let port_str = port.to_string();
    let host_arg = format!("{}@{}", username, host);
    
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(&[
        "-p", &port_str,
        "-o", "StrictHostKeyChecking=no",
        "-o", "BatchMode=yes",
        &host_arg,
        "find . -maxdepth 3 -not -path '*/.*'"
    ]);
    
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    
    let output = cmd.output().map_err(|e| format!("Failed to execute ssh command: {}", e))?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(format!("SSH command exited with error: {}", stderr))
    }
}
