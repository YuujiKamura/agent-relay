use crate::backend::AgentBackend;
use crate::error::Result;
use crate::librarian;

pub fn run(backend: &dyn AgentBackend, session_hint: &str) -> Result<()> {
    let judgment = librarian::observe(backend, session_hint, 20)?;
    println!("{}|{}", judgment.state.as_str(), judgment.raw_response.trim());
    for line in judgment.context(5) {
        println!("  {}", line);
    }
    Ok(())
}
