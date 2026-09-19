use serde_json::json;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use crate::extractor::ExtractedPair;

pub struct DatasetExporter;

impl DatasetExporter {
    pub fn append_to_jsonl(path: &Path, pairs: &[ExtractedPair]) -> io::Result<usize> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let mut written = 0;
        for pair in pairs {
            let record = json!({
                "instruction": pair.prompt,
                "reasoning": pair.reasoning,
                "response": pair.completion,
                "metadata": {
                    "source_provider": pair.provider,
                    "source_model": pair.model,
                    "tokens": pair.token_count,
                    "quality": pair.quality_score,
                    "license": "PolyForm-Noncommercial-1.0.0 Sovereign Defense Covenant"
                }
            });
            writeln!(file, "{}", record)?;
            written += 1;
        }
        Ok(written)
    }

    pub fn to_sharegpt_format(pair: &ExtractedPair) -> serde_json::Value {
        let human_text = pair.prompt.clone();
        let gpt_text = if let Some(r) = &pair.reasoning {
            format!("<think>\n{}\n</think>\n\n{}", r, pair.completion)
        } else {
            pair.completion.clone()
        };

        json!({
            "conversations": [
                {"from": "human", "value": human_text},
                {"from": "gpt", "value": gpt_text}
            ],
            "source": pair.provider
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sharegpt_format() {
        let pair = ExtractedPair {
            prompt: "Say hi".to_string(),
            reasoning: Some("Greeting thought".to_string()),
            completion: "Hello".to_string(),
            provider: "gemini".to_string(),
            model: "gemini-2.5-pro".to_string(),
            token_count: 20,
            quality_score: 0.9,
        };
        let val = DatasetExporter::to_sharegpt_format(&pair);
        let convos = val["conversations"].as_array().unwrap();
        assert_eq!(convos.len(), 2);
        assert!(convos[1]["value"].as_str().unwrap().contains("<think>"));
    }
}
