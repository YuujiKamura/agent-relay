use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    timeout_secs: u64,
    auto_approve: bool,
) -> Result<()> {
    backend.wait(session_hint, timeout_secs, auto_approve)?;
    eprintln!("[wait] Done.");
    Ok(())
}
