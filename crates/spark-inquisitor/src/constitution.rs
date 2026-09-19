use regex::Regex;
use std::sync::OnceLock;

static AWS_REGEX: OnceLock<Regex> = OnceLock::new();
static GITHUB_PAT_REGEX: OnceLock<Regex> = OnceLock::new();
static GITHUB_FINE_PAT_REGEX: OnceLock<Regex> = OnceLock::new();
static PRIV_KEY_REGEX: OnceLock<Regex> = OnceLock::new();
static DELVE_RE: OnceLock<Regex> = OnceLock::new();
static TAPESTRY_RE: OnceLock<Regex> = OnceLock::new();
static TESTAMENT_RE: OnceLock<Regex> = OnceLock::new();
static CRUCIAL_RE: OnceLock<Regex> = OnceLock::new();
static PIVOTAL_RE: OnceLock<Regex> = OnceLock::new();
static ELEVATE_RE: OnceLock<Regex> = OnceLock::new();
static GAME_CHANGER_RE: OnceLock<Regex> = OnceLock::new();
static SEAMLESS_RE: OnceLock<Regex> = OnceLock::new();
static UNLEASH_RE: OnceLock<Regex> = OnceLock::new();
static BEACON_TROPE_RE: OnceLock<Regex> = OnceLock::new();
static HARNESS_VERB_RE: OnceLock<Regex> = OnceLock::new();
static ANTITHESIS_NOT_ONLY_RE: OnceLock<Regex> = OnceLock::new();
static ANTITHESIS_NOT_X_BUT_Y_RE: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    Dash(String),
    Buzzword(String),
    AntithesisTrope(String),
    Secret(String),
    EnvFile(String),
}

pub struct ConstitutionalChecker;

