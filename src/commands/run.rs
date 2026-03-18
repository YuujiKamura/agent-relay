use crate::backend::AgentBackend;
use crate::error::{AgentCtlError, Result};
use std::time::{Duration, Instant};

/// Agent configuration: (start_command, ready_prompt)
fn agent_config(agent: &str) -> Result<(&'static str, &'static str)> {
    match agent {
        "claude" => Ok(("claude --max-turns 30", "❯")),
        "gemini" => Ok(("gemini", ">Type")),
        "codex" => Ok(("codex --full-auto", ">")),
        other => Err(AgentCtlError::Other(format!(
            "Unknown agent '{}'. Supported: claude, gemini, codex",
            other
        ))),
    }
}

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    agent: &str,
    task: &str,
) -> Result<()> {
    let (start_cmd, ready_prompt) = agent_config(agent)?;
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    // Step 1: bash + Enter, wait 2s
    eprintln!("[run] Sending bash...");
    backend.raw_send(session_hint, "bash\r")?;
    std::thread::sleep(Duration::from_secs(2));

    // Step 2: cd to working dir + Enter, wait 2s
    eprintln!("[run] cd {}", cwd);
    backend.raw_send(session_hint, &format!("cd {}\r", cwd))?;
    std::thread::sleep(Duration::from_secs(2));

    // Step 3: agent start command + Enter
    eprintln!("[run] Starting {}...", agent);
    backend.raw_send(session_hint, &format!("{}\r", start_cmd))?;

    // Step 4: poll read buffer until ready prompt detected
    eprintln!("[run] Waiting for ready prompt '{}'...", ready_prompt);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if Instant::now() > deadline {
            return Err(AgentCtlError::WaitTimeout(60));
        }
        std::thread::sleep(Duration::from_secs(2));
        if let Ok(buf) = backend.read(session_hint, 30) {
            if buf.contains(ready_prompt) {
                eprintln!("[run] Ready prompt detected.");
                break;
            }
        }
    }

    // Step 5+6: send task text (INPUT bracketed paste) + Enter
    eprintln!("[run] Sending task...");
    backend.send(session_hint, task)?;

    // Step 7: poll state until prompt=1
    eprintln!("[run] Polling state until prompt=1...");
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        if Instant::now() > deadline {
            return Err(AgentCtlError::WaitTimeout(600));
        }
        std::thread::sleep(Duration::from_secs(3));
        if let Ok(state_resp) = backend.state(session_hint) {
            if state_resp.contains("prompt=1") {
                eprintln!("[run] Done (prompt=1).");
                return Ok(());
            }
        }
    }
}
