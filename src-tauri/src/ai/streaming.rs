/// Parse a chunk of SSE data into individual event data strings.
/// SSE format: lines starting with "data: " followed by JSON, separated by blank lines.
/// The special "data: [DONE]" marker signals the end of the stream.
pub fn parse_sse_events(buffer: &str) -> Vec<String> {
    let mut events = Vec::new();

    for line in buffer.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            if data != "[DONE]" {
                events.push(data.to_string());
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_single_event() {
        let input = "data: {\"text\":\"hello\"}\n\n";
        let events = parse_sse_events(input);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], "{\"text\":\"hello\"}");
    }

    #[test]
    fn test_parse_sse_multiple_events() {
        let input = "data: {\"a\":1}\n\ndata: {\"b\":2}\n\n";
        let events = parse_sse_events(input);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_parse_sse_done_marker() {
        let input = "data: {\"text\":\"hi\"}\n\ndata: [DONE]\n\n";
        let events = parse_sse_events(input);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_parse_sse_ignores_other_lines() {
        let input = "event: message\ndata: {\"x\":1}\nid: 123\n\n";
        let events = parse_sse_events(input);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], "{\"x\":1}");
    }
}
