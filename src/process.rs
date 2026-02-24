use std::process::{Command, Stdio};

pub fn launch_split_command(command_line: &str, debug: bool) -> Result<(), String> {
    let parts: Vec<&str> = command_line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    let mut cmd = Command::new(parts[0]);
    cmd.args(&parts[1..]).stdin(Stdio::null());

    if debug {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Error launching '{command_line}': {e}"))
}

pub fn run_shell_status(command: &str) -> Result<bool, String> {
    if command.trim().is_empty() {
        return Err("Status check command is empty".to_string());
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("Failed to run status check '{command}': {err}"))?;

    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_status_command_is_rejected() {
        let err = run_shell_status("   ").expect_err("blank status command should fail");
        assert!(err.contains("empty"));
    }

    #[test]
    fn shell_status_reports_success_and_failure() {
        assert!(run_shell_status("true").expect("true should run"));
        assert!(!run_shell_status("false").expect("false should run"));
    }

    #[test]
    fn empty_launch_command_is_noop() {
        launch_split_command("   ", false).expect("blank launch command should be a no-op");
    }

    #[test]
    fn launch_errors_are_reported() {
        let err = launch_split_command("streamrs-test-command-that-should-not-exist", false)
            .expect_err("missing executable should return an error");
        assert!(err.contains("Error launching"));
    }
}
