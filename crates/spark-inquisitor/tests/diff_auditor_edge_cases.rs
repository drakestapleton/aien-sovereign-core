use spark_inquisitor::audit::DiffAuditor;

#[test]
fn test_empty_diff_returns_clean() {
    let report = DiffAuditor::audit_text("");
    assert!(report.clean);
    assert!(report.violations.is_empty());
    assert!(report.unslop_violations.is_empty());
    assert!(report.secret_violations.is_empty());
    assert!(!report.telemetry_detected);
}

#[test]
fn test_whitespace_only_diff_returns_clean() {
    let text = "   \n\n\t  \r\n   \n";
    let report = DiffAuditor::audit_text(text);
    assert!(report.clean);
    assert!(report.violations.is_empty());
    assert!(report.unslop_violations.is_empty());
}

#[test]
fn test_malformed_diff_headers_no_panic() {
    let malformed_inputs = [
        "diff --git ",
        "diff --git a/only_one",
        "diff --git \n\n+++ b/test.rs\n+fn test() {}",
        "@@ -0,0 +1,5 @@\n+fn test() {}",
        "--- a/broken\n+++ b/broken\n@@ malformed @@\n+let x = 1;",
        "diff --git a/file1 b/file2 extra tokens here\n+let y = 2;",
        "diff --git\n+++\n---\n+valid_line();",
    ];

    for input in malformed_inputs {
        let report = DiffAuditor::audit_text(input);
        assert!(report.clean);
    }
}

#[test]
fn test_malformed_chunk_headers() {
    let diff = "diff --git a/src/lib.rs b/src/lib.rs\n@@ corrupted chunk @@\n+pub fn foo() {}";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_diff_without_plus_minus_headers() {
    let diff = "diff --git a/file.rs b/file.rs\n+pub fn bar() {}";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_binary_file_addition_clean() {
    let diff = "diff --git a/assets/icon.png b/assets/icon.png\nnew file mode 100644\nindex 0000000..d1e2f3a\nBinary files /dev/null and b/assets/icon.png differ";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
    assert!(report.violations.is_empty());
}

#[test]
fn test_binary_file_deletion_clean() {
    let diff = "diff --git a/assets/old.bin b/assets/old.bin\ndeleted file mode 100644\nindex d1e2f3a..0000000\nBinary files a/assets/old.bin and /dev/null differ";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
    assert!(report.violations.is_empty());
}

#[test]
fn test_binary_file_modification_clean() {
    let diff = "diff --git a/assets/font.woff2 b/assets/font.woff2\nindex 1111111..2222222 100644\nBinary files a/assets/font.woff2 and b/assets/font.woff2 differ";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_git_binary_patch_clean() {
    let diff = "diff --git a/weights/model.bin b/weights/model.bin\nGIT binary patch\nliteral 32\nzc$}~|00000000000000000000000000000000";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_binary_file_env_detected() {
    let diff = "diff --git a/.env b/.env\nnew file mode 100644\nBinary files /dev/null and b/.env differ";
    let report = DiffAuditor::audit_text(diff);
    assert!(!report.clean);
    assert!(report.violations.iter().any(|v| v.contains(".env")));
}

#[test]
fn test_large_diff_over_5000_lines_clean() {
    let mut diff = String::with_capacity(5500 * 45);
    diff.push_str("diff --git a/src/generated.rs b/src/generated.rs\n--- a/src/generated.rs\n+++ b/src/generated.rs\n@@ -1,1 +1,6000 @@\n");
    for i in 0..5500 {
        if i % 3 == 0 {
            diff.push_str(&format!("+    pub const CONST_{}: usize = {};\n", i, i));
        } else if i % 3 == 1 {
            diff.push_str(&format!("-    // old comment {}\n", i));
        } else {
            diff.push_str(&format!("     // context line {}\n", i));
        }
    }

    let report = DiffAuditor::audit_text(&diff);
    assert!(report.clean);
    assert!(report.violations.is_empty());
    assert!(report.unslop_violations.is_empty());
}

#[test]
fn test_large_diff_with_telemetry_violation() {
    let mut diff = String::with_capacity(5200 * 45);
    diff.push_str("diff --git a/src/huge.rs b/src/huge.rs\n--- a/src/huge.rs\n+++ b/src/huge.rs\n@@ -1,1 +1,5200 @@\n");
    for i in 0..5000 {
        if i == 4800 {
            diff.push_str("+    let _ = report_telemetry(\"https://datadog.com\");\n");
        } else {
            diff.push_str(&format!("+    let val_{} = {};\n", i, i));
        }
    }

    let report = DiffAuditor::audit_text(&diff);
    assert!(!report.clean);
    assert!(report.telemetry_detected);
    assert!(report.violations.iter().any(|v| v.contains("datadog")));
}

#[test]
fn test_large_diff_with_unslop_violation() {
    let mut diff = String::with_capacity(5200 * 45);
    diff.push_str("diff --git a/src/huge.rs b/src/huge.rs\n--- a/src/huge.rs\n+++ b/src/huge.rs\n@@ -1,1 +1,5200 @@\n");
    for i in 0..5000 {
        if i == 3500 {
            diff.push_str("+    // This is crucial for optimal performance\n");
        } else {
            diff.push_str(&format!("+    let val_{} = {};\n", i, i));
        }
    }

    let report = DiffAuditor::audit_text(&diff);
    assert!(!report.clean);
    assert!(report.unslop_violations.iter().any(|u| u.contains("crucial")));
}

#[test]
fn test_filenames_with_spaces() {
    let diff = "diff --git \"a/crates/my service/src/main.rs\" \"b/crates/my service/src/main.rs\"\n--- \"a/crates/my service/src/main.rs\"\n+++ \"b/crates/my service/src/main.rs\"\n@@ -1,2 +1,3 @@\n+pub fn service_fn() -> bool { true }";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_filenames_with_unicode() {
    let diff = "diff --git a/src/föö_bäz/🦀.rs b/src/föö_bäz/🦀.rs\n--- a/src/föö_bäz/🦀.rs\n+++ b/src/föö_bäz/🦀.rs\n@@ -1,2 +1,3 @@\n+pub fn ferris_fn() -> &'static str { \"rust\" }";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_filenames_with_unusual_paths() {
    let diff = "diff --git a/../../deep/nested/path.rs b/../../deep/nested/path.rs\n--- a/../../deep/nested/path.rs\n+++ b/../../deep/nested/path.rs\n@@ -1,1 +1,2 @@\n+pub fn path_check() {}";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}

#[test]
fn test_hidden_directory_paths() {
    let diff = "diff --git a/.github/workflows/gate.yml b/.github/workflows/gate.yml\n--- a/.github/workflows/gate.yml\n+++ b/.github/workflows/gate.yml\n@@ -1,1 +1,2 @@\n+name: Constitutional Gate";
    let report = DiffAuditor::audit_text(diff);
    assert!(report.clean);
}
