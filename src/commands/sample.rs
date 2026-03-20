//! Sample command: capture real terminal buffers for Librarian test fixtures.
//!
//! Usage:
//!   agent-ctl sample <session> --name codex_ready --state AGENT_READY
//!     → TAIL 20 lines, save to tests/fixtures/buffers.toml
//!
//!   agent-ctl sample <session>
//!     → TAIL 20 lines, run Librarian judgment, print both. No save.
//!
//!   agent-ctl sample <session> --name codex_ready
//!     → TAIL + Librarian auto-label. Save with Librarian's judgment as expected.

use crate::backend::AgentBackend;
use crate::error::Result;
use crate::librarian;
use std::fs;
use std::path::PathBuf;

/// All valid state tags.
const VALID_STATES: &[&str] = &[
    "SHELL_IDLE",
    "SHELL_BUSY",
    "AGENT_STARTING",
    "AGENT_READY",
    "AGENT_WORKING",
    "AGENT_APPROVAL",
    "AGENT_DONE",
    "AGENT_INTERRUPTED",
    "AGENT_ERROR",
    "UNKNOWN",
];

pub fn run(
    backend: &dyn AgentBackend,
    session_hint: &str,
    name: Option<&str>,
    state: Option<&str>,
    lines: usize,
) -> Result<()> {
    // Step 1: TAIL
    let raw_tail = backend.read(session_hint, lines, None)?;
    let buffer = if let Some(idx) = raw_tail.find('\n') {
        &raw_tail[idx + 1..]
    } else {
        &raw_tail
    };

    // Step 2: Librarian judgment
    let judgment = librarian::judge(buffer)?;
    let judged_state = judgment.state.as_str();

    // Step 3: Display
    eprintln!("══════════════════════════════════════════");
    eprintln!("  BUFFER ({} lines):", lines);
    eprintln!("──────────────────────────────────────────");
    for line in buffer.lines() {
        eprintln!("  │ {}", line);
    }
    eprintln!("──────────────────────────────────────────");
    eprintln!("  LIBRARIAN: {}", judged_state);
    eprintln!("  RAW:       {}", judgment.raw_response.trim());
    eprintln!("══════════════════════════════════════════");

    // Step 4: Save if --name given
    let Some(case_name) = name else {
        // No save, just display
        println!("SAMPLE|{}|{}", judged_state, buffer.lines().count());
        return Ok(());
    };

    // Determine expected state: explicit --state overrides Librarian
    let expected = if let Some(s) = state {
        if !VALID_STATES.contains(&s) {
            eprintln!(
                "WARNING: '{}' is not a known state. Valid: {:?}",
                s, VALID_STATES
            );
        }
        s.to_string()
    } else {
        eprintln!("  (no --state given, using Librarian judgment: {})", judged_state);
        judged_state.to_string()
    };

    // Build TOML entry
    let toml_entry = format!(
        "\n[[case]]\nname = \"{}\"\nexpected = \"{}\"\nbuffer = \"\"\"\n{}\"\"\"\n",
        case_name,
        expected,
        buffer,
    );

    // Find buffers.toml
    let fixture_path = find_fixtures_path();
    eprintln!("  Appending to: {}", fixture_path.display());

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&fixture_path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(toml_entry.as_bytes())
        })
        .map_err(|e| crate::error::AgentCtlError::Other(format!("Failed to write fixture: {}", e)))?;

    println!("SAVED|{}|{}|{}", case_name, expected, fixture_path.display());
    eprintln!("  ✓ Saved case '{}' (expected={})", case_name, expected);

    Ok(())
}

fn find_fixtures_path() -> PathBuf {
    // Try relative to crate root first
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = crate_root.join("tests").join("fixtures").join("buffers.toml");
    if fixture.parent().map(|p| p.exists()).unwrap_or(false) {
        return fixture;
    }
    // Fallback: known path
    PathBuf::from(r"C:\Users\yuuji\agent-relay\tests\fixtures\buffers.toml")
}
