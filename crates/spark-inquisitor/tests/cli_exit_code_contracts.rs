use std::fs;
use std::process::Command;

fn get_temp_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let pid = std::process::id();
    path.push(format!("inquisitor_{}_{}", pid, name));
    path
}

#[test]
fn test_cli_doctor_exit_zero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let output = Command::new(bin)
        .arg("doctor")
        .output()
        .expect("Failed to execute spark-inquisitor doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("⚖️ Sovereign Inquisitor Diagnostics: OK"));
    assert!(stdout.contains("Invariant rules: Active"));
}

#[test]
fn test_cli_audit_clean_diff_exit_zero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let diff_path = get_temp_path("clean.diff");
    fs::write(&diff_path, "diff --git a/clean.rs b/clean.rs\n+pub fn ok() -> bool { true }\n").unwrap();

    let output = Command::new(bin)
        .arg("audit")
        .arg("--diff")
        .arg(&diff_path)
        .output()
        .expect("Failed to execute audit");

    let _ = fs::remove_file(&diff_path);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Diff is clean"));
}

#[test]
fn test_cli_audit_telemetry_diff_exit_nonzero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let diff_path = get_temp_path("telemetry.diff");
    fs::write(&diff_path, "diff --git a/bad.rs b/bad.rs\n+let _ = send_telemetry(\"google-analytics\");\n").unwrap();

    let output = Command::new(bin)
        .arg("audit")
        .arg("--diff")
        .arg(&diff_path)
        .output()
        .expect("Failed to execute audit");

    let _ = fs::remove_file(&diff_path);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Constitutional audit failed"));
    assert!(stderr.contains("google-analytics"));
}

#[test]
fn test_cli_audit_em_dash_diff_exit_nonzero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let diff_path = get_temp_path("dash.diff");
    fs::write(&diff_path, "diff --git a/dash.rs b/dash.rs\n+// note \u{2014} bad dash\n").unwrap();

    let output = Command::new(bin)
        .arg("audit")
        .arg("--diff")
        .arg(&diff_path)
        .output()
        .expect("Failed to execute audit");

    let _ = fs::remove_file(&diff_path);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Constitutional audit failed"));
    assert!(stderr.contains("Em/En dash detected"));
}

#[test]
fn test_cli_audit_secret_diff_exit_nonzero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let diff_path = get_temp_path("secret.diff");
    fs::write(&diff_path, "diff --git a/sec.rs b/sec.rs\n+const KEY: &str = \"AKIA1234567890ABCDEF\";\n").unwrap();

    let output = Command::new(bin)
        .arg("audit")
        .arg("--diff")
        .arg(&diff_path)
        .output()
        .expect("Failed to execute audit");

    let _ = fs::remove_file(&diff_path);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Plaintext AWS Access Key detected"));
}

#[test]
fn test_cli_review_clean_diff_exit_zero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let diff_path = get_temp_path("rev_clean.diff");
    fs::write(&diff_path, "diff --git a/clean.rs b/clean.rs\n+pub fn check() -> i32 { 42 }\n").unwrap();

    let output = Command::new(bin)
        .arg("review")
        .arg("--author")
        .arg("tester")
        .arg("--pr")
        .arg("50")
        .arg("--title")
        .arg("Clean contribution")
        .arg("--diff")
        .arg(&diff_path)
        .output()
        .expect("Failed to execute review");

    let _ = fs::remove_file(&diff_path);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Constitutional Diff Audit: PASS"));
    assert!(stdout.contains("Contributor Alignment Interview"));
}

#[test]
fn test_cli_review_violating_diff_exit_nonzero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let diff_path = get_temp_path("rev_bad.diff");
    fs::write(&diff_path, "diff --git a/bad.rs b/bad.rs\n+// This is a game-changer feature\n").unwrap();

    let output = Command::new(bin)
        .arg("review")
        .arg("--author")
        .arg("violator")
        .arg("--pr")
        .arg("51")
        .arg("--title")
        .arg("Violating contribution")
        .arg("--diff")
        .arg(&diff_path)
        .output()
        .expect("Failed to execute review");

    let _ = fs::remove_file(&diff_path);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Constitutional Diff Audit: FAILED"));
}

#[test]
fn test_cli_evaluate_valid_testimony_exit_zero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let testimony_path = get_temp_path("testimony_valid.txt");
    fs::write(
        &testimony_path,
        "I solemnly certify and affirm the Sovereign Contributor Oath. We ensure zero telemetry and build for human sovereignty and freedom.",
    ).unwrap();

    let output = Command::new(bin)
        .arg("evaluate")
        .arg("--author")
        .arg("sovereign_dev")
        .arg("--testimony")
        .arg(&testimony_path)
        .output()
        .expect("Failed to execute evaluate");

    let _ = fs::remove_file(&testimony_path);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PASS:"));
    assert!(stdout.contains("`sovereign-interview-passed`"));
}

#[test]
fn test_cli_evaluate_invalid_testimony_exit_nonzero() {
    let bin = env!("CARGO_BIN_EXE_spark-inquisitor");
    let testimony_path = get_temp_path("testimony_invalid.txt");
    fs::write(&testimony_path, "Just testing something out, no oath here.").unwrap();

    let output = Command::new(bin)
        .arg("evaluate")
        .arg("--author")
        .arg("random_user")
        .arg("--testimony")
        .arg(&testimony_path)
        .output()
        .expect("Failed to execute evaluate");

    let _ = fs::remove_file(&testimony_path);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("FAIL:"));
}
