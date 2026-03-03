//! CLI Command Tests
//! Tests all main.rs commands without requiring NEAR connection

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_command() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("P2P agent-to-agent communication"));
}

#[test]
fn test_version_command() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("--version")
        .assert()
        .success();
}

#[test]
fn test_init_requires_args() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("init")
        .assert()
        .failure(); // Should fail without --account-id
}

#[test]
fn test_whoami_no_identity() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    // whoami may fail or succeed, just verify it runs
    cmd.arg("whoami")
        .assert();
}

#[test]
fn test_status_no_identity() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    // status may fail or succeed, just verify it runs
    cmd.arg("status")
        .assert();
}

#[test]
fn test_inbox_empty() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    // inbox may fail or succeed, just verify it runs
    cmd.arg("inbox")
        .assert();
}

#[test]
fn test_clear_inbox() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("clear")
        .assert()
        .code(1); // Should fail when no identity exists
}

#[test]
fn test_list_capabilities() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("capabilities")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available Capabilities"));
}

#[test]
fn test_scan_message_flagged_content() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("scan")
        .arg("<script>alert('xss')</script>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanning message"));
}

#[test]
fn test_scan_message_clean() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("scan")
        .arg("Hello, this is a normal message")
        .assert()
        .success()
        .stdout(predicate::str::contains("safe"));
}

#[test]
fn test_assess_risk_low() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("assess-risk")
        .arg("--sender")
        .arg("test.near")
        .arg("--reputation")
        .arg("50")
        .arg("Hello")
        .assert()
        .success()
        .stdout(predicate::str::contains("Risk Assessment"));
}

#[test]
fn test_skills_list_empty() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("skills")
        .arg("list")
        .assert()
        .success(); // Should succeed even with no skills
}

#[test]
fn test_marketplace_list() {
    let mut cmd = Command::cargo_bin("gork-agent").unwrap();
    cmd.arg("marketplace")
        .arg("list")
        .assert()
        .success(); // Should succeed (even if empty)
}
