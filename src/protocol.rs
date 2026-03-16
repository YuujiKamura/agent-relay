use base64::{engine::general_purpose::STANDARD, Engine};

/// Encode text payload as base64 for INPUT/RAW_INPUT commands.
pub fn encode_payload(text: &str) -> String {
    STANDARD.encode(text.as_bytes())
}

/// Build a PING request.
pub fn ping() -> String {
    "PING".to_string()
}

/// Build a STATE request, optionally for a specific tab.
pub fn state(tab_index: Option<usize>) -> String {
    match tab_index {
        Some(idx) => format!("STATE|{}", idx),
        None => "STATE".to_string(),
    }
}

/// Build a TAIL request.
pub fn tail(lines: usize) -> String {
    format!("TAIL|{}", lines)
}

/// Build a LIST_TABS request.
pub fn list_tabs() -> String {
    "LIST_TABS".to_string()
}

/// Build an INPUT request (bracketed paste).
pub fn input(from: &str, text: &str) -> String {
    format!("INPUT|{}|{}", from, encode_payload(text))
}

/// Build a RAW_INPUT request (direct terminal write).
pub fn raw_input(from: &str, text: &str) -> String {
    format!("RAW_INPUT|{}|{}", from, encode_payload(text))
}

/// Build an AGENT_STATUS request.
pub fn agent_status() -> String {
    "AGENT_STATUS".to_string()
}

/// Build a NEW_TAB request.
pub fn new_tab() -> String {
    "NEW_TAB".to_string()
}

/// Build a CLOSE_TAB request.
pub fn close_tab(index: Option<usize>) -> String {
    match index {
        Some(idx) => format!("CLOSE_TAB|{}", idx),
        None => "CLOSE_TAB".to_string(),
    }
}

/// Build a SWITCH_TAB request.
pub fn switch_tab(index: usize) -> String {
    format!("SWITCH_TAB|{}", index)
}

/// Build a FOCUS request.
pub fn focus() -> String {
    "FOCUS".to_string()
}

/// Build a SET_AGENT request.
pub fn set_agent(tab_index: usize, agent_type: &str) -> String {
    format!("SET_AGENT|{}|{}", tab_index, agent_type)
}

/// Parse AGENT_STATUS response fields.
#[derive(Debug, Clone)]
pub struct AgentStatusResponse {
    pub session: String,
    pub status: String,
    pub ms_since_change: u64,
    pub tab: usize,
}

impl AgentStatusResponse {
    /// Parse "AGENT_STATUS|session|status|ms|tab=N"
    pub fn parse(response: &str) -> Option<Self> {
        let line = response.lines().next()?.trim();
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 5 || parts[0] != "AGENT_STATUS" {
            return None;
        }
        let tab = parts[4]
            .strip_prefix("tab=")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        Some(Self {
            session: parts[1].to_string(),
            status: parts[2].to_string(),
            ms_since_change: parts[3].parse().unwrap_or(0),
            tab,
        })
    }
}

/// Check if a response is an error.
pub fn is_error(response: &str) -> Option<String> {
    let line = response.lines().next()?.trim();
    if line.starts_with("ERR|") {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() >= 3 {
            return Some(parts[2].to_string());
        }
        return Some(line.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_payload() {
        let encoded = encode_payload("hello");
        assert_eq!(encoded, "aGVsbG8=");
    }

    #[test]
    fn test_input_message() {
        let msg = input("claude", "echo hello");
        assert!(msg.starts_with("INPUT|claude|"));
    }

    #[test]
    fn test_raw_input_message() {
        let msg = raw_input("claude", "\r");
        assert!(msg.starts_with("RAW_INPUT|claude|"));
    }

    #[test]
    fn test_agent_status_parse() {
        let resp = "AGENT_STATUS|my-session|IDLE|1234|tab=2\n";
        let parsed = AgentStatusResponse::parse(resp).unwrap();
        assert_eq!(parsed.session, "my-session");
        assert_eq!(parsed.status, "IDLE");
        assert_eq!(parsed.ms_since_change, 1234);
        assert_eq!(parsed.tab, 2);
    }

    #[test]
    fn test_is_error() {
        assert_eq!(
            is_error("ERR|session|unknown\n"),
            Some("unknown".to_string())
        );
        assert_eq!(is_error("PONG|session|123\n"), None);
    }
}
