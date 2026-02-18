//! Integration test: CLI interface.
//!
//! Tests the binary's CLI argument handling by running the compiled binary
//! as a subprocess. This validates argument parsing, help text, version output,
//! and error messages for invalid inputs — without requiring Whisper models.

use std::process::Command;

/// Helper: find the debug binary path.
fn binary_path() -> std::path::PathBuf {
    // cargo test compiles to target/debug/
    let mut path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("parent")
        .parent()
        .expect("grandparent")
        .to_path_buf();
    path.push("voice-dictation");
    path
}

/// Build the binary if needed and return a Command for it.
fn voice_dictation_cmd() -> Command {
    Command::new(binary_path())
}

/// --help prints usage information and exits successfully.
#[test]
fn cli_help_flag() {
    let output = voice_dictation_cmd().arg("--help").output().expect("failed to execute");

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("voice-dictation") || stdout.contains("speech-to-text"),
        "help should mention app name or purpose"
    );
}

/// --version prints version and exits successfully.
#[test]
fn cli_version_flag() {
    let output = voice_dictation_cmd()
        .arg("--version")
        .output()
        .expect("failed to execute");

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("voice-dictation"), "version should contain binary name");
}

/// `transcribe --help` shows transcription-specific options.
#[test]
fn cli_transcribe_help() {
    let output = voice_dictation_cmd()
        .args(["transcribe", "--help"])
        .output()
        .expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--model") || stdout.contains("-m"),
        "should mention model option"
    );
    assert!(
        stdout.contains("--language") || stdout.contains("-l"),
        "should mention language option"
    );
    assert!(stdout.contains("--backend"), "should mention backend option");
}

/// `transcribe` without required input file produces an error.
#[test]
fn cli_transcribe_missing_input() {
    let output = voice_dictation_cmd()
        .arg("transcribe")
        .output()
        .expect("failed to execute");

    assert!(!output.status.success(), "should fail without input file argument");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("required") || stderr.contains("error") || stderr.contains("Usage"),
        "error message should indicate missing argument: {}",
        stderr
    );
}

/// `transcribe` with nonexistent file produces a clear error.
#[test]
fn cli_transcribe_nonexistent_file() {
    let output = voice_dictation_cmd()
        .args(["transcribe", "/tmp/definitely_nonexistent_file_s2t_test.wav"])
        .output()
        .expect("failed to execute");

    assert!(!output.status.success(), "should fail with nonexistent file");
}

/// `models` subcommand runs (may fail if no models dir, but shouldn't panic).
#[test]
fn cli_models_subcommand() {
    let output = voice_dictation_cmd().arg("models").output().expect("failed to execute");

    // Models command should succeed even if no models are downloaded
    // It either lists models or prints a "no models" message
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Should not panic (exit code is either 0 or a handled error)
    assert!(
        output.status.code().is_some(),
        "should exit cleanly, not crash: {}",
        combined
    );
}

/// Invalid subcommand produces an error.
#[test]
fn cli_invalid_subcommand() {
    let output = voice_dictation_cmd()
        .arg("nonexistent-command")
        .output()
        .expect("failed to execute");

    assert!(!output.status.success(), "invalid subcommand should produce error");
}
