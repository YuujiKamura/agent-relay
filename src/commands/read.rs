use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(backend: &dyn AgentBackend, session_hint: &str, lines: usize, tab_index: Option<usize>) -> Result<()> {
    let output = backend.read(session_hint, lines, tab_index)?;
    // TAIL response format: "TAIL|session|lines\n<content>"
    if let Some(idx) = output.find('\n') {
        print!("{}", &output[idx + 1..]);
    } else {
        print!("{}", output);
    }
    Ok(())
}
