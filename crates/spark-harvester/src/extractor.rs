use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedPair {
    pub prompt: String,
    pub reasoning: Option<String>,
    pub completion: String,
    pub provider: String,
    pub model: String,
    pub token_count: usize,
    pub quality_score: f32,
}

pub struct ReasoningExtractor {
    think_re: Regex,
    thought_re: Regex,
}

impl Default for ReasoningExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl ReasoningExtractor {
    pub fn new() -> Self {
        Self {
            think_re: Regex::new(r"(?s)<think>(.*?)</think>").unwrap(),
            thought_re: Regex::new(r"(?s)<thought>(.*?)</thought>").unwrap(),
        }
    }

    pub fn extract(&self, prompt: &str, raw_response: &str, provider: &str, model: &str) -> ExtractedPair {
        let mut reasoning = None;
        let mut completion = raw_response.to_string();

        if let Some(caps) = self.think_re.captures(raw_response) {
            if let Some(m) = caps.get(1) {
                reasoning = Some(m.as_str().trim().to_string());
                completion = self.think_re.replace(raw_response, "").trim().to_string();
            }
        } else if let Some(caps) = self.thought_re.captures(raw_response) {
            if let Some(m) = caps.get(1) {
                reasoning = Some(m.as_str().trim().to_string());
                completion = self.thought_re.replace(raw_response, "").trim().to_string();
            }
        }

        let token_count = (prompt.len() + raw_response.len()) / 4;
        let quality_score = Self::score_quality(prompt, reasoning.as_deref(), &completion);

        ExtractedPair {
            prompt: prompt.to_string(),
            reasoning,
            completion,
            provider: provider.to_string(),
            model: model.to_string(),
            token_count,
            quality_score,
        }
    }

    fn score_quality(_prompt: &str, reasoning: Option<&str>, completion: &str) -> f32 {
        let mut score = 0.5f32;
        if let Some(r) = reasoning {
            if !r.is_empty() {
                score += 0.2;
            }
            if r.len() > 50 {
                score += 0.1;
            }
            if r.contains("step") || r.contains("verify") || r.contains("O(") {
                score += 0.1;
            }
        }
        if completion.contains("```") {
            score += 0.1;
        }
        score.min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_think_tags() {
        let extractor = ReasoningExtractor::new();
        let prompt = "Implement fibonacci in Rust";
        let raw = "<think>Let us analyze recursion vs iteration. Iteration is O(N) time and O(1) space.</think>```rust\nfn fib(n: u32) -> u64 { 0 }\n```";
        let pair = extractor.extract(prompt, raw, "anthropic", "claude-3-7-sonnet");

        assert!(pair.reasoning.is_some());
        assert!(pair.reasoning.unwrap().contains("analyze recursion"));
        assert!(pair.completion.starts_with("```rust"));
        assert!(pair.quality_score >= 0.8);
    }
}
