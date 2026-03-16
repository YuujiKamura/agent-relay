use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentCtlError {
    #[error("pipe connection failed: {0}")]
    PipeConnect(String),

    #[error("pipe I/O error: {0}")]
    PipeIo(String),

    #[error("pipe timeout after {0}s")]
    PipeTimeout(u64),

    #[error("no sessions found")]
    NoSessions,

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("session process not alive: pid={0}")]
    SessionDead(u32),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("server error: {0}")]
    ServerError(String),

    #[error("wait timeout after {0}s")]
    WaitTimeout(u64),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AgentCtlError>;
