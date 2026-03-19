use crate::backend::AgentBackend;
use crate::error::{AgentCtlError, Result};
use crate::librarian::{self, AgentState};
use std::time::{Duration, Instant};

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    timeout_secs: u64,
    auto_approve: bool,
    use_librarian: bool,
) -> Result<()> {
    if use_librarian {
        return run_librarian(backend, session_hint, timeout_secs, auto_approve);
    }
    backend.wait(session_hint, timeout_secs, auto_approve)?;
    eprintln!("[wait] Done.");
    Ok(())
}

/// Wait loop using LLM librarian for state judgment.
fn run_librarian(
    backend: &dyn AgentBackend,
    session_hint: &str,
    timeout_secs: u64,
    auto_approve: bool,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let poll_interval = Duration::from_secs(5);
    let mut approval_sent = false;

    loop {
        if Instant::now() > deadline {
            return Err(AgentCtlError::WaitTimeout(timeout_secs));
        }

        let judgment = match librarian::observe(backend, session_hint, 20) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("[wait/librarian] observe error: {e}");
                std::thread::sleep(poll_interval);
                continue;
            }
        };

        eprintln!("[wait/librarian] state={}", judgment.state.as_str());

        match judgment.state {
            AgentState::Approval => {
                if !auto_approve {
                    return Err(AgentCtlError::Other(
                        "APPROVAL required but auto_approve is disabled (use --auto-approve)"
                            .into(),
                    ));
                }
                if !approval_sent {
                    eprintln!("[wait/librarian] APPROVAL detected, sending approval...");
                    // Send "1" as raw input to approve
                    backend.raw_send(session_hint, "1")?;
                    approval_sent = true;
                }
            }
            AgentState::Done => {
                eprintln!("[wait/librarian] DONE — task completed.");
                return Ok(());
            }
            AgentState::Idle => {
                eprintln!("[wait/librarian] IDLE — agent not running.");
                return Ok(());
            }
            AgentState::Ready => {
                eprintln!("[wait/librarian] READY — agent awaiting task.");
                return Ok(());
            }
            AgentState::Stopped => {
                eprintln!("[wait/librarian] STOPPED — agent interrupted or crashed.");
                return Err(AgentCtlError::Other("Agent stopped unexpectedly".into()));
            }
            AgentState::Working | AgentState::Starting => {
                approval_sent = false;
            }
            AgentState::Unknown => {
                eprintln!("[wait/librarian] UNKNOWN state, continuing...");
            }
        }

        std::thread::sleep(poll_interval);
    }
}
