use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    action: &str,
    index: Option<usize>,
) -> Result<()> {
    let response = backend.tab(session_hint, action, index)?;
    print!("{}", response);
    Ok(())
}
