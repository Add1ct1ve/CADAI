use crate::ai::message::ChatMessage;

/// Trim conversation history to fit within token limits.
/// Simple approach: keep all system messages plus the last `max_messages` non-system messages.
#[allow(dead_code)]
pub fn trim_history(messages: &[ChatMessage], max_messages: usize) -> Vec<ChatMessage> {
    if messages.len() <= max_messages {
        return messages.to_vec();
    }

    let mut trimmed = Vec::new();

    // Always keep system messages.
    for msg in messages {
        if msg.role == "system" {
            trimmed.push(msg.clone());
        }
    }

    // Keep the last N non-system messages.
    let non_system: Vec<&ChatMessage> = messages.iter().filter(|m| m.role != "system").collect();

    let start = if non_system.len() > max_messages {
        non_system.len() - max_messages
    } else {
        0
    };

    for msg in &non_system[start..] {
        trimmed.push((*msg).clone());
    }

    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_trim_no_truncation_needed() {
        let messages = vec![
            msg("system", "sys"),
            msg("user", "hi"),
            msg("assistant", "hello"),
        ];
        let result = trim_history(&messages, 10);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_trim_keeps_system_and_last_n() {
        let messages = vec![
            msg("system", "sys"),
            msg("user", "1"),
            msg("assistant", "2"),
            msg("user", "3"),
            msg("assistant", "4"),
            msg("user", "5"),
        ];
        let result = trim_history(&messages, 3);
        // system + last 3 non-system
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].role, "system");
        assert_eq!(result[1].content, "3");
        assert_eq!(result[2].content, "4");
        assert_eq!(result[3].content, "5");
    }
}
