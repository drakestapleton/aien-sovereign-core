use sha2::{Digest, Sha256};
use std::collections::HashSet;
use crate::extractor::ExtractedPair;

pub struct DeduplicationFilter {
    seen_hashes: HashSet<String>,
    min_quality: f32,
}

impl DeduplicationFilter {
    pub fn new(min_quality: f32) -> Self {
        Self {
            seen_hashes: HashSet::new(),
            min_quality,
        }
    }

    pub fn accept(&mut self, pair: &ExtractedPair) -> bool {
        if pair.quality_score < self.min_quality {
            return false;
        }
        let hash = self.compute_fingerprint(&pair.prompt, &pair.completion);
        self.seen_hashes.insert(hash)
    }

    fn compute_fingerprint(&self, prompt: &str, completion: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(prompt.trim().to_lowercase().as_bytes());
        hasher.update(b"||");
        hasher.update(completion.trim().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn count(&self) -> usize {
        self.seen_hashes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_filter() {
        let mut filter = DeduplicationFilter::new(0.6);
        let pair1 = ExtractedPair {
            prompt: "Test prompt".to_string(),
            reasoning: Some("Thought".to_string()),
            completion: "Result".to_string(),
            provider: "openai".to_string(),
            model: "o3-mini".to_string(),
            token_count: 50,
            quality_score: 0.8,
        };

        assert!(filter.accept(&pair1));
        assert!(!filter.accept(&pair1));
        assert_eq!(filter.count(), 1);
    }
}
