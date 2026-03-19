use crate::backend::AgentBackend;
use crate::error::Result;
use crate::librarian;

pub fn run(backend: &dyn AgentBackend, session_hint: &str, use_librarian: bool) -> Result<()> {
    if use_librarian {
        let judgment = librarian::observe(backend, session_hint, 20)?;
        println!("LIBRARIAN|{}|{}", judgment.state.as_str(), judgment.raw_response.trim());
        return Ok(());
    }

    let status = backend.status(session_hint)?;
    println!(
        "AGENT_STATUS|{}|{}|{}|tab={}",
        status.name, status.status, status.ms_since_change, status.tab
    );
    Ok(())
}
