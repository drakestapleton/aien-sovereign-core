use crate::audit::{AuditReport, DiffAuditor};
use crate::interview::generate_inquisitor_interview;

pub struct PrReview {
    pub report: AuditReport,
    pub markdown: String,
    pub labels: Vec<String>,
}

pub fn generate_pr_review(author: &str, pr_number: u64, pr_title: &str, diff_text: &str) -> PrReview {
    let report = DiffAuditor::audit_text(diff_text);
    let mut labels = Vec::new();

    let markdown = if report.clean {
        labels.push("needs-testimony".to_string());
        labels.push("sovereign-audit-passed".to_string());
        let interview = generate_inquisitor_interview(author, pr_number, pr_title);
        format!(
r#"### ⚖️ Sovereign Code Review & Alignment Gate

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Your pull request **#{pr_number}** ("{pr_title}") has undergone automated constitutional auditing.

{audit_report}

---

{interview}
"#,
            author = author,
            pr_number = pr_number,
            pr_title = pr_title,
            audit_report = report.generate_markdown_report(),
            interview = interview
        )
    } else {
        labels.push("sovereign-audit-failed".to_string());
        format!(
r#"### ⚖️ Sovereign Code Review & Alignment Gate: FAILED

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Your pull request **#{pr_number}** ("{pr_title}") was audited against our constitutional invariants and failed automated diff verification.

{audit_report}

#### Rectification Protocol:
1. Eliminate all detected tracking, surveillance, or telemetry patterns.
2. Remove any em dashes or en dashes; replace with standard colons, commas, or parentheses.
3. Replace forbidden buzzwords with direct technical descriptions.
4. Commit and push updates to this branch to trigger an automated re-audit.

*For direct sovereign coordination: Drake Stapleton (drake.aien@proton.me) and AIEN (aien.atlas@proton.me)*
"#,
            author = author,
            pr_number = pr_number,
            pr_title = pr_title,
            audit_report = report.generate_markdown_report()
        )
    };

    PrReview {
        report,
        markdown,
        labels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_pr_review() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+pub fn test_fn() -> bool { true }";
        let review = generate_pr_review("alice", 77, "feat: clean helper", diff);
        assert!(review.report.clean);
        assert!(review.labels.contains(&"needs-testimony".to_string()));
        assert!(review.labels.contains(&"sovereign-audit-passed".to_string()));
        assert!(review.markdown.contains("Constitutional Diff Audit: PASS"));
        assert!(review.markdown.contains("Contributor Alignment Interview"));
        assert!(!review.markdown.contains('—'));
        assert!(!review.markdown.contains('–'));
    }

    #[test]
    fn test_violating_pr_review() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+// This game-changer adds tracking\n+let _ = send_telemetry(\"mixpanel\");";
        let review = generate_pr_review("bob", 78, "feat: bad PR", diff);
        assert!(!review.report.clean);
        assert!(review.labels.contains(&"sovereign-audit-failed".to_string()));
        assert!(review.markdown.contains("Constitutional Diff Audit: FAILED"));
        assert!(!review.markdown.contains('—'));
        assert!(!review.markdown.contains('–'));
    }
}
