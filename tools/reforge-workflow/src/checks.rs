use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::{CheckKind, CheckResult, ChecksArtifact, PlanArtifact};

pub(crate) fn evaluate(
    plan: &PlanArtifact,
    checks: &ChecksArtifact,
    failed: &mut Vec<String>,
    needs_input: &mut Vec<String>,
) {
    if !checks
        .checks
        .iter()
        .any(|check| check.kind == CheckKind::Test && check.success)
    {
        needs_input.push("a successful test check is required".into());
    }
    for required in &plan.required_checks {
        match checks
            .checks
            .iter()
            .rev()
            .find(|check| check.kind == required.kind)
        {
            None => needs_input.push(format!("required {:?} check is missing", required.kind)),
            Some(check) if !check.success => {
                failed.push(format!("required {:?} check failed", required.kind));
            }
            Some(_) => {}
        }
    }
}

pub(crate) fn execute(
    kind: CheckKind,
    timeout_seconds: u64,
    command: &[String],
    root: &Path,
) -> Result<CheckResult> {
    let started = Instant::now();
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to execute {}", command[0]))?;
    let deadline = started + Duration::from_secs(timeout_seconds);
    let mut timed_out = false;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            child.kill()?;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let output = child.wait_with_output()?;
    Ok(CheckResult {
        kind,
        command: command.to_vec(),
        success: output.status.success() && !timed_out,
        exit_code: output.status.code(),
        timed_out,
        duration_ms: started.elapsed().as_millis(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_check_timeout() {
        let result = execute(
            CheckKind::Custom,
            0,
            &["sh".into(), "-c".into(), "sleep 1".into()],
            Path::new("."),
        )
        .unwrap();
        assert!(result.timed_out);
        assert!(!result.success);
    }
}
