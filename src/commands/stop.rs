use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    agent_type: &str,
) -> Result<()> {
    backend.stop(session_hint, agent_type)?;
    eprintln!("Stop command sent to {}.", agent_type);
    Ok(())
}
