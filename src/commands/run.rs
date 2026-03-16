use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    prompt: &str,
    timeout_secs: u64,
    auto_approve: bool,
    lines: usize,
) -> Result<()> {
    backend.send(session_hint, prompt)?;
    eprintln!("Sent.");
    backend.wait(session_hint, timeout_secs, auto_approve)?;
    eprintln!("[wait] Done.");
    let output = backend.read(session_hint, lines)?;
    if let Some(idx) = output.find('\n') {
        print!("{}", &output[idx + 1..]);
    } else {
        print!("{}", output);
    }
    Ok(())
}
