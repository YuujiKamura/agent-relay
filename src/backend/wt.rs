use crate::backend::{AgentBackend, AgentStatus};
use crate::error::{AgentCtlError, Result};
use crate::pipe;
use crate::protocol::{self, AgentStatusResponse};
use crate::session::{self, SessionInfo};
use std::time::{Duration, Instant};

pub struct WtBackend;

impl AgentBackend for WtBackend {
    fn list(&self) -> Result<Vec<SessionInfo>> {
        Ok(session::discover_sessions())
    }

    fn status(&self, session_hint: &str) -> Result<AgentStatus> {
        let s = session::find_session(session_hint)?;
        let resp = pipe::send_pipe_message(&s.pipe_path, &protocol::agent_status())?;
        let parsed = AgentStatusResponse::parse(&resp)
            .ok_or_else(|| AgentCtlError::Protocol(format!("Failed to parse status response: {}", resp)))?;
        
        Ok(AgentStatus {
            name: parsed.session,
            status: parsed.status,
            ms_since_change: parsed.ms_since_change,
            tab: parsed.tab,
        })
    }

    fn send(&self, session_hint: &str, text: &str) -> Result<()> {
        let s = session::find_session(session_hint)?;
        // Step 1: Send text via INPUT (bracketed paste)
        let msg = protocol::input("agent-ctl", text);
        let response = pipe::send_pipe_message(&s.pipe_path, &msg)?;
        if let Some(err) = protocol::is_error(&response) {
            return Err(AgentCtlError::ServerError(err));
        }
        // Step 2: Wait for TUI to process INPUT before sending Enter
        std::thread::sleep(Duration::from_secs(1));
        // Step 3: Send Enter via RAW_INPUT (separate from INPUT so TUI treats it as submit)
        let enter_msg = protocol::raw_input("agent-ctl", "\r");
        let response2 = pipe::send_pipe_message(&s.pipe_path, &enter_msg)?;
        if let Some(err) = protocol::is_error(&response2) {
            return Err(AgentCtlError::ServerError(err));
        }
        Ok(())
    }

    fn read(&self, session_hint: &str, lines: usize) -> Result<String> {
        let s = session::find_session(session_hint)?;
        let msg = protocol::tail(lines);
        let response = pipe::send_pipe_message(&s.pipe_path, &msg)?;
        if let Some(err) = protocol::is_error(&response) {
            return Err(AgentCtlError::ServerError(err));
        }
        Ok(response)
    }

    fn wait(&self, session_hint: &str, timeout_secs: u64, auto_approve: bool) -> Result<()> {
        let s = session::find_session(session_hint)?;
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let poll_interval = Duration::from_millis(3000);

        // State machine: WORKING → IDLE → DONE (after 30s idle)
        //                WORKING → APPROVAL → (send once) → wait for !APPROVAL → WORKING
        let mut approval_sent = false; // true = already sent, waiting for APPROVAL to clear

        loop {
            if Instant::now() > deadline {
                return Err(AgentCtlError::WaitTimeout(timeout_secs));
            }

            let response = match pipe::send_pipe_message(&s.pipe_path, &protocol::agent_status()) {
                Ok(r) => r,
                Err(_) => {
                    // Bug #3: reset consecutive_idle on pipe error (now removed, but keep approval_sent)
                    std::thread::sleep(poll_interval);
                    continue;
                }
            };

            let Some(status) = AgentStatusResponse::parse(&response) else {
                // Bug #3: reset consecutive_idle on parse error (now removed, but keep approval_sent)
                std::thread::sleep(poll_interval);
                continue;
            };

            match status.status.as_str() {
                // ── APPROVAL: send once, wait for state transition ──
                "APPROVAL" => {
                    if !auto_approve {
                        return Err(AgentCtlError::Other(
                            "APPROVAL required but auto_approve is disabled (use --auto-approve)".into(),
                        ));
                    }
                    if !approval_sent {
                        eprintln!("[wait] APPROVAL detected, sending approval...");
                        let approve_msg = protocol::raw_input("agent-ctl", "1");
                        let _ = pipe::send_pipe_message(&s.pipe_path, &approve_msg);
                        approval_sent = true;
                    }
                    // approval_sent=true: do nothing, wait for WORKING/IDLE transition
                }
                // ── IDLE/READY: DONE after 8s sustained idle ──
                "IDLE" | "READY" => {
                    approval_sent = false; // APPROVAL cleared
                    if status.ms_since_change > 8000 {
                        eprintln!("[wait] DONE (idle for {}ms)", status.ms_since_change);
                        return Ok(());
                    }
                }
                // ── WORKING: agent is active, APPROVAL was consumed ──
                "WORKING" | "STARTING" => {
                    approval_sent = false; // state transitioned away from APPROVAL, safe to reset
                }
                other => {
                    eprintln!("[wait] Unknown status: {}", other);
                }
            }

            std::thread::sleep(poll_interval);
        }
    }

    fn approve(&self, session_hint: &str) -> Result<()> {
        let s = session::find_session(session_hint)?;
        let msg = protocol::raw_input("agent-ctl", "y\r");
        let response = pipe::send_pipe_message(&s.pipe_path, &msg)?;
        if let Some(err) = protocol::is_error(&response) {
            return Err(AgentCtlError::ServerError(err));
        }
        Ok(())
    }

