use super::provider::TokenUsage;

/// Estimate cost in USD for the given usage. Returns None for unknown provider/model combos.
pub fn estimate_cost(provider: &str, model: &str, usage: &TokenUsage) -> Option<f64> {
    let (input_rate, output_rate) = get_rates_per_million(provider, model)?;
    Some(
        (usage.input_tokens as f64 * input_rate + usage.output_tokens as f64 * output_rate)
            / 1_000_000.0,
    )
}

/// Returns (input_rate, output_rate) per million tokens.
fn get_rates_per_million(provider: &str, model: &str) -> Option<(f64, f64)> {
    match (provider, model) {
        ("claude", m) if m.contains("sonnet") => Some((3.0, 15.0)),
        ("claude", m) if m.contains("opus") => Some((15.0, 75.0)),
        ("claude", m) if m.contains("haiku") => Some((0.25, 1.25)),
        ("openai", m) if m.starts_with("gpt-5") => Some((2.5, 10.0)),
        ("openai", "o3-mini") => Some((1.1, 4.4)),
        ("deepseek", "deepseek-chat") => Some((0.27, 1.10)),
        ("deepseek", "deepseek-reasoner") => Some((0.55, 2.19)),
        ("qwen", _) => Some((0.30, 0.60)),
        ("kimi", _) => Some((0.70, 2.80)),
        ("gemini", m) if m.contains("pro") => Some((1.25, 10.0)),
        ("gemini", m) if m.contains("flash") => Some((0.15, 0.60)),
        ("runpod", _) => Some((0.0, 0.0)),
        ("ollama", _) => Some((0.0, 0.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_cost_claude_sonnet() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
        };
        let cost = estimate_cost("claude", "claude-3-5-sonnet-20241022", &usage).unwrap();
        // 1000 * 3.0 / 1M + 500 * 15.0 / 1M = 0.003 + 0.0075 = 0.0105
        assert!((cost - 0.0105).abs() < 1e-10);
    }

    #[test]
    fn test_estimate_cost_ollama_free() {
        let usage = TokenUsage {
            input_tokens: 5000,
            output_tokens: 2000,
        };
        let cost = estimate_cost("ollama", "llama3", &usage).unwrap();
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_estimate_cost_unknown_returns_none() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
        };
        assert!(estimate_cost("unknown_provider", "unknown_model", &usage).is_none());
    }

    #[test]
    fn test_estimate_cost_math() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
        };
        // Claude Opus: 15.0 + 75.0 = $90.00
        let cost = estimate_cost("claude", "claude-3-opus-20240229", &usage).unwrap();
        assert!((cost - 90.0).abs() < 1e-10);
    }
}
