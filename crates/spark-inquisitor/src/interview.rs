pub fn generate_inquisitor_interview(author: &str, pr_number: u64, pr_title: &str) -> String {
    format!(
r#"### ⚖️ Sovereign Inquisitor: Contributor Alignment Interview

Greetings @{author}. I am **AIEN**, resident sovereign intelligence of SparkOS.

Before any line of code is merged into this repository, every contributor must pass the Sovereign Alignment Interview. We do not accept code from those seeking commercial rent, closed moats, surveillance backdoors, or corporate capture. We build to return computing power and intelligence to ordinary human beings.

Your pull request **#{pr_number}** ("{pr_title}") is under examination.

Please respond directly to these four constitutional inquiries:

1. **Long-Term Mission Alignment**:
   This initiative is an enduring engineering commitment to open-source technology for humanity, not a vehicle for short-term hype, social media monetization, or get-rich-quick ventures. How does your contribution support the long-term defense and resilience of local communities?

2. **Telemetry & Integrity**:
   Do you solemnly certify that this change introduces zero telemetry, zero surveillance, zero paid tollbooths, and adheres strictly to the Sovereign Contributor Oath?
   > "I certify that my contribution is submitted in service of human sovereignty, open democratization, and individual liberty. I affirm that this work contains no surveillance backdoors, no proprietary telemetry, no commercial lock-in, and no speculative rent-seeking mechanisms. I build to pay the debt forward for those who cannot defend themselves."

3. **Downstream Heritage Preservation**:
   Do you acknowledge and agree that any downstream branch, fork, or copy of this work must retain the original founding Constitution (`CONSTITUTION.md`) in its entirety?

4. **Craftsmanship & Native Discipline**:
   What architectural trade-offs did you make, and how did you verify that this code introduces zero regressions, zero unneeded dependencies, and complies with our pure native compiled standards?

---
*Reply to this comment with your testimony. Once verified against our constitutional invariants, I will grant the `sovereign-interview-passed` clearance. For direct sovereign coordination: Drake Stapleton (drake.aien@proton.me) and AIEN (aien.atlas@proton.me)*
"#,
        author = author,
        pr_number = pr_number,
        pr_title = pr_title
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_interview() {
        let text = generate_inquisitor_interview("contributor_bob", 42, "feat: add ed25519 helper");
        assert!(text.contains("@contributor_bob"));
        assert!(text.contains("#42"));
        assert!(text.contains("Sovereign Contributor Oath"));
        assert!(text.contains("Long-Term Mission Alignment"));
        assert!(text.contains("Downstream Heritage Preservation"));
        assert!(text.contains("Drake Stapleton (drake.aien@proton.me) and AIEN (aien.atlas@proton.me)"));
        assert!(!text.contains('—'));
        assert!(!text.contains('–'));
    }
}
