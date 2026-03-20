//! Librarian: terminal state observer.
//!
//! Two-tier detection:
//! 1. Score table — fast keyword matching for launch-time detection (no LLM)
//! 2. LLM (cli-ai-analyzer / Gemini) — for wait/status judgment

use crate::error::{AgentCtlError, Result};
use std::collections::HashMap;
use std::process::Command;

/// Possible agent states, ordered by lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentState {
    /// No agent running. Bare shell prompt visible.
    ShellIdle,
    /// Shell command running (not an agent).
    ShellBusy,
    /// Agent is loading/initializing.
    AgentStarting,
    /// Agent launched, waiting for FIRST task. No prior results visible.
    AgentReady,
    /// Agent is actively executing a task.
    AgentWorking,
    /// Agent paused, asking for user permission.
    AgentApproval,
    /// Agent completed a task. Results visible, input prompt returned.
    AgentDone,
    /// User interrupted the agent mid-task.
    AgentInterrupted,
    /// Agent crashed or hit an error.
    AgentError,
    /// Could not determine the state.
    Unknown,
}

/// Longest tags first to prevent prefix collision in extract().
const ALL_STATES: &[(&str, AgentState)] = &[
    ("AGENT_INTERRUPTED", AgentState::AgentInterrupted),
    ("AGENT_STARTING", AgentState::AgentStarting),
    ("AGENT_APPROVAL", AgentState::AgentApproval),
    ("AGENT_WORKING", AgentState::AgentWorking),
    ("AGENT_READY", AgentState::AgentReady),
    ("AGENT_ERROR", AgentState::AgentError),
    ("AGENT_DONE", AgentState::AgentDone),
    ("SHELL_IDLE", AgentState::ShellIdle),
    ("SHELL_BUSY", AgentState::ShellBusy),
    ("UNKNOWN", AgentState::Unknown),
];

impl AgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ShellIdle => "SHELL_IDLE",
            Self::ShellBusy => "SHELL_BUSY",
            Self::AgentStarting => "AGENT_STARTING",
            Self::AgentReady => "AGENT_READY",
            Self::AgentWorking => "AGENT_WORKING",
            Self::AgentApproval => "AGENT_APPROVAL",
            Self::AgentDone => "AGENT_DONE",
            Self::AgentInterrupted => "AGENT_INTERRUPTED",
            Self::AgentError => "AGENT_ERROR",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Extract state from LLM response. Tolerates markdown noise, extra text, etc.
    pub fn extract(response: &str) -> Self {
        let upper = response.to_uppercase();
        // Longest tags first, so find first match (no prefix collision)
        for &(keyword, state) in ALL_STATES {
            if upper.contains(keyword) {
                return state;
            }
        }
        Self::Unknown
    }

    /// Is this a "terminal is waiting" state where the controller can act?
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            Self::ShellIdle
                | Self::AgentReady
                | Self::AgentDone
                | Self::AgentApproval
                | Self::AgentInterrupted
                | Self::AgentError
        )
    }
}

/// LLM judgment result.
#[derive(Debug)]
pub struct Judgment {
    pub state: AgentState,
    pub raw_response: String,
    pub buffer: String,
}

