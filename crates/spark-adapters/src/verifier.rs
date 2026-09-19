use crate::models::{VerificationOutcome, VerificationStrategy};

pub struct Verifier;

impl Verifier {
    pub fn verify(content: &str, strategy: VerificationStrategy) -> VerificationOutcome {
        let mut violations = Vec::new();
        let mut score: f32 = 1.0;

        // 1. Unslop and Tone Checks
        if content.contains('\u{2014}') || content.contains('—') || content.contains("—") {
            violations.push("Contains forbidden em dash (—)".to_string());
            score -= 0.25;
        }
        if content.contains('\u{2013}') || content.contains('–') || content.contains("—") {
            violations.push("Contains forbidden en dash (–)".to_string());
            score -= 0.15;
        }

        let banned_words = [
            "delve", "tapestry", "crucial", "beacon",
            "game-changer", "unleash", "seamlessly",
            "elevate", "pivotal"
        ];
        let lower = content.to_lowercase();
        for word in banned_words {
            if lower.contains(word) {
                violations.push(format!("Contains banned AI cliché: '{}'", word));
                score -= 0.15;
            }
        }

        let sycophancy_phrases = [
            "certainly!", "great question!", "sure thing!",
            "i would be happy to help", "as an ai language model"
        ];
        for phrase in sycophancy_phrases {
            if lower.contains(phrase) {
                violations.push(format!("Contains sycophantic phrase: '{}'", phrase));
                score -= 0.2;
            }
        }

        // 2. Zero-Secret Leak Checks
        let secret_signatures = [
            "-----BEGIN PRIVATE KEY-----",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "sk-proj-",
            "ghp_",
        ];
        for sig in secret_signatures {
            if content.contains(sig) {
                violations.push(format!("Critical: Contains secret signature: {}", sig));
                score = 0.0;
            }
        }

        // 3. Strategy-specific validation
        let mut compiler_output = None;
        match strategy {
            VerificationStrategy::CompilerCheck => {
                if let Some(err) = Self::check_balanced_delimiters(content) {
                    violations.push(format!("Code structure syntax error: {}", err));
                    score -= 0.3;
                    compiler_output = Some(err);
                } else {
                    compiler_output = Some("Balanced delimiters and code syntax verified.".to_string());
                }
            }
            VerificationStrategy::JsonSchema => {
                if let Some(json_start) = content.find('{') {
                    if let Some(json_end) = content.rfind('}') {
                        let slice = &content[json_start..=json_end];
                        if serde_json::from_str::<serde_json::Value>(slice).is_err() {
                            violations.push("Embedded JSON payload is malformed".to_string());
                            score -= 0.3;
                        }
                    } else {
                        violations.push("Incomplete JSON object brackets".to_string());
                        score -= 0.3;
                    }
                }
            }
            VerificationStrategy::UnslopStrict => {
                // Strict zero-tolerance for any unslop violation
                if !violations.is_empty() {
                    score = score.min(0.5);
                }
            }
            VerificationStrategy::DualConsensus => {}
        }

        score = score.max(0.0).min(1.0);
        let passed = violations.is_empty() || (score >= 0.70 && !violations.iter().any(|v| v.starts_with("Critical")));

        VerificationOutcome {
            passed,
            score,
            rule_violations: violations,
            compiler_output,
        }
    }

    fn check_balanced_delimiters(content: &str) -> Option<String> {
        let mut stack = Vec::new();
        let mut in_string = false;
        let mut escape = false;

        for (i, c) in content.chars().enumerate() {
            if escape {
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
                continue;
            }
            if c == '"' {
                in_string = !in_string;
                continue;
            }
            if in_string {
                continue;
            }

            match c {
                '(' | '[' | '{' => stack.push((c, i)),
                ')' => {
                    if let Some(('(', _)) = stack.last() {
                        stack.pop();
                    } else {
                        return Some(format!("Unmatched ')' at position {}", i));
                    }
                }
                ']' => {
                    if let Some(('[', _)) = stack.last() {
                        stack.pop();
                    } else {
                        return Some(format!("Unmatched ']' at position {}", i));
                    }
                }
                '}' => {
                    if let Some(('{', _)) = stack.last() {
                        stack.pop();
                    } else {
                        return Some(format!("Unmatched '}}' at position {}", i));
                    }
                }
                _ => {}
            }
        }

        if let Some((unmatched, pos)) = stack.pop() {
            Some(format!("Unclosed delimiter '{}' opened at position {}", unmatched, pos))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_response_passes() {
        let text = "Here is the pure Rust ring buffer implementation with zero allocations.";
        let out = Verifier::verify(text, VerificationStrategy::UnslopStrict);
        assert!(out.passed);
        assert_eq!(out.score, 1.0);
        assert!(out.rule_violations.is_empty());
    }

    #[test]
    fn test_em_dash_detected() {
        let text = "This module is fast — and very reliable.";
        let out = Verifier::verify(text, VerificationStrategy::UnslopStrict);
        assert!(!out.passed);
        assert!(out.rule_violations.iter().any(|v| v.contains("em dash")));
    }

    #[test]
    fn test_banned_cliche_detected() {
        let text = "Let us delve into the architecture.";
        let out = Verifier::verify(text, VerificationStrategy::UnslopStrict);
        assert!(out.rule_violations.iter().any(|v| v.contains("delve")));
    }
}
