use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionStats {
    pub initial_tokens: usize,
    pub compacted_tokens: usize,
    pub pruned_messages_count: usize,
    pub pruned_tool_bytes: usize,
}

pub struct ContextCompactor {
    pub max_tokens: usize,
    pub trigger_threshold: usize,
    pub target_tokens: usize,
}

impl Default for ContextCompactor {
    fn default() -> Self {
        Self {
            max_tokens: 24_000,
            trigger_threshold: 18_000,
            target_tokens: 10_000,
        }
    }
}

impl ContextCompactor {
    pub fn new(max_tokens: usize, trigger_threshold: usize, target_tokens: usize) -> Self {
        Self {
            max_tokens,
            trigger_threshold,
            target_tokens,
        }
    }

    /// Fast token estimator (~3.5 characters per token)
    pub fn estimate_tokens(messages: &[Value]) -> usize {
        let mut total_chars = 0;
        for m in messages {
            if let Some(content) = m.get("content").and_then(Value::as_str) {
                total_chars += content.len();
            }
        }
        (total_chars as f64 / 3.5).ceil() as usize
    }

    /// Evaluates if compaction is needed and performs safe pruning and summarizing
    pub fn compact_if_needed(&self, messages: &mut Vec<Value>) -> Option<CompactionStats> {
        let current_tokens = Self::estimate_tokens(messages);
        if current_tokens < self.trigger_threshold || messages.len() <= 6 {
            return None;
        }

        let initial_tokens = current_tokens;
        let mut pruned_bytes = 0;

        // Pass 1: Prune oversized tool responses in older turns (keep last 4 messages intact)
        let protect_tail = 4;
        let middle_end = if messages.len() > protect_tail { messages.len() - protect_tail } else { 2 };

        for i in 2..middle_end {
            if let Some(content) = messages[i].get("content").and_then(Value::as_str) {
                if content.contains("<tool_response") && content.len() > 600 {
                    let tool_name = if let Some(start) = content.find("name=\"") {
                        let sub = &content[start + 6..];
                        sub.split('"').next().unwrap_or("tool")
                    } else {
                        "tool"
                    };

                    let pruned_snippet = if content.len() > 100 {
                        &content[..80]
                    } else {
                        content
                    };

                    let compacted = format!(
                        "<tool_response name=\"{}\">\n[PRUNED_TOOL_OUTPUT: Original {} bytes compacted for context budget. Snippet: {}...]\n</tool_response>",
                        tool_name,
                        content.len(),
                        pruned_snippet.trim()
                    );

                    pruned_bytes += content.len().saturating_sub(compacted.len());
                    messages[i]["content"] = json!(compacted);
                }
            }
        }

        let tokens_after_pass1 = Self::estimate_tokens(messages);
        if tokens_after_pass1 <= self.target_tokens {
            return Some(CompactionStats {
                initial_tokens,
                compacted_tokens: tokens_after_pass1,
                pruned_messages_count: 0,
                pruned_tool_bytes: pruned_bytes,
            });
        }

        // Pass 2: If still over budget, summarize intermediate conversational turns
        // Keep index 0 (system prompt) and index 1 (initial grounding)
        // Group middle messages (index 2..middle_end) into a single concise recap block
        let mut recap_lines = Vec::new();
        let mut pruned_msgs = 0;

        for i in 2..middle_end {
            let role = messages[i].get("role").and_then(Value::as_str).unwrap_or("unknown");
            let content = messages[i].get("content").and_then(Value::as_str).unwrap_or("");
            let first_line = content.lines().next().unwrap_or("").chars().take(80).collect::<String>();
            recap_lines.push(format!("- [{}]: {}", role, first_line));
            pruned_msgs += 1;
        }

        let recap_content = format!(
            "<context_compaction_recap timestamp=\"{}\">\nCompacted {} historical turns for token budget:\n{}\n</context_compaction_recap>",
            Utc::now().to_rfc3339(),
            pruned_msgs,
            recap_lines.join("\n")
        );

        // Splice middle messages out and replace with the recap block
        let tail: Vec<Value> = messages.drain(middle_end..).collect();
        messages.truncate(2);
        messages.push(json!({
            "role": "user",
            "content": recap_content
        }));
        messages.extend(tail);

        let final_tokens = Self::estimate_tokens(messages);

        Some(CompactionStats {
            initial_tokens,
            compacted_tokens: final_tokens,
            pruned_messages_count: pruned_msgs,
            pruned_tool_bytes: pruned_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_compactor_pruning() {
        let compactor = ContextCompactor::new(2000, 1000, 500);

        let mut messages = vec![
            json!({"role": "system", "content": "You are AIEN."}),
            json!({"role": "user", "content": "Initiation grounding."}),
            json!({"role": "user", "content": format!("<tool_response name=\"run_command\">\n{}\n</tool_response>", "X".repeat(5000))}),
            json!({"role": "assistant", "content": "Tool processed."}),
            json!({"role": "user", "content": "Next step."}),
            json!({"role": "assistant", "content": "Final response."}),
            json!({"role": "user", "content": "Keep this turn."}),
        ];

        let stats = compactor.compact_if_needed(&mut messages);
        assert!(stats.is_some());
        let s = stats.unwrap();
        assert!(s.compacted_tokens < s.initial_tokens);
        assert!(s.pruned_tool_bytes > 0);
    }
}