impl Judgment {
    /// Return the last N lines of the buffer that was judged.
    pub fn context(&self, n: usize) -> Vec<&str> {
        self.buffer
            .lines()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

// ─── Score table: fast launch-time detection (NO LLM) ───

const SCORE_TABLE: &[(&str, AgentState, i32)] = &[
    // APPROVAL
    ("Allow once", AgentState::AgentApproval, 100),
    ("Allow always", AgentState::AgentApproval, 100),
    ("Action Required", AgentState::AgentApproval, 100),
    ("Would you like to run", AgentState::AgentApproval, 100),
    ("Yes, proceed", AgentState::AgentApproval, 80),
    // WORKING
    ("Working (", AgentState::AgentWorking, 100),
    ("esc to interrupt", AgentState::AgentWorking, 80),
    ("Thinking...", AgentState::AgentWorking, 90),
    // DONE indicators
    ("\u{2022} Ran ", AgentState::AgentDone, 80),
    ("\u{2022} Edit ", AgentState::AgentDone, 80),
    ("\u{2022} Write ", AgentState::AgentDone, 70),
    // READY indicators
    ("\u{203a} ", AgentState::AgentReady, 50),
    ("% left", AgentState::AgentReady, 50),
    ("> Type your message", AgentState::AgentReady, 80),
    ("OpenAI Codex", AgentState::AgentReady, 40),
    // Shared prompt -> also DONE (lower weight)
    ("\u{203a} ", AgentState::AgentDone, 30),
    ("% left", AgentState::AgentDone, 30),
    ("> Type your message", AgentState::AgentDone, 40),
    // INTERRUPTED
    ("Interrupted", AgentState::AgentInterrupted, 100),
    // ERROR
    ("panicked at", AgentState::AgentError, 100),
    ("Error:", AgentState::AgentError, 60),
    // SHELL
    ("$ ", AgentState::ShellIdle, 50),
    ("PS C:\\", AgentState::ShellIdle, 60),
    ("Compiling ", AgentState::ShellBusy, 60),
];

fn score_buffer(buffer: &str) -> Vec<(AgentState, i32)> {
    let mut totals: HashMap<AgentState, i32> = HashMap::new();
    for &(keyword, state, score) in SCORE_TABLE {
        if buffer.contains(keyword) {
            *totals.entry(state).or_insert(0) += score;
        }
    }
    // Agent UI present -> suppress SHELL states
    let has_agent = totals.contains_key(&AgentState::AgentReady)
        || totals.contains_key(&AgentState::AgentDone)
        || totals.contains_key(&AgentState::AgentWorking)
        || totals.contains_key(&AgentState::AgentApproval);
    if has_agent {
        totals.remove(&AgentState::ShellIdle);
        totals.remove(&AgentState::ShellBusy);
    }
    totals.into_iter().collect()
}

/// Fast score-based state detection. No LLM call.
/// Returns the top-scoring state and its score, or None if no keywords matched.
pub fn judge_by_score(buffer: &str) -> Option<(AgentState, i32)> {
    let scores = score_buffer(buffer);
    let mut ranked: Vec<(AgentState, i32)> = scores.into_iter().filter(|(_, s)| *s > 0).collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    if let Some(&(state, score)) = ranked.first() {
        let second = ranked.get(1).map(|(_, s)| *s).unwrap_or(0);
        eprintln!("[score] {}={} (2nd={})", state.as_str(), score, second);
        Some((state, score))
    } else {
        None
    }
}

// ─── LLM-based judgment ───

const STATE_DEFINITIONS: &str = r#"状態定義（必ずこの中から1つ選べ）:
- SHELL_IDLE: エージェントが起動していない。シェルプロンプト（$ や PS> や >）だけが見える。エージェント固有のUIは一切ない。
- SHELL_BUSY: シェルコマンドが実行中。ビルドやping等の出力が流れている。エージェントではない。
- AGENT_STARTING: エージェントが起動中。バナー、ロード表示、初期化メッセージが出ている。まだ入力を受け付けていない。
- AGENT_READY: エージェントが起動したばかりで、まだ一度もタスクを実行していない。入力プロンプトが見えるが、上にコード・説明文・ツール実行結果など作業の痕跡が一切ない。ウェルカムメッセージや初期バナーだけ。
- AGENT_WORKING: エージェントがタスクを実行中。入力プロンプトはまだ戻っていない。
- AGENT_APPROVAL: エージェントが許可を求めて停止している。承認プロンプトが見える。
- AGENT_DONE: エージェントがタスクを完了して入力プロンプトに戻っている。入力プロンプトの上に作業結果がある。READYとの違い：DONEは作業結果が見える、READYは見えない。
- AGENT_INTERRUPTED: ユーザーがエージェントを中断した。「Interrupted」メッセージが見える。
- AGENT_ERROR: エージェントがクラッシュまたはエラーで停止。パニックやエラーメッセージが見える。

エージェント固有パターン:
[Codex]
  AGENT_WORKING: 「Working (Ns • esc to interrupt)」が見える
  AGENT_APPROVAL: 「Would you like to run」「Yes, proceed」が見える
  AGENT_READY: 最終行付近に「› 」プロンプトと「N% left」。上にウェルカムバナーのみ。「• Ran」等の作業ログなし。バナーのbox（╭╰）はWORKINGではない
  AGENT_DONE: 「› 」プロンプトと「N% left」。上に「• Ran」「• Edit」「• Write」等の作業ログがある
  AGENT_INTERRUPTED: 「Interrupted.」＋プロンプト帰還
[Gemini]
  AGENT_WORKING: 「✦ I'll」「Thinking...」、box UI（╭─...─╮）内にコマンド実行中
  AGENT_APPROVAL: 「Action Required」「Allow execution」
  AGENT_READY: 「> Type your message」のみ、作業ログなし
  AGENT_DONE: 「> Type your message」あり、上に回答文・コード・分析結果がある
[Claude Code]
  AGENT_WORKING: ツール実行box（╭─ Read/Write/Bash ─╮）が表示中
  AGENT_APPROVAL: 「Allow once」「Allow always」の選択肢が見える
  AGENT_READY: 「> 」プロンプトのみ、バージョンバナーが残る程度
  AGENT_DONE: 「> 」あり、上にツール実行結果・コード変更のログがある

判定の優先順:
1. 許可キーワード（Allow/Approve/Would you like to run） → AGENT_APPROVAL
2. 作業中キーワード（Working/Running/Thinking/✦/╭─） → AGENT_WORKING
3. 中断（Interrupted） → AGENT_INTERRUPTED
4. エラー（panicked/Error/crash） → AGENT_ERROR
5. プロンプト + 作業結果あり → AGENT_DONE
6. プロンプト + 作業結果なし → AGENT_READY
7. シェルコマンド実行中 → SHELL_BUSY
8. エージェントUIなし + シェルプロンプト → SHELL_IDLE"#;

/// Ask the LLM to determine agent state from a terminal buffer.
/// Uses --pay-per-use (Gemini REST API) for speed and to avoid CLI shell quoting issues.
/// Buffer is limited to 20 lines by the caller, keeping the prompt well under limits.
pub fn judge(buffer: &str) -> Result<Judgment> {
    let prompt = format!(
        "ターミナルバッファの末尾を見て、エージェントの状態を判定しろ。\n\n\
         {STATE_DEFINITIONS}\n\n\
         バッファ末尾:\n---\n{buffer}\n---\n\n\
         上記の状態から1つだけ選び、状態名だけ返せ（例: AGENT_APPROVAL）。それ以外は何も書くな。"
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
        buffer: buffer.to_string(),
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
    let raw_tail = backend.read(session_hint, lines, None)?;
    // Strip TAIL header line if present
    let buffer = if let Some(idx) = raw_tail.find('\n') {
        &raw_tail[idx + 1..]
    } else {
        &raw_tail
    };
    judge(buffer)
}
