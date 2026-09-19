use crate::constitution::{ConstitutionalChecker, ViolationKind};

#[derive(Debug, Clone)]
pub struct AuditReport {
    pub clean: bool,
    pub violations: Vec<String>,
    pub telemetry_detected: bool,
    pub unslop_violations: Vec<String>,
    pub secret_violations: Vec<String>,
}

impl AuditReport {
    pub fn generate_markdown_report(&self) -> String {
        if self.clean {
            return "#### ✅ Constitutional Diff Audit: PASS\n- **Telemetry Check**: Clean (zero tracking or surveillance hooks detected).\n- **Sovereign Voice & Formatting Check**: Clean (zero em/en dashes, zero banned buzzwords).\n- **Secrets & Credentials Check**: Clean (zero plaintext secrets, hardware vault only).\n- **Status**: Constitutional invariants verified.".to_string();
        }

        let mut out = String::from("#### ❌ Constitutional Diff Audit: FAILED\n\n");
        if !self.violations.is_empty() {
            out.push_str("**Telemetry, Surveillance, and Environment Violations:**\n");
            for v in &self.violations {
                out.push_str(&format!("* {}\n", v));
            }
            out.push('\n');
        }
        if !self.secret_violations.is_empty() {
            out.push_str("**Plaintext Secrets and Credential Violations:**\n");
            for s in &self.secret_violations {
                out.push_str(&format!("* {}\n", s));
            }
            out.push('\n');
        }
        if !self.unslop_violations.is_empty() {
            out.push_str("**Sovereign Voice and Unslop Invariant Violations:**\n");
            for u in &self.unslop_violations {
                out.push_str(&format!("* {}\n", u));
            }
            out.push('\n');
        }
        out.push_str("**Action Required:** Remove all detected tracking hooks, secrets, and forbidden tokens before review can proceed.");
        out
    }
}

pub struct DiffAuditor;

impl DiffAuditor {
    pub fn audit_diff(text: &str) -> AuditReport {
        Self::audit_text(text)
    }

    pub fn audit_text(text: &str) -> AuditReport {
        let mut violations = Vec::new();
        let mut telemetry_detected = false;
        let mut unslop_violations = Vec::new();
        let mut secret_violations = Vec::new();

        let telemetry_patterns = [
            "google-analytics", "segment.io", "mixpanel", "sentry.io",
            "datadog", "tracking_id", "analytics.js",
            "amplitude.com", "hotjar", "clarity.ms",
            "send_telemetry", "report_telemetry", "collect_telemetry",
        ];

        let mut current_file = String::new();
        let mut in_binary_patch = false;

        for line in text.lines() {
            if line.starts_with("diff --git ") {
                in_binary_patch = false;
                current_file = Self::extract_target_file(line).unwrap_or_default();
                if ConstitutionalChecker::is_env_file(&current_file) {
                    violations.push(format!("Forbidden plaintext .env secret file detected in diff header: {}", current_file));
                }
                continue;
            }

            if line.starts_with("+++ ") {
                if let Some(target) = Self::extract_plus_file(line) {
                    current_file = target;
                    if ConstitutionalChecker::is_env_file(&current_file) {
                        violations.push(format!("Forbidden plaintext .env secret file added in diff: {}", current_file));
                    }
                }
                continue;
            }

            if line.starts_with("Binary files ") {
                if let Some(target) = Self::extract_binary_target(line) {
                    if ConstitutionalChecker::is_env_file(&target) {
                        violations.push(format!("Forbidden plaintext .env secret file in binary diff: {}", target));
                    }
                }
                continue;
            }

            if line.starts_with("GIT binary patch") {
                in_binary_patch = true;
                continue;
            }

            if in_binary_patch {
                continue;
            }

            // Exclude inquisitor internal pattern definitions from false positives
            if current_file.contains("spark-inquisitor") || current_file.contains("constitution.rs") {
                continue;
            }

            let lower = line.to_lowercase();

            // Telemetry pattern scanning
            for pattern in &telemetry_patterns {
                if lower.contains(pattern) && (line.starts_with('+') && !line.starts_with("+++")) {
                    violations.push(format!("Telemetry pattern '{}' detected in added line: {}", pattern, line.trim()));
                    telemetry_detected = true;
                }
            }

            // In non-markdown source code, also flag raw telemetry calls
            if !current_file.ends_with(".md") && line.starts_with('+') && !line.starts_with("+++") {
                if lower.contains("telemetry") 
                    && !lower.contains("gpu-telemetry") 
                    && !lower.contains("zero telemetry") && !lower.contains("zero-telemetry") && !lower.contains("no-telemetry") 
                    && !lower.contains("no telemetry") 
                    && !lower.contains("anti-telemetry")
                    && !lower.contains("block telemetry")
                {
                    violations.push(format!("Telemetry indicator detected in source line: {}", line.trim()));
                    telemetry_detected = true;
                }
            }

            // Constitutional unslop and secret invariant scanning on added lines
            if line.starts_with('+') && !line.starts_with("+++") {
                let const_violations = ConstitutionalChecker::check_line(line, &current_file);
                for cv in const_violations {
                    match cv {
                        ViolationKind::Dash(msg) => unslop_violations.push(msg),
                        ViolationKind::Buzzword(msg) => unslop_violations.push(msg),
                        ViolationKind::AntithesisTrope(msg) => unslop_violations.push(msg),
                        ViolationKind::Secret(msg) => secret_violations.push(msg),
                        ViolationKind::EnvFile(msg) => violations.push(msg),
                    }
                }
            }
        }

        let clean = violations.is_empty() && unslop_violations.is_empty() && secret_violations.is_empty();

        AuditReport {
            clean,
            violations,
            telemetry_detected,
            unslop_violations,
            secret_violations,
        }
    }

