use crate::agent::prompts::PromptTier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextBucket {
    K128,
    K200,
    K400,
    K1M,
}

pub fn context_bucket_for_model(provider: &str, model: &str) -> ContextBucket {
    match provider {
        "claude" => ContextBucket::K200,
        "openai" => match model {
            "gpt-5.2" => ContextBucket::K400,
            "o3-mini" => ContextBucket::K200,
            _ => ContextBucket::K128,
        },
        "deepseek" => match model {
            "deepseek-reasoner" => ContextBucket::K128,
            _ => ContextBucket::K128,
        },
        "qwen" => ContextBucket::K128,
        "kimi" => match model {
            "kimi-k2.5" => ContextBucket::K400,
            _ => ContextBucket::K128,
        },
        "gemini" => match model {
            "gemini-2.5-pro" | "gemini-2.5-flash" => ContextBucket::K1M,
            _ => ContextBucket::K128,
        },
        "ollama" => ContextBucket::K128,
        _ => ContextBucket::K128,
    }
}

pub fn prompt_tier_for_bucket(bucket: ContextBucket) -> PromptTier {
    match bucket {
        ContextBucket::K128 => PromptTier::Expanded,
        ContextBucket::K200 => PromptTier::ExpandedPlus,
        ContextBucket::K400 | ContextBucket::K1M => PromptTier::Full,
    }
}

#[cfg(test)]
mod tests {
    use super::{context_bucket_for_model, prompt_tier_for_bucket, ContextBucket};
    use crate::agent::prompts::PromptTier;

    #[test]
    fn bucket_mapping_examples() {
        assert_eq!(
            context_bucket_for_model("deepseek", "deepseek-reasoner"),
            ContextBucket::K128
        );
        assert_eq!(
            context_bucket_for_model("claude", "claude-sonnet-4-5-20250929"),
            ContextBucket::K200
        );
        assert_eq!(
            context_bucket_for_model("openai", "gpt-5.2"),
            ContextBucket::K400
        );
        assert_eq!(
            context_bucket_for_model("gemini", "gemini-2.5-pro"),
            ContextBucket::K1M
        );
    }

    #[test]
    fn prompt_tier_examples() {
        assert_eq!(
            prompt_tier_for_bucket(ContextBucket::K128),
            PromptTier::Expanded
        );
        assert_eq!(
            prompt_tier_for_bucket(ContextBucket::K200),
            PromptTier::ExpandedPlus
        );
        assert_eq!(
            prompt_tier_for_bucket(ContextBucket::K400),
            PromptTier::Full
        );
        assert_eq!(
            prompt_tier_for_bucket(ContextBucket::K1M),
            PromptTier::Full
        );
    }
}
