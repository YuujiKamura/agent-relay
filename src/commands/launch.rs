use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    agent_type: &str,
    prompt: Option<&str>,
) -> Result<()> {
    eprintln!("Launching {}...", agent_type);
    backend.launch(session_hint, agent_type, prompt)?;
    eprintln!("Launched {} in session {}.", agent_type, session_hint);
    Ok(())
}