    fn extract_target_file(line: &str) -> Option<String> {
        let remainder = line.strip_prefix("diff --git ")?.trim();
        if remainder.contains('"') {
            let parts: Vec<&str> = remainder.split("\" \"").collect();
            if parts.len() == 2 {
                let target = parts[1].trim_matches('"');
                let stripped = target.strip_prefix("b/").unwrap_or(target);
                return Some(stripped.to_string());
            }
        }
        let tokens: Vec<&str> = remainder.split_whitespace().collect();
        if tokens.len() >= 2 {
            let target = tokens[1];
            let stripped = target.strip_prefix("b/").unwrap_or(target);
            return Some(stripped.to_string());
        }
        None
    }

    fn extract_plus_file(line: &str) -> Option<String> {
        let path = line.strip_prefix("+++ ")?.trim();
        if path == "/dev/null" {
            return None;
        }
        let clean = path.trim_matches('"');
        let stripped = clean.strip_prefix("b/").unwrap_or(clean);
        Some(stripped.to_string())
    }

    fn extract_binary_target(line: &str) -> Option<String> {
        // e.g. "Binary files /dev/null and b/.env differ"
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() >= 5 {
            let target = tokens[4].trim_matches('"');
            let stripped = target.strip_prefix("b/").unwrap_or(target);
            return Some(stripped.to_string());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_diff() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+pub fn add(a: i32, b: i32) -> i32 {\n+    a + b\n+}";
        let report = DiffAuditor::audit_text(diff);
        assert!(report.clean);
        let md = report.generate_markdown_report();
        assert!(md.contains("PASS"));
        assert!(!md.contains('\u{2014}'));
        assert!(!md.contains('\u{2013}'));
    }

    #[test]
    fn test_telemetry_flagged() {
        let diff = "diff --git a/src/net.rs b/src/net.rs\n+    let _ = send_telemetry(\"https://google-analytics.com/collect\");";
        let report = DiffAuditor::audit_text(diff);
        assert!(!report.clean);
        assert!(report.telemetry_detected);
        let md = report.generate_markdown_report();
        assert!(md.contains("FAILED"));
        assert!(md.contains("google-analytics"));
    }

    #[test]
    fn test_unslop_flagged() {
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+// This is a game-changer feature -- unleash seamless power.";
        let report = DiffAuditor::audit_text(diff);
        assert!(!report.clean);
        assert!(!report.unslop_violations.is_empty());
    }

    #[test]
    fn test_secret_flagged() {
        let diff = "diff --git a/src/config.rs b/src/config.rs\n+let key = \"AKIA1234567890ABCDEF\";";
        let report = DiffAuditor::audit_text(diff);
        assert!(!report.clean);
        assert!(!report.secret_violations.is_empty());
    }

    #[test]
    fn test_env_file_flagged() {
        let diff = "diff --git a/.env b/.env\n+SECRET_KEY=12345";
        let report = DiffAuditor::audit_text(diff);
        assert!(!report.clean);
        assert!(!report.violations.is_empty());
    }
}
