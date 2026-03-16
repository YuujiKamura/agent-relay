use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(backend: &dyn AgentBackend, session_hint: &str) -> Result<()> {
    let status = backend.status(session_hint)?;
    println!(
        "AGENT_STATUS|{}|{}|{}|tab={}",
        status.name, status.status, status.ms_since_change, status.tab
    );
    Ok(())
}
