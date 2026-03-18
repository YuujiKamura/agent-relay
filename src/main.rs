mod backend;
mod commands;
mod error;
mod pipe;
mod protocol;
mod session;

use backend::{AgentBackend, wt::WtBackend};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agent-ctl", about = "Agent Control Plane CLI")]
struct Cli {
    /// Backend to use: wt
    #[arg(long, default_value = "wt")]
    backend: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all control plane sessions
    List {
        /// Only show alive sessions
        #[arg(long)]
        alive_only: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Get agent status of a session
    Status {
        /// Session name or hint (substring match)
        session: String,
    },
    /// Send text to a session (INPUT + Enter)
    Send {
        /// Session name or hint
        session: String,
        /// Text to send
        text: String,
        /// Also send raw CR after the text
        #[arg(long)]
        enter: bool,
    },
    /// Read terminal output (TAIL)
    Read {
        /// Session name or hint
        session: String,
        /// Number of lines to read
        #[arg(long, default_value = "30")]
        lines: usize,
    },
    /// Wait for session to become idle
    Wait {
        /// Session name or hint
        session: String,
        /// Timeout in seconds
        #[arg(long, default_value = "120")]
        timeout: u64,
        /// Auto-approve when approval is detected
        #[arg(long)]
        auto_approve: bool,
    },
    /// Launch agent in session, send task, wait for completion
    Run {
        /// Session name or hint
        session: String,
        /// Agent type: claude, gemini, codex
        #[arg(long)]
        agent: String,
        /// Task to send to the agent
        task: String,
    },
    /// Send approval (y + Enter) to a session
    Approve {
        /// Session name or hint
        session: String,
    },
    /// Tab management (new, switch, close, list)
    Tab {
        /// Session name or hint
        session: String,
        /// Action: new, switch, close, list
        action: String,
        /// Tab index (required for switch, optional for close)
        index: Option<usize>,
    },
    /// Launch an agent in a session
    Launch {
        /// Session name or hint
        session: String,
        /// Agent type: claude, gemini, codex
        agent_type: String,
        /// Optional initial prompt
        #[arg(long)]
        prompt: Option<String>,
    },
    /// Stop an agent in a session
    Stop {
        /// Session name or hint
        session: String,
        /// Agent type: claude, gemini, codex
        agent_type: String,
    },
    /// Send PING, expect OK response
    Ping {
        /// Session name or hint
        session: String,
    },
    /// Send RAW_INPUT (no Enter appended)
    RawSend {
        /// Session name or hint
        session: String,
        /// Text to send raw
        text: String,
    },
    /// Send STATE request, print response
    State {
        /// Session name or hint
        session: String,
    },
    /// Send LIST_TABS request, print response
    Tabs {
        /// Session name or hint
        session: String,
    },
    /// Remove dead session files from LocalAppData
    Clean,
    /// Automated smoke test (PING + LIST_TABS + STATE)
    Smoke {
        /// Session name or hint
        session: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let backend: Box<dyn AgentBackend> = match cli.backend.as_str() {
        "wt" => Box::new(WtBackend),
        other => {
            eprintln!("Error: Unknown backend '{}'", other);
            std::process::exit(1);
        }
    };

    let result = match cli.command {
        Commands::List { alive_only, json } => commands::list::run(backend.as_ref(), alive_only, json),
        Commands::Status { session } => commands::status::run(backend.as_ref(), &session),
        Commands::Send { session, text, enter } => commands::send::run(backend.as_ref(), &session, &text, enter),
        Commands::Read { session, lines } => commands::read::run(backend.as_ref(), &session, lines),
        Commands::Wait {
            session,
            timeout,
            auto_approve,
        } => commands::wait::run(backend.as_ref(), &session, timeout, auto_approve),
        Commands::Run {
            session,
            agent,
            task,
        } => commands::run::run(backend.as_ref(), &session, &agent, &task),
        Commands::Approve { session } => commands::approve::run(backend.as_ref(), &session),
        Commands::Tab {
            session,
            action,
            index,
        } => commands::tab::run(backend.as_ref(), &session, &action, index),
        Commands::Launch {
            session,
            agent_type,
            prompt,
        } => commands::launch::run(backend.as_ref(), &session, &agent_type, prompt.as_deref()),
        Commands::Stop {
            session,
            agent_type,
        } => commands::stop::run(backend.as_ref(), &session, &agent_type),
        Commands::Ping { session } => commands::ping::run(backend.as_ref(), &session),
        Commands::RawSend { session, text } => commands::raw_send::run(backend.as_ref(), &session, &text),
        Commands::State { session } => commands::state::run(backend.as_ref(), &session),
        Commands::Tabs { session } => commands::tabs::run(backend.as_ref(), &session),
        Commands::Clean => commands::clean::run(),
        Commands::Smoke { session } => commands::smoke::run(backend.as_ref(), &session),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
