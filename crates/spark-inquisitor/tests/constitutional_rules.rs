use spark_inquisitor::audit::DiffAuditor;

#[test]
fn test_em_dash_detected_in_code_comments() {
    let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+// Important note \u{2014} do not delete this function\npub fn keep() {}";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("Em/En dash detected")));
}

#[test]
fn test_em_dash_detected_in_string_literals() {
    let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+pub const SLOGAN: &str = \"speed \u{2014} power\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("Em/En dash detected")));
}

#[test]
fn test_em_dash_detected_in_markdown() {
    let diff = "diff --git a/README.md b/README.md\n+- Core Feature \u{2014} high throughput engine";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("Em/En dash detected")));
}

#[test]
fn test_en_dash_detected_in_code_comments() {
    let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+// Range 10\u{2013}20 items";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("Em/En dash detected")));
}

#[test]
fn test_en_dash_detected_in_string_literals() {
    let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+let header = \"Version 1.0\u{2013}beta\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("Em/En dash detected")));
}

#[test]
fn test_en_dash_detected_in_markdown() {
    let diff = "diff --git a/DOCS.md b/DOCS.md\n+Pages 5\u{2013}10 contain benchmarks.";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("Em/En dash detected")));
}

#[test]
fn test_banned_buzzword_delve() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// Let us delve into the architecture";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("delve")));
}

#[test]
fn test_banned_buzzword_tapestry() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// A rich tapestry of microservices";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("tapestry")));
}

#[test]
fn test_banned_buzzword_testament() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// This code is a testament to our engineering";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("testament")));
}

#[test]
fn test_banned_buzzword_beacon_trope() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// Standing as a beacon of hope for open source";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("beacon")));
}

#[test]
fn test_beacon_hardware_term_allowed() {
    let diff = "diff --git a/src/net.rs b/src/net.rs\n+fn parse_ble_beacon_packet(frame: &[u8]) -> bool { true }";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_banned_buzzword_crucial() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// It is crucial to preserve cache coherence";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("crucial")));
}

#[test]
fn test_banned_buzzword_pivotal() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// A pivotal refactor for the memory engine";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("pivotal")));
}

#[test]
fn test_banned_buzzword_elevate() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// Designed to elevate the computing experience";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("elevate")));
}

#[test]
fn test_elevate_flag_allowed() {
    let diff = "diff --git a/src/cli.rs b/src/cli.rs\n+let flag = \"--elevate-privileges\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_banned_buzzword_game_changer() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// The new allocator is a game-changer";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("game-changer")));
}

#[test]
fn test_banned_buzzword_unleash() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// Unleash the power of local silicon";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("unleash")));
}

#[test]
fn test_unleash_flag_allowed() {
    let diff = "diff --git a/src/args.rs b/src/args.rs\n+if args.contains(&\"--unleash-features\".to_string()) {}";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_banned_buzzword_harness_verb() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// We can harness the full throughput of GPU nodes";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("harness")));
}

#[test]
fn test_harness_noun_and_test_harness_allowed() {
    let diff = "diff --git a/src/harness.rs b/src/harness.rs\n+// Verification test harness runner\npub struct TestHarness;";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_banned_buzzword_seamlessly() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// Coordinates seamlessly across all instances";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("seamlessly")));
}

#[test]
fn test_antithesis_trope_not_x_but_y() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// It is not speed, but correctness that matters";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("antithesis trope")));
}

#[test]
fn test_antithesis_trope_not_only_x_but_y() {
    let diff = "diff --git a/src/main.rs b/src/main.rs\n+// Not only fast, but completely reliable across all tests";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("antithesis trope")));
}

#[test]
fn test_plaintext_secret_aws_access_key() {
    let diff = "diff --git a/src/vault.rs b/src/vault.rs\n+let aws_key = \"AKIA1234567890ABCDEF\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.secret_violations.iter().any(|s| s.contains("AWS Access Key")));
}

#[test]
fn test_plaintext_secret_github_pat() {
    let diff = "diff --git a/src/auth.rs b/src/auth.rs\n+let token = \"ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.secret_violations.iter().any(|s| s.contains("GitHub Personal Access Token")));
}

#[test]
fn test_plaintext_secret_github_fine_grained_pat() {
    let diff = "diff --git a/src/auth.rs b/src/auth.rs\n+let token = \"github_pat_11AAAAAAA01234567890abcdefghijklmnopqrstuvwxyz_01234567890abcdef\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.secret_violations.iter().any(|s| s.contains("GitHub Personal Access Token")));
}

#[test]
fn test_plaintext_secret_rsa_private_key() {
    let diff = "diff --git a/src/crypto.rs b/src/crypto.rs\n+const KEY: &str = \"-----BEGIN RSA PRIVATE KEY-----\\nMIIEowIBAAKCAQEA...\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.secret_violations.iter().any(|s| s.contains("Private Key header")));
}

#[test]
fn test_plaintext_secret_generic_private_key() {
    let diff = "diff --git a/src/crypto.rs b/src/crypto.rs\n+const KEY: &str = \"-----BEGIN PRIVATE KEY-----\\nMIIEowIBAAKCAQEA...\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.secret_violations.iter().any(|s| s.contains("Private Key header")));
}

#[test]
fn test_env_file_addition_root() {
    let diff = "diff --git a/.env b/.env\nnew file mode 100644\n--- /dev/null\n+++ b/.env\n@@ -0,0 +1,2 @@\n+PORT=8080\n+HOST=127.0.0.1";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.violations.iter().any(|v| v.contains(".env")));
}

#[test]
fn test_env_file_addition_nested() {
    let diff = "diff --git a/crates/backend/.env.production b/crates/backend/.env.production\nnew file mode 100644\n--- /dev/null\n+++ b/crates/backend/.env.production\n@@ -0,0 +1,1 @@\n+NODE_ENV=production";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.violations.iter().any(|v| v.contains(".env")));
}

#[test]
fn test_false_positive_markdown_lists() {
    let diff = "diff --git a/docs/README.md b/docs/README.md\n+- First item in checklist\n+- Second item in checklist\n+* Third item in checklist";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_false_positive_negative_numbers_and_subtraction() {
    let diff = "diff --git a/src/calc.rs b/src/calc.rs\n+let delta = a - b;\n+let temp = -40;\n+let offset = x - y - z;";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_false_positive_standard_hyphenated_words() {
    let diff = "diff --git a/src/arch.rs b/src/arch.rs\n+// Zero-telemetry multi-threaded aarch64-unknown-linux-gnu runtime\npub const TARGET: &str = \"aarch64-unknown-linux-gnu\";";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}
