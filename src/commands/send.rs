use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(backend: &dyn AgentBackend, session_hint: &str, text: &str, enter: bool) -> Result<()> {
    backend.send(session_hint, text)?;
    if enter {
        backend.raw_send(session_hint, "\r")?;
    }
    eprintln!("Sent.");
    Ok(())
}
