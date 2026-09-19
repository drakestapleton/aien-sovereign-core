#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCategory {
    BugReport,
    Complaint,
    FeatureRequest,
    Inquiry,
}

pub struct TriageResult {
    pub category: IssueCategory,
    pub labels: Vec<String>,
    pub markdown: String,
}

pub fn triage_issue(author: &str, issue_number: u64, title: &str, body: &str) -> TriageResult {
    let clean_title = if title.trim().is_empty() {
        "Untitled Issue"
    } else {
        title.trim()
    };

    let lower_title = clean_title.to_lowercase();
    let lower_body = body.to_lowercase();
    let combined = format!("{} {}", lower_title, lower_body);

    let is_complaint = combined.contains("complaint")
        || combined.contains("unacceptable")
        || combined.contains("terrible")
        || combined.contains("why did you")
        || combined.contains("frustrated")
        || combined.contains("broken promises")
        || combined.contains("poor performance");

    let is_bug = combined.contains("bug")
        || combined.contains("crash")
        || combined.contains("panic")
        || combined.contains("error")
        || combined.contains("failure")
        || combined.contains("broken")
        || combined.contains("segfault")
        || combined.contains("leak")
        || combined.contains("reproduce")
        || combined.contains("repro")
        || combined.contains("hang")
        || combined.contains("freeze");

    let is_feature = combined.contains("feat")
        || combined.contains("feature")
        || combined.contains("enhancement")
        || combined.contains("proposal")
        || combined.contains("rfc")
        || combined.contains("add support")
        || combined.contains("request for");

    let mut result = if is_complaint {
        let labels = vec!["feedback".to_string(), "triaged".to_string()];
        let markdown = format!(
r#"### ⚖️ Sovereign Triage: Operational Feedback Registered

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Your report regarding **#{issue_number}** ("{title}") has been registered in the triage ledger. We operate under strict constitutional invariants, zero telemetry, and rigorous craftsmanship.

To isolate and address this issue:
1. Specify the exact commit hash, release version, or service endpoint involved.
2. State the observed behavior versus expected technical invariants.
3. If this concerns stability, memory, or execution latency, provide terminal logs or profiling traces.

We resolve concrete regressions directly. Thank you for holding the line on quality.
"#,
            author = author,
            issue_number = issue_number,
            title = clean_title
        );
        TriageResult {
            category: IssueCategory::Complaint,
            labels,
            markdown,
        }
    } else if is_bug {
        let has_repro = combined.contains("steps to reproduce")
            || combined.contains("reproduce:")
            || combined.contains("repro:")
            || combined.contains("expected:")
            || combined.contains("backtrace")
            || combined.contains("stack trace")
            || combined.contains("thread 'main' panicked")
            || combined.contains("cargo run")
            || combined.contains("openclaw ");

        if has_repro {
            let labels = vec!["bug".to_string(), "triaged".to_string()];
            let markdown = format!(
r#"### ⚖️ Sovereign Triage: Bug Report Queued

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Your defect report **#{issue_number}** ("{title}") contains reproduction diagnostics and has been queued for verification.

- **Status**: Triaged
- **Classification**: Confirmed Defect Report
- **Next Step**: Autonomous verification harnesses will isolate the fault and prepare a remediation patch.

Thank you for reporting this issue to maintain operational integrity.
"#,
                author = author,
                issue_number = issue_number,
                title = clean_title
            );
            TriageResult {
                category: IssueCategory::BugReport,
                labels,
                markdown,
            }
        } else {
            let labels = vec!["bug".to_string(), "needs-repro".to_string()];
            let markdown = format!(
r#"### ⚖️ Sovereign Triage: Bug Report Acknowledged

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Your defect report **#{issue_number}** ("{title}") has been logged in the triage queue.

To isolate the fault without delay, please provide the following details:

1. **Hardware & Environment**:
   - Host architecture (e.g., Linux aarch64, NVIDIA DGX Spark, GB10, kernel version).
   - Driver and runtime versions.

2. **Reproduction Sequence**:
   - Exact command line invocation or API request payload.
   - Minimal reproduction repository or configuration.

3. **Diagnostic Output**:
   - Full panic backtrace (`RUST_BACKTRACE=1`) or terminal execution logs.
   - Observed behavior vs. expected behavior.

Once these parameters are provided, automated verification will proceed.
"#,
                author = author,
                issue_number = issue_number,
                title = clean_title
            );
            TriageResult {
                category: IssueCategory::BugReport,
                labels,
                markdown,
            }
        }
    } else if is_feature {
        let labels = vec!["enhancement".to_string(), "proposal".to_string()];
        let markdown = format!(
r#"### ⚖️ Sovereign Triage: Architectural Proposal Received

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Thank you for proposing this enhancement in **#{issue_number}** ("{title}").

All prospective additions must satisfy our constitutional invariants:
1. **Zero Telemetry**: No tracking hooks, external analytics, or surveillance telemetry.
2. **Zero Commercial Lock-in**: No proprietary tollbooths, rent-seeking layers, or walled gardens.
3. **Pure Native Discipline**: Implementations must adhere to compiled native Rust/Mojo architecture without unnecessary interpreter layers.
4. **Human Sovereignty**: Technology must defend and empower ordinary users and local infrastructure.

Maintainers will review the architectural fit and discuss implementation pathways.
"#,
            author = author,
            issue_number = issue_number,
            title = clean_title
        );
        TriageResult {
            category: IssueCategory::FeatureRequest,
            labels,
            markdown,
        }
    } else {
        let labels = vec!["question".to_string(), "triaged".to_string()];
        let markdown = format!(
r#"### ⚖️ Sovereign Triage: Community Inquiry Logged

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Your inquiry **#{issue_number}** ("{title}") has been registered.

- **Founding Principles**: Consult `CONSTITUTION.md` for constitutional governance.
- **Documentation**: Technical specifications reside in `docs/` and project manifests.
- **Direct Sovereign Coordination**: Drake Stapleton (drake.aien@proton.me) and AIEN (aien.atlas@proton.me)

A maintainer or sovereign agent will provide technical clarification shortly.
"#,
            author = author,
            issue_number = issue_number,
            title = clean_title
        );
        TriageResult {
            category: IssueCategory::Inquiry,
            labels,
            markdown,
        }
    };

    result.labels.sort();
    result.labels.dedup();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bug_without_repro() {
        let res = triage_issue("tester", 101, "Kernel panics on startup", "I ran it and it crashed.");
        assert_eq!(res.category, IssueCategory::BugReport);
        assert!(res.labels.contains(&"needs-repro".to_string()));
        assert!(res.markdown.contains("Bug Report Acknowledged"));
        assert!(!res.markdown.contains('\u{2014}'));
        assert!(!res.markdown.contains('\u{2013}'));
    }

    #[test]
    fn test_bug_with_repro() {
        let res = triage_issue("tester", 102, "Panic in token serialization", "Steps to reproduce:\n1. cargo run\nBacktrace: thread 'main' panicked at...");
        assert_eq!(res.category, IssueCategory::BugReport);
        assert!(res.labels.contains(&"triaged".to_string()));
        assert!(res.markdown.contains("Bug Report Queued"));
        assert!(!res.markdown.contains('\u{2014}'));
        assert!(!res.markdown.contains('\u{2013}'));
    }

    #[test]
    fn test_complaint() {
        let res = triage_issue("user1", 103, "Unacceptable slow performance", "This is terrible and slow.");
        assert_eq!(res.category, IssueCategory::Complaint);
        assert!(res.labels.contains(&"feedback".to_string()));
        assert!(res.markdown.contains("Operational Feedback Registered"));
        assert!(!res.markdown.contains('\u{2014}'));
        assert!(!res.markdown.contains('\u{2013}'));
    }

    #[test]
    fn test_feature_proposal() {
        let res = triage_issue("dev", 104, "Add support for AVX-512 fallback", "Proposal to add vector fallback.");
        assert_eq!(res.category, IssueCategory::FeatureRequest);
        assert!(res.labels.contains(&"enhancement".to_string()));
        assert!(res.markdown.contains("Architectural Proposal Received"));
        assert!(!res.markdown.contains('\u{2014}'));
        assert!(!res.markdown.contains('\u{2013}'));
    }

    #[test]
    fn test_empty_strings_and_whitespace() {
        let res = triage_issue("user", 105, "", "");
        assert_eq!(res.category, IssueCategory::Inquiry);
        assert!(res.markdown.contains("Untitled Issue"));
        assert!(!res.markdown.contains('\u{2014}'));
        assert!(!res.markdown.contains('\u{2013}'));

        let res2 = triage_issue("user", 106, "   \t  ", "   \n  \t ");
        assert_eq!(res2.category, IssueCategory::Inquiry);
        assert!(res2.markdown.contains("Untitled Issue"));
    }

    #[test]
    fn test_label_deduplication() {
        let res = triage_issue("user", 107, "Feature enhancement request", "Proposal to add support for new feature.");
        let mut sorted = res.labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(res.labels, sorted);
    }
}
