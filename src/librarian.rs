//! Librarian: LLM-based terminal state observer.
//!
//! Reads the terminal buffer and asks an LLM (Gemini via cli-ai-analyzer)
//! to determine the agent's state. No mechanical keyword matching.

use crate::error::{AgentCtlError, Result};
use std::process::Command;

/// Possible agent states, ordered by lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// No agent running. Bare shell prompt visible.
    Idle,
    /// Agent is loading/initializing.
    Starting,
    /// Agent launched, waiting for FIRST task. No prior results visible.
    Ready,
    /// Agent is actively executing a task.
    Working,
    /// Agent paused, asking for user permission.
    Approval,
    /// Agent completed a task. Results visible, input prompt returned.
    Done,
    /// Agent was interrupted or crashed mid-task. Error output or abrupt shell return visible.
    Stopped,
    /// LLM could not determine the state.
    Unknown,
}

const ALL_STATES: &[(&str, AgentState)] = &[
    ("IDLE", AgentState::Idle),
    ("STARTING", AgentState::Starting),
    ("READY", AgentState::Ready),
    ("WORKING", AgentState::Working),
    ("APPROVAL", AgentState::Approval),
    ("DONE", AgentState::Done),
    ("STOPPED", AgentState::Stopped),
];

impl AgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Starting => "STARTING",
            Self::Ready => "READY",
            Self::Working => "WORKING",
            Self::Approval => "APPROVAL",
            Self::Done => "DONE",
            Self::Stopped => "STOPPED",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Extract state from LLM response. Tolerates markdown noise, extra text, etc.
    pub fn extract(response: &str) -> Self {
        let upper = response.to_uppercase();
        // Find which state keyword appears in the response
        let mut found: Option<AgentState> = None;
        for &(keyword, state) in ALL_STATES {
            if upper.contains(keyword) {
                if found.is_some() {
                    // Multiple states found — ambiguous, but prefer the first match
                    // (states are ordered by lifecycle)
                    break;
                }
                found = Some(state);
            }
        }
        found.unwrap_or(Self::Unknown)
    }

    /// Is this a "terminal is waiting" state where the controller can act?
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            Self::Idle | Self::Ready | Self::Approval | Self::Done | Self::Stopped
        )
    }
}

/// LLM judgment result.
#[derive(Debug)]
pub struct Judgment {
    pub state: AgentState,
    pub raw_response: String,
}

const STATE_DEFINITIONS: &str = r#"状態定義（必ずこの中から1つ選べ）:
- IDLE: エージェントが起動していない。シェルプロンプト（$ や PS> や >）だけが見える。エージェント固有のUIは一切ない。
- STARTING: エージェントが起動中。バナー、ロード表示、初期化メッセージが出ている。まだ入力を受け付けていない。
- READY: エージェントが起動したばかりで、まだ一度もタスクを実行していない。入力プロンプトが見えるが、上にコード・説明文・ツール実行結果など作業の痕跡が一切ない。ウェルカムメッセージや初期バナーだけ。
- WORKING: エージェントがタスクを実行中。コード生成、ファイル読み込み、grep実行、「Thinking...」等の出力が流れている。入力プロンプトはまだ戻っていない。
- APPROVAL: エージェントが許可を求めて停止している。「Allow once」「Yes, proceed」「Would you like to run」「Action Required」等の承認プロンプトが見える。
- DONE: エージェントがタスクを完了して入力プロンプトに戻っている。入力プロンプトの上に、タスク実行の痕跡（コード出力、説明文、ファイル操作結果、箇条書きなど）がある。READYとの違い：DONEは作業結果が見える、READYは見えない。
- STOPPED: エージェントが中断された、またはクラッシュした。エラーメッセージや異常終了の痕跡があり、シェルプロンプトに戻っている。タスクは完了していない。

判定のコツ：入力プロンプト（「>」「Type your message」等）が見える場合、その上にタスク実行の結果があればDONE、なければREADY。"#;

/// Ask the LLM to determine agent state from a terminal buffer.
/// Uses --pay-per-use (Gemini REST API) for speed and to avoid CLI shell quoting issues.
/// Buffer is limited to 20 lines by the caller, keeping the prompt well under limits.
pub fn judge(buffer: &str) -> Result<Judgment> {
    let prompt = format!(
        "ターミナルバッファの末尾を見て、エージェントの状態を判定しろ。\n\n\
         {STATE_DEFINITIONS}\n\n\
         バッファ末尾:\n---\n{buffer}\n---\n\n\
         上記の状態から1つだけ選び、状態名だけ返せ（例: APPROVAL）。それ以外は何も書くな。"
    );

    let analyzer = find_analyzer();
    let output = Command::new(&analyzer)
        .args([
            "prompt",
            &prompt,
            "--backend",
            "gemini",
            "--model",
            "gemini-2.5-flash",
        ])
        .output()
        .map_err(|e| AgentCtlError::Other(format!("failed to run cli-ai-analyzer: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AgentCtlError::Other(format!(
            "cli-ai-analyzer failed (exit {}): {}",
            output.status, stderr
        )));
    }

    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let state = AgentState::extract(&raw);

    Ok(Judgment {
        state,
        raw_response: raw,
    })
}

/// Locate cli-ai-analyzer binary. Checks PATH first, then known build locations.
fn find_analyzer() -> String {
    // Check if it's in PATH
    if Command::new("cli-ai-analyzer")
        .arg("--version")
        .output()
        .is_ok()
    {
        return "cli-ai-analyzer".to_string();
    }
    // Known build locations
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\yuuji".to_string());
    let candidates = [
        format!(r"{}\cli-ai-analyzer\target\release\cli-ai-analyzer.exe", home),
        format!(r"{}\cli-ai-analyzer\target\debug\cli-ai-analyzer.exe", home),
    ];
    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.clone();
        }
    }
    "cli-ai-analyzer".to_string()
}

/// Read buffer from backend and judge state.
pub fn observe(
    backend: &dyn crate::backend::AgentBackend,
    session_hint: &str,
    lines: usize,
) -> Result<Judgment> {
    let raw_tail = backend.read(session_hint, lines)?;
    // Strip TAIL header line if present
    let buffer = if let Some(idx) = raw_tail.find('\n') {
        &raw_tail[idx + 1..]
    } else {
        &raw_tail
    };
    judge(buffer)
}
