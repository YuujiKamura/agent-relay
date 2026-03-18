use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(backend: &dyn AgentBackend, session_hint: &str, text: &str) -> Result<()> {
    backend.raw_send(session_hint, text)?;
    eprintln!("Sent (raw).");
    Ok(())
}
