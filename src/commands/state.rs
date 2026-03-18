use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(backend: &dyn AgentBackend, session_hint: &str) -> Result<()> {
    let response = backend.state(session_hint)?;
    println!("{}", response.trim());
    Ok(())
}
