use crate::backend::AgentBackend;
use crate::error::{AgentCtlError, Result};
use crate::pipe::send_pipe_message;
use crate::session;
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

/// Find an ALIVE session, or launch the exe and wait for one to appear.
fn ensure_session(
    session_hint: &str,
    exe: Option<&str>,
) -> Result<String> {
    // Try to find an existing alive session (check PID first to avoid pipe hang on DEAD)
    let sessions = session::discover_sessions();
    for s in &sessions {
        if !session::is_process_alive(s.pid) {
            continue;
        }
        if session_hint.is_empty() || s.session_name.contains(session_hint) {
            if send_pipe_message(&s.pipe_path, "PING").is_ok() {
                eprintln!("[run] Found alive session: {}", s.session_name);
                return Ok(s.session_name.clone());
            }
        }
    }

    // No alive session — need exe path to launch
    let exe_path = exe.ok_or_else(|| {
        AgentCtlError::Other(
            "No alive session found. Provide --exe to launch a terminal.".into(),
        )
    })?;

    eprintln!("[run] No alive session. Launching {}...", exe_path);
    use std::os::windows::process::CommandExt;
    // CREATE_NEW_CONSOLE: ghostty gets its own console, survives if agent-ctl dies
    const CREATE_NEW_CONSOLE: u32 = 0x00000010;
    let mut cmd = std::process::Command::new(exe_path);
    cmd.env("GHOSTTY_CONTROL_PLANE", "1");
    cmd.creation_flags(CREATE_NEW_CONSOLE);
    let child = cmd.spawn().map_err(|e| {
        AgentCtlError::Other(format!("Failed to launch {}: {}", exe_path, e))
    })?;
    eprintln!("[run] Launched PID {}", child.id());

    // Poll for alive session
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() > deadline {
            return Err(AgentCtlError::WaitTimeout(30));
        }
        std::thread::sleep(Duration::from_secs(2));
        let sessions = session::discover_sessions();
        for s in &sessions {
            if !session::is_process_alive(s.pid) {
                continue;
            }
            if send_pipe_message(&s.pipe_path, "PING").is_ok() {
                eprintln!("[run] Session appeared: {}", s.session_name);
                return Ok(s.session_name.clone());
            }
        }
    }
}

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    agent: &str,
    task: &str,
    exe: Option<&str>,
) -> Result<()> {
    let (start_cmd, ready_prompt) = agent_config(agent)?;
    let session = ensure_session(session_hint, exe)?;
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    // Step 1: bash + Enter, wait 2s
    eprintln!("[run] Sending bash...");
    backend.raw_send(&session, "bash\r")?;
    std::thread::sleep(Duration::from_secs(2));

    // Step 2: cd to working dir + Enter, wait 2s
    eprintln!("[run] cd {}", cwd);
    backend.raw_send(&session, &format!("cd {}\r", cwd))?;
    std::thread::sleep(Duration::from_secs(2));

    // Step 3: agent start command + Enter
    eprintln!("[run] Starting {}...", agent);
    backend.raw_send(&session, &format!("{}\r", start_cmd))?;

    // Step 4: poll read buffer until ready prompt detected
    eprintln!("[run] Waiting for ready prompt '{}'...", ready_prompt);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if Instant::now() > deadline {
            return Err(AgentCtlError::WaitTimeout(60));
        }
        std::thread::sleep(Duration::from_secs(2));
        if let Ok(buf) = backend.read(&session, 30) {
            if buf.contains(ready_prompt) {
                eprintln!("[run] Ready prompt detected.");
                break;
            }
        }
    }

    // Step 5: send task text + Enter
    eprintln!("[run] Sending task...");
    backend.send(&session, task)?;

    // Step 6: poll state until prompt=1
    eprintln!("[run] Polling state until prompt=1...");
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        if Instant::now() > deadline {
            return Err(AgentCtlError::WaitTimeout(600));
        }
        std::thread::sleep(Duration::from_secs(3));
        if let Ok(state_resp) = backend.state(&session) {
            if state_resp.contains("prompt=1") {
                eprintln!("[run] Done (prompt=1).");
                return Ok(());
            }
        }
    }
}