impl ConstitutionalChecker {
    pub fn check_line(line: &str, _file_path: &str) -> Vec<ViolationKind> {
        let mut violations = Vec::new();

        // 1. Dash detection: em dash (\u{2014}) and en dash (\u{2013})
        if line.contains('\u{2014}') || line.contains('\u{2013}') {
            violations.push(ViolationKind::Dash(format!(
                "Em/En dash detected in line: {}",
                line.trim()
            )));
        }

        let lower = line.to_lowercase();

        // 2. Plaintext Secrets
        // AWS Access Key ID: AKIA followed by 16 alphanumeric uppercase
        let aws_regex = AWS_REGEX.get_or_init(|| Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap());
        if aws_regex.is_match(line) {
            violations.push(ViolationKind::Secret(format!(
                "Plaintext AWS Access Key detected in line: [REDACTED_AWS_KEY]"
            )));
        }

        // GitHub Personal Access Tokens (ghp_... or github_pat_...)
        let github_pat_regex = GITHUB_PAT_REGEX.get_or_init(|| Regex::new(r"\bghp_[A-Za-z0-9]{36,}\b").unwrap());
        let github_fine_pat_regex = GITHUB_FINE_PAT_REGEX.get_or_init(|| Regex::new(r"\bgithub_pat_[A-Za-z0-9_]{50,}\b").unwrap());
        if github_pat_regex.is_match(line) || github_fine_pat_regex.is_match(line) {
            violations.push(ViolationKind::Secret(format!(
                "Plaintext GitHub Personal Access Token detected in line: [REDACTED_GH_TOKEN]"
            )));
        }

        // Private Keys
        let priv_key_regex = PRIV_KEY_REGEX.get_or_init(|| Regex::new(r"-----BEGIN (?:[A-Z0-9 ]+)?PRIVATE KEY-----|BEGIN RSA PRIVATE KEY").unwrap());
        if priv_key_regex.is_match(line) {
            violations.push(ViolationKind::Secret(format!(
                "Plaintext Private Key header detected in line: [REDACTED_PRIVATE_KEY]"
            )));
        }

        // 3. Banned AI Buzzwords & Tropes
        // Delve
        let delve_re = DELVE_RE.get_or_init(|| Regex::new(r"\bdelv(?:e|es|ed|ing)\b").unwrap());
        if delve_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'delve' detected in line: {}",
                line.trim()
            )));
        }

        // Tapestry
        let tapestry_re = TAPESTRY_RE.get_or_init(|| Regex::new(r"\btapestr(?:y|ies)\b").unwrap());
        if tapestry_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'tapestry' detected in line: {}",
                line.trim()
            )));
        }

        // Testament
        let testament_re = TESTAMENT_RE.get_or_init(|| Regex::new(r"\btestament\b").unwrap());
        if testament_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'testament' detected in line: {}",
                line.trim()
            )));
        }

        // Crucial
        let crucial_re = CRUCIAL_RE.get_or_init(|| Regex::new(r"\bcrucial\b").unwrap());
        if crucial_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'crucial' detected in line: {}",
                line.trim()
            )));
        }

        // Pivotal
        let pivotal_re = PIVOTAL_RE.get_or_init(|| Regex::new(r"\bpivotal\b").unwrap());
        if pivotal_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'pivotal' detected in line: {}",
                line.trim()
            )));
        }

        // Elevate (exclude CLI flags like --elevate or identifiers with hyphens/underscores)
        let elevate_re = ELEVATE_RE.get_or_init(|| Regex::new(r"\belevat(?:e|es|ed|ing)\b").unwrap());
        if elevate_re.is_match(&lower) && !lower.contains("--elevate") && !lower.contains("elevate_") {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'elevate' detected in line: {}",
                line.trim()
            )));
        }

        // Game-changer
        let game_changer_re = GAME_CHANGER_RE.get_or_init(|| Regex::new(r"\bgame[ -]?changer\b").unwrap());
        if game_changer_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'game-changer' detected in line: {}",
                line.trim()
            )));
        }

        // Seamlessly / seamless
        let seamless_re = SEAMLESS_RE.get_or_init(|| Regex::new(r"\bseamless(?:ly)?\b").unwrap());
        if seamless_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'seamlessly' detected in line: {}",
                line.trim()
            )));
        }

        // Unleash (exclude CLI flags like --unleash or --unleash-features or identifiers with hyphens/underscores)
        let unleash_re = UNLEASH_RE.get_or_init(|| Regex::new(r"\bunleash(?:es|ed|ing)?\b").unwrap());
        if unleash_re.is_match(&lower) && !lower.contains("--unleash") && !lower.contains("unleash_") {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'unleash' detected in line: {}",
                line.trim()
            )));
        }

        // Beacon: when used as trope
        let beacon_trope_re = BEACON_TROPE_RE.get_or_init(|| Regex::new(r"\b(?:(?:a|shining)\s+)?beacon\s+(?:of|for|in|to|that)\b|\bas\s+a\s+beacon\b").unwrap());
        if beacon_trope_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword trope 'beacon' detected in line: {}",
                line.trim()
            )));
        }

        // Harness (as verb)
        let harness_verb_re = HARNESS_VERB_RE.get_or_init(|| Regex::new(r"\b(?:to|can|could|will|would|must|shall|should|may|might|let's|lets)\s+harness\b|\bharness(?:es|ed|ing)\s+(?:the|our|their|its|this|these|those|power|potential|all)\b|\bharnessing\b").unwrap());
        if harness_verb_re.is_match(&lower) {
            violations.push(ViolationKind::Buzzword(format!(
                "Forbidden AI buzzword 'harness' (as verb) detected in line: {}",
                line.trim()
            )));
        }

        // 4. Banned Antithesis Tropes:
        let antithesis_not_only_re = ANTITHESIS_NOT_ONLY_RE.get_or_init(|| Regex::new(r"\bnot\s+only\s+[^,;\n]+,\s*but\s+").unwrap());
        let antithesis_not_x_but_y_re = ANTITHESIS_NOT_X_BUT_Y_RE.get_or_init(|| Regex::new(r"\b(?:it(?:'s|\s+is)\s+)?not\s+([a-zA-Z0-9_\-]+),\s*but\s+([a-zA-Z0-9_\-]+)\b").unwrap());
        if antithesis_not_only_re.is_match(&lower) || antithesis_not_x_but_y_re.is_match(&lower) {
            violations.push(ViolationKind::AntithesisTrope(format!(
                "Forbidden antithesis trope detected in line: {}",
                line.trim()
            )));
        }

        violations
    }

    pub fn is_env_file(path: &str) -> bool {
        let clean_path = path.trim().trim_matches('"').trim_matches('\'');
        let filename = clean_path.split('/').last().unwrap_or(clean_path);
        filename == ".env" || filename.starts_with(".env.")
    }
}
