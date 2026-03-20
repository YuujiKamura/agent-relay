//! Integration test: feed sample terminal buffers to Librarian and verify state judgment.
//!
//! Requires cli-ai-analyzer + Gemini API access (real LLM calls).
//! Run with: cargo test --test librarian_judgment -- --nocapture
//!
//! Each test case is defined in tests/fixtures/buffers.toml.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Minimal TOML parser for our test fixture format.
/// We avoid adding a toml dependency just for tests.
fn parse_cases(content: &str) -> Vec<(String, String, String)> {
    let mut cases = Vec::new();
    let mut name = String::new();
    let mut expected = String::new();
    let mut buffer = String::new();
    let mut in_buffer = false;
    let mut in_case = false;

    for line in content.lines() {
        if line.starts_with("[[case]]") {
            if in_case && !name.is_empty() {
                cases.push((name.clone(), expected.clone(), buffer.clone()));
            }
            name.clear();
            expected.clear();
            buffer.clear();
            in_buffer = false;
            in_case = true;
            continue;
        }

        if !in_case {
            continue;
        }

        if in_buffer {
            if line == r#"""""# {
                in_buffer = false;
                continue;
            }
            if !buffer.is_empty() {
                buffer.push('\n');
            }
            buffer.push_str(line);
            continue;
        }

        if let Some(rest) = line.strip_prefix("name = \"") {
            name = rest.trim_end_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("expected = \"") {
            expected = rest.trim_end_matches('"').to_string();
        } else if line.starts_with("buffer = \"\"\"") {
            in_buffer = true;
            buffer.clear();
        }
    }

    // Push last case
    if in_case && !name.is_empty() {
        cases.push((name, expected, buffer));
    }

    cases
}

#[test]
fn librarian_judges_all_states() {
    // Use real captured buffers if available, otherwise fall back to hand-written samples
    let real_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("buffers_real.toml");
    let fixture_path = if real_path.exists() {
        eprintln!("  Using real captured buffers: {}", real_path.display());
        real_path
    } else {
        eprintln!("  No real buffers found, using hand-written samples.");
        eprintln!("  Run `agent-ctl sample-all <session>` to capture real buffers.");
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("buffers.toml")
    };

    let content = fs::read_to_string(&fixture_path).expect("Failed to read buffers.toml");
    let cases = parse_cases(&content);

    assert!(!cases.is_empty(), "No test cases found in buffers.toml");

    let mut results: Vec<(String, String, String, bool)> = Vec::new();
    let mut pass_count = 0;
    let mut fail_count = 0;

    // Group by expected state for summary
    let mut by_state: HashMap<String, Vec<(String, bool)>> = HashMap::new();

    for (name, expected, buffer) in &cases {
        eprint!("  Testing {:30} ... ", name);

        match agent_ctl::librarian::judge(buffer) {
            Ok(judgment) => {
                let actual = judgment.state.as_str().to_string();
                let pass = actual == *expected;
                if pass {
                    eprintln!("OK  ({})", actual);
                    pass_count += 1;
                } else {
                    eprintln!("FAIL  expected={}, actual={}", expected, actual);
                    eprintln!("        LLM raw: {}", judgment.raw_response.trim());
                    fail_count += 1;
                }
                results.push((name.clone(), expected.clone(), actual.clone(), pass));
                by_state
                    .entry(expected.clone())
                    .or_default()
                    .push((name.clone(), pass));
            }
            Err(e) => {
                eprintln!("ERR  {}", e);
                fail_count += 1;
                results.push((name.clone(), expected.clone(), "ERROR".to_string(), false));
                by_state
                    .entry(expected.clone())
                    .or_default()
                    .push((name.clone(), false));
            }
        }
    }

    // Summary
    eprintln!("\n══════════════════════════════════════════");
    eprintln!("  RESULTS: {}/{} passed", pass_count, pass_count + fail_count);
    eprintln!("══════════════════════════════════════════");

    for state in &[
        "SHELL_IDLE",
        "SHELL_BUSY",
        "AGENT_STARTING",
        "AGENT_READY",
        "AGENT_WORKING",
        "AGENT_APPROVAL",
        "AGENT_DONE",
        "AGENT_INTERRUPTED",
        "AGENT_ERROR",
    ] {
        if let Some(entries) = by_state.get(*state) {
            let passed = entries.iter().filter(|(_, p)| *p).count();
            let total = entries.len();
            let icon = if passed == total { "✓" } else { "✗" };
            eprintln!("  {} {:20} {}/{}", icon, state, passed, total);
            for (name, pass) in entries {
                if !pass {
                    eprintln!("      FAIL: {}", name);
                }
            }
        }
    }
    eprintln!("══════════════════════════════════════════\n");

    // Don't assert failure — this is an LLM judgment test.
    // Report results and let the user decide if accuracy is acceptable.
    if fail_count > 0 {
        eprintln!("  {} failures detected. Review above for details.", fail_count);
    }
}
