use crate::backend::AgentBackend;
use crate::error::Result;

pub fn run(backend: &dyn AgentBackend, session_hint: &str) -> Result<()> {
    let mut pass = 0u32;
    let mut fail = 0u32;

    // Test 1: PING
    print!("PING ... ");
    match backend.ping(session_hint) {
        Ok(resp) => {
            let trimmed = resp.trim();
            if trimmed.contains("OK") || trimmed.starts_with("PONG") {
                println!("PASS ({})", trimmed);
                pass += 1;
            } else {
                println!("FAIL (unexpected: {})", trimmed);
                fail += 1;
            }
        }
        Err(e) => {
            println!("FAIL ({})", e);
            fail += 1;
        }
    }

    // Test 2: LIST_TABS
    print!("LIST_TABS ... ");
    match backend.tabs(session_hint) {
        Ok(resp) => {
            let trimmed = resp.trim();
            // Expect at least 1 tab: response should have pipe-delimited fields
            // or at least be non-empty and not an error
            if trimmed.is_empty() {
                println!("FAIL (empty response)");
                fail += 1;
            } else if trimmed.starts_with("ERR") {
                println!("FAIL ({})", trimmed);
                fail += 1;
            } else {
                // Count tabs: each line or pipe-group is a tab
                let has_content = !trimmed.is_empty();
                if has_content {
                    println!("PASS ({})", trimmed);
                    pass += 1;
                } else {
                    println!("FAIL (no tabs found)");
                    fail += 1;
                }
            }
        }
        Err(e) => {
            println!("FAIL ({})", e);
            fail += 1;
        }
    }

    // Test 3: STATE
    print!("STATE ... ");
    match backend.state(session_hint) {
        Ok(resp) => {
            let trimmed = resp.trim();
            if trimmed.is_empty() {
                println!("FAIL (empty response)");
                fail += 1;
            } else if trimmed.starts_with("ERR") {
                println!("FAIL ({})", trimmed);
                fail += 1;
            } else {
                println!("PASS ({})", trimmed);
                pass += 1;
            }
        }
        Err(e) => {
            println!("FAIL ({})", e);
            fail += 1;
        }
    }

    // Summary
    println!();
    println!("Results: {} passed, {} failed", pass, fail);

    if fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}
