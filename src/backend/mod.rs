pub mod wt;
// Future backends:
// pub mod tmux;
// pub mod ssh;

use crate::error::Result;
use crate::session::SessionInfo;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStatus {
    pub name: String,
    pub status: String,
    pub ms_since_change: u64,
    pub tab: usize,
}

pub trait AgentBackend {
    fn list(&self) -> Result<Vec<SessionInfo>>;
    fn status(&self, session_hint: &str) -> Result<AgentStatus>;
    fn send(&self, session_hint: &str, text: &str) -> Result<()>;
    fn read(&self, session_hint: &str, lines: usize) -> Result<String>;
    fn wait(&self, session_hint: &str, timeout_secs: u64, auto_approve: bool) -> Result<()>;
    fn approve(&self, session_hint: &str) -> Result<()>;
    fn tab(&self, session_hint: &str, action: &str, index: Option<usize>) -> Result<String>;
    fn launch(&self, session_hint: &str, agent_type: &str, prompt: Option<&str>) -> Result<()>;
    fn stop(&self, session_hint: &str, agent_type: &str) -> Result<()>;
    /// Send PING, return raw response string
    fn ping(&self, session_hint: &str) -> Result<String>;
    /// Send RAW_INPUT with text (no Enter appended)
    fn raw_send(&self, session_hint: &str, text: &str) -> Result<()>;
    /// Send STATE request, return raw response
    fn state(&self, session_hint: &str) -> Result<String>;
    /// Send LIST_TABS request, return raw response
    fn tabs(&self, session_hint: &str) -> Result<String>;
}
