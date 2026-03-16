use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(backend: &dyn AgentBackend, session_hint: &str) -> Result<()> {
    backend.approve(session_hint)?;
    eprintln!("Approved.");
    Ok(())
}
