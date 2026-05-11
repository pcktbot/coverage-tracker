#[derive(Debug, Clone)]
pub enum EventKind {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    Notification,
    Stop { error: bool },
    Progress,
    Artifact,
    InboxIn,
}

pub const KNOWN_STATUSES: [&str; 6] =
    ["working", "idle", "needs_input", "done", "error", "unknown"];

pub fn transition(prev: &str, event: &EventKind) -> &'static str {
    if !KNOWN_STATUSES.contains(&prev) && !prev.is_empty() {
        panic!("unknown prev status: {prev}");
    }
    match event {
        EventKind::SessionStart => "working",
        EventKind::UserPromptSubmit => "working",
        EventKind::PreToolUse if prev == "needs_input" => "needs_input",
        EventKind::PreToolUse => "working",
        EventKind::Notification => "needs_input",
        EventKind::Stop { error: true } => "error",
        EventKind::Stop { error: false } => "done",
        EventKind::Progress | EventKind::Artifact | EventKind::InboxIn => {
            match prev {
                "" | "unknown" => "working",
                "working"      => "working",
                "idle"         => "idle",
                "needs_input"  => "needs_input",
                "done"         => "done",
                "error"        => "error",
                _ => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EventKind::*;

    #[test]
    fn transitions_for_every_event() {
        let cases = [
            ("unknown",     SessionStart,             "working"),
            ("idle",        UserPromptSubmit,         "working"),
            ("working",     PreToolUse,               "working"),
            ("needs_input", PreToolUse,               "needs_input"),
            ("working",     Notification,             "needs_input"),
            ("needs_input", UserPromptSubmit,         "working"),
            ("working",     Stop { error: false },    "done"),
            ("working",     Stop { error: true },     "error"),
            ("done",        SessionStart,             "working"),
        ];
        for (prev, ev, expected) in cases {
            let got = transition(prev, &ev);
            assert_eq!(got, expected, "from {prev} on {ev:?}");
        }
    }

    #[test]
    #[should_panic(expected = "unknown prev status")]
    fn unknown_prev_panics() {
        let _ = transition("bogus", &PreToolUse);
    }
}
