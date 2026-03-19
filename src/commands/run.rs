use crate::backend::AgentBackend;
use crate::error::{AgentCtlError, Result};
use crate::pipe::send_pipe_message;
use crate::session;
use std::time::{Duration, Instant};

/// Find an ALIVE session, or launch the exe and wait for one to appear.
fn ensure_session(
    session_hint: &str,
    exe: Option<&str>,
) -> Result<String> {
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

    let exe_path = exe.ok_or_else(|| {
        AgentCtlError::Other(
            "No alive session found. Provide --exe to launch a terminal.".into(),
        )
    })?;

    eprintln!("[run] No alive session. Launching {}...", exe_path);
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x00000010;
    let mut cmd = std::process::Command::new(exe_path);
    cmd.env("GHOSTTY_CONTROL_PLANE", "1");
    cmd.creation_flags(CREATE_NEW_CONSOLE);
    let child = cmd.spawn().map_err(|e| {
        AgentCtlError::Other(format!("Failed to launch {}: {}", exe_path, e))
    })?;
    eprintln!("[run] Launched PID {}", child.id());

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
    stop_at: &str,
    use_librarian: bool,
) -> Result<()> {
    // Step 1: Ensure session exists
    let session = ensure_session(session_hint, exe)?;
    println!("LAUNCH | session={}", session);
    if stop_at == "launch" {
        return Ok(());
    }

    // Step 2: Launch agent via backend.launch (SET_AGENT + status-based READY detection)
    eprintln!("[run] Launching agent '{}'...", agent);
    backend.launch(&session, agent, None)?;
    println!("AGENT_READY | session={} | agent={}", session, agent);
    if stop_at == "ready" {
        return Ok(());
    }

    // Step 3: Send task via backend.send (INPUT + Enter)
    eprintln!("[run] Sending task...");
    backend.send(&session, task)?;
    println!("TASK_SET | session={} | agent={}", session, agent);
    if stop_at == "sent" {
        return Ok(());
    }

    // Step 4: Wait for completion
    eprintln!("[run] Waiting for completion (auto-approve enabled)...");
    if use_librarian {
        super::wait::run(backend, &session, 600, true, true)?;
    } else {
        backend.wait(&session, 600, true)?;
    }
    println!("TASK_DONE | session={} | agent={}", session, agent);
    Ok(())
}