    fn tab(&self, session_hint: &str, action: &str, index: Option<usize>) -> Result<String> {
        let s = session::find_session(session_hint)?;
        let msg = match action {
            "new" => protocol::new_tab(),
            "switch" => {
                let idx = index.expect("tab switch requires an index");
                protocol::switch_tab(idx)
            }
            "close" => protocol::close_tab(index),
            "list" => protocol::list_tabs(),
            other => return Err(AgentCtlError::Other(format!("Unknown tab action: {}", other))),
        };

        let response = pipe::send_pipe_message(&s.pipe_path, &msg)?;
        if let Some(err) = protocol::is_error(&response) {
            return Err(AgentCtlError::ServerError(err));
        }
        Ok(response)
    }

    fn launch(&self, session_hint: &str, agent_type: &str, prompt: Option<&str>) -> Result<()> {
        let s = session::find_session(session_hint)?;

        // Step 1: Register agent type with SET_AGENT
        let status_resp = pipe::send_pipe_message(&s.pipe_path, &protocol::agent_status())?;
        let tab_idx = AgentStatusResponse::parse(&status_resp)
            .map(|st| st.tab)
            .unwrap_or(0);

        let set_msg = protocol::set_agent(tab_idx, agent_type);
        let _ = pipe::send_pipe_message(&s.pipe_path, &set_msg);

        // Step 2: Launch "bash" + Enter
        let bash_msg = protocol::raw_input("agent-ctl", "bash\r");
        let _ = pipe::send_pipe_message(&s.pipe_path, &bash_msg);
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Step 3: Launch agent command + Enter
        let agent_cmd = match agent_type {
            "claude" => "claude",
            "gemini" => "gemini",
            "codex" => "codex",
            other => other,
        };
        let launch_msg = protocol::raw_input("agent-ctl", &format!("{}\r", agent_cmd));
        let _ = pipe::send_pipe_message(&s.pipe_path, &launch_msg);

        // Step 4: Wait for READY status
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if std::time::Instant::now() > deadline {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
            if let Ok(resp) = pipe::send_pipe_message(&s.pipe_path, &protocol::agent_status()) {
                if let Some(st) = AgentStatusResponse::parse(&resp) {
                    if st.status == "READY" || st.status == "IDLE" {
                        break;
                    }
                }
            }
        }

        // Step 5: Send prompt if provided
        if let Some(prompt_text) = prompt {
            let input_msg = protocol::input("agent-ctl", prompt_text);
            let _ = pipe::send_pipe_message(&s.pipe_path, &input_msg);

            let enter_msg = protocol::raw_input("agent-ctl", "\r");
            let _ = pipe::send_pipe_message(&s.pipe_path, &enter_msg);
        }

        Ok(())
    }

    fn ping(&self, session_hint: &str) -> Result<String> {
        let s = session::find_session(session_hint)?;
        let resp = pipe::send_pipe_message(&s.pipe_path, &protocol::ping())?;
        Ok(resp)
    }

    fn raw_send(&self, session_hint: &str, text: &str) -> Result<()> {
        let s = session::find_session(session_hint)?;
        let msg = protocol::raw_input("agent-ctl", text);
        let response = pipe::send_pipe_message(&s.pipe_path, &msg)?;
        if let Some(err) = protocol::is_error(&response) {
            return Err(AgentCtlError::ServerError(err));
        }
        Ok(())
    }

    fn state(&self, session_hint: &str) -> Result<String> {
        let s = session::find_session(session_hint)?;
        let resp = pipe::send_pipe_message(&s.pipe_path, &protocol::state(None))?;
        Ok(resp)
    }

    fn tabs(&self, session_hint: &str) -> Result<String> {
        let s = session::find_session(session_hint)?;
        let resp = pipe::send_pipe_message(&s.pipe_path, &protocol::list_tabs())?;
        Ok(resp)
    }

    fn stop(&self, session_hint: &str, agent_type: &str) -> Result<()> {
        let s = session::find_session(session_hint)?;

        match agent_type {
            "claude" => {
                let ctrl_c = protocol::raw_input("agent-ctl", "\x03");
                let _ = pipe::send_pipe_message(&s.pipe_path, &ctrl_c);
                std::thread::sleep(std::time::Duration::from_millis(500));
                let _ = pipe::send_pipe_message(&s.pipe_path, &ctrl_c);
            }
            "gemini" => {
                let ctrl_c = protocol::raw_input("agent-ctl", "\x03");
                let _ = pipe::send_pipe_message(&s.pipe_path, &ctrl_c);
            }
            "codex" => {
                let exit_msg = protocol::raw_input("agent-ctl", "/exit\r");
                let _ = pipe::send_pipe_message(&s.pipe_path, &exit_msg);
            }
            _ => {
                let ctrl_c = protocol::raw_input("agent-ctl", "\x03");
                let _ = pipe::send_pipe_message(&s.pipe_path, &ctrl_c);
            }
        }
        Ok(())
    }
}
