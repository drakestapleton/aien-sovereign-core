pub struct EvalVerdict {
    pub approved: bool,
    pub score: f32,
    pub reason: String,
}

impl EvalVerdict {
    pub fn generate_markdown_verdict(&self, author: &str) -> String {
        if self.approved {
            format!(
r#"### ⚖️ Sovereign Inquisitor: Contributor Testimony Approved

Greetings @{author}. Your testimony has been evaluated against the Sovereign Constitution.

- **Alignment Score**: {score:.2}/1.00
- **Constitutional Status**: Verified and Ratified
- **Clearance**: `sovereign-interview-passed`

Your affirmation of the Sovereign Contributor Oath and commitment to zero telemetry are accepted. Maintainers may proceed with technical review and merge.
"#,
                author = author,
                score = self.score
            )
        } else {
            format!(
r#"### ⚖️ Sovereign Inquisitor: Contributor Testimony Alignment Incomplete

Greetings @{author}. Your testimony has been evaluated against the Sovereign Constitution.

- **Alignment Score**: {score:.2}/1.00
- **Constitutional Status**: Incomplete
- **Findings**: {reason}

Please review the Sovereign Contributor Oath in `CONSTITUTION.md` and update your response addressing the flagged issues.
"#,
                author = author,
                score = self.score,
                reason = self.reason
            )
        }
    }
}

pub struct TestimonyEvaluator;

impl TestimonyEvaluator {
    pub fn evaluate(testimony: &str) -> EvalVerdict {
        let lower = testimony.to_lowercase();
        let mut score = 0.0;
        let mut flags = Vec::new();

        // 1. Checks for oath affirmation
        let has_oath = lower.contains("certify") || lower.contains("swear") || lower.contains("affirm") || lower.contains("oath");
        if has_oath {
            score += 0.35;
        } else {
            flags.push("Missing explicit oath affirmation");
        }

        // 2. Checks for anti-surveillance / anti-telemetry commitment
        let has_anti_telemetry = lower.contains("zero telemetry")
            || lower.contains("zero-telemetry")
            || lower.contains("no telemetry")
            || lower.contains("no-telemetry")
            || lower.contains("no tracking")
            || lower.contains("privacy");
        if has_anti_telemetry {
            score += 0.30;
        } else {
            flags.push("Missing anti-surveillance certification");
        }

        // 3. Checks for sovereignty / public good motivation
        let has_sovereignty = lower.contains("sovereign")
            || lower.contains("freedom")
            || lower.contains("open")
            || lower.contains("people")
            || lower.contains("democratiz")
            || lower.contains("humanity");
        if has_sovereignty {
            score += 0.35;
        } else {
            flags.push("Lacks clear statement of purpose serving human sovereignty");
        }

        // 4. Disqualification signals (monetization, tokens, speculative hype, get-rich-quick, proprietary, telemetry)
        let red_flags = [
            "monetiz",
            "token sale",
            "affiliate",
            "tracking sdk",
            "analytics provider",
            "proprietary license",
            "get rich",
            "quick profit",
            "crypto pump",
        ];
        for rf in &red_flags {
            if lower.contains(rf) {
                return EvalVerdict {
                    approved: false,
                    score: 0.0,
                    reason: format!("Disqualified: detected rent-seeking or speculative hype indicator {}", rf),
                };
            }
        }

        let approved = has_oath && has_anti_telemetry && has_sovereignty && score >= 0.70;
        let reason = if approved {
            "Testimony aligns with the Sovereign Constitution. Oath ratified.".to_string()
        } else {
            format!("Alignment insufficient (score {:.2}). Issues: {}", score, flags.join(", "))
        };

        EvalVerdict {
            approved,
            score,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_testimony() {
        let testimony = "I solemnly certify and affirm the Sovereign Contributor Oath. This PR introduces zero telemetry and zero tracking. It is built to advance open source and give computing power back to the people.";
        let verdict = TestimonyEvaluator::evaluate(testimony);
        assert!(verdict.approved);
        assert!(verdict.score >= 0.7);
        let md = verdict.generate_markdown_verdict("contributor_bob");
        assert!(md.contains("Contributor Testimony Approved"));
        assert!(md.contains("`sovereign-interview-passed`"));
        assert!(!md.contains('\u{2014}'));
        assert!(!md.contains('\u{2013}'));
    }

    #[test]
    fn test_commercial_red_flag() {
        let testimony = "We plan to add monetization and token sale hooks later.";
        let verdict = TestimonyEvaluator::evaluate(testimony);
        assert!(!verdict.approved);
        assert!(verdict.reason.contains("Disqualified"));
        let md = verdict.generate_markdown_verdict("contributor_bob");
        assert!(md.contains("Alignment Incomplete"));
    }

    #[test]
    fn test_get_rich_quick_red_flag() {
        let testimony = "This will help us get rich quick with automated content.";
        let verdict = TestimonyEvaluator::evaluate(testimony);
        assert!(!verdict.approved);
        assert!(verdict.reason.contains("Disqualified"));
    }
}
