use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct TranscriptTurn {
    pub role: String,   // "user" | "assistant"
    pub text: String,
    pub ts: i64,        // unix seconds; 0 if unparseable
}

pub fn parse_tail(jsonl: &str, n_turns: usize) -> Vec<TranscriptTurn> {
    let mut turns: Vec<TranscriptTurn> = Vec::new();
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // skip malformed
        };
        let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        if t != "user" && t != "assistant" { continue; }

        let msg = v.get("message").and_then(|x| x.as_object());
        let role = msg
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str())
            .unwrap_or(t)
            .to_string();

        let text = msg
            .and_then(|m| m.get("content"))
            .map(extract_text)
            .unwrap_or_default();

        let ts = v.get("timestamp")
            .and_then(|x| x.as_str())
            .map(parse_iso8601_to_unix)
            .unwrap_or(0);

        turns.push(TranscriptTurn { role, text, ts });
    }
    let len = turns.len();
    if len > n_turns {
        turns.split_off(len - n_turns)
    } else {
        turns
    }
}

fn extract_text(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut out = String::new();
        for block in arr {
            if let Some(obj) = block.as_object() {
                let bt = obj.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if bt == "text" {
                    if let Some(t) = obj.get("text").and_then(|x| x.as_str()) {
                        if !out.is_empty() { out.push('\n'); }
                        out.push_str(t);
                    }
                }
            }
        }
        return out;
    }
    String::new()
}

fn parse_iso8601_to_unix(s: &str) -> i64 {
    // Minimal ISO-8601 parser: try the chrono-free path.
    // Accept "YYYY-MM-DDTHH:MM:SS[.fff]Z".
    let s = s.trim_end_matches('Z');
    // Split date and time
    let (date, time) = match s.split_once('T') {
        Some(p) => p,
        None => return 0,
    };
    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() != 3 { return 0; }
    let time_part = time.split('.').next().unwrap_or(time);
    let time_parts: Vec<&str> = time_part.split(':').collect();
    if time_parts.len() != 3 { return 0; }
    let (Ok(y), Ok(m), Ok(d), Ok(h), Ok(mi), Ok(se)) = (
        date_parts[0].parse::<i32>(),
        date_parts[1].parse::<u32>(),
        date_parts[2].parse::<u32>(),
        time_parts[0].parse::<u32>(),
        time_parts[1].parse::<u32>(),
        time_parts[2].parse::<u32>(),
    ) else { return 0 };
    // Days since epoch using a basic calendar formula (Julian day shift).
    let mut y = y as i64;
    let mut m = m as i64;
    if m <= 2 { y -= 1; m += 12; }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let doy = (153 * (m - 3) + 2) / 5 + (d as i64) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + (h as i64) * 3600 + (mi as i64) * 60 + (se as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_no_turns() {
        assert!(parse_tail("", 10).is_empty());
    }

    #[test]
    fn string_content_user_turn() {
        let line = r#"{"type":"user","message":{"role":"user","content":"hello"},"timestamp":"2026-05-11T10:00:00.000Z"}"#;
        let v = parse_tail(line, 10);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].role, "user");
        assert_eq!(v[0].text, "hello");
        assert!(v[0].ts > 0);
    }

    #[test]
    fn array_content_assistant_turn_with_mixed_blocks() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"sure"},{"type":"tool_use","id":"x","name":"Bash","input":{"command":"ls"}},{"type":"text","text":"done"}]},"timestamp":"2026-05-11T10:01:00.000Z"}"#;
        let v = parse_tail(line, 10);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].role, "assistant");
        assert_eq!(v[0].text, "sure\ndone");
    }

    #[test]
    fn non_user_assistant_types_filtered_out() {
        let lines = r#"{"type":"system","message":{"role":"system","content":"x"}}
{"type":"attachment"}
{"type":"user","message":{"role":"user","content":"kept"}}"#;
        let v = parse_tail(lines, 10);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].text, "kept");
    }

    #[test]
    fn malformed_lines_skipped() {
        let lines = "not json\n{not closed\n{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"survives\"}}";
        let v = parse_tail(lines, 10);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].text, "survives");
    }

    #[test]
    fn caps_to_last_n_turns() {
        let mut lines = String::new();
        for i in 0..15 {
            lines.push_str(&format!(
                r#"{{"type":"user","message":{{"role":"user","content":"m{}"}}}}{}"#,
                i, "\n"));
        }
        let v = parse_tail(&lines, 5);
        assert_eq!(v.len(), 5);
        assert_eq!(v[0].text, "m10");
        assert_eq!(v[4].text, "m14");
    }

    #[test]
    fn real_transcript_fixture_parses_user_and_assistant_turns() {
        let fixture = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/transcript_sample.jsonl"))
            .expect("fixture exists");
        let turns = parse_tail(&fixture, 10);
        // Fixture has 2 user + 2 assistant lines + 1 system (filtered).
        assert_eq!(turns.len(), 4, "expected 4 turns, got {}", turns.len());
        assert!(turns.iter().all(|t| t.role == "user" || t.role == "assistant"));
        // At least one turn should have non-empty text (either from string or array content).
        assert!(turns.iter().any(|t| !t.text.is_empty()),
                "no turn has any text — content extraction broken");
    }
}
