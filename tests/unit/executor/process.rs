use std::path::Path;
use std::time::Duration;

use yt_dlp::executor::{ProcessOutput, execute_command};

// ============================== ProcessOutput Display ==============================

#[test]
fn display_process_output_shows_code_and_len() {
    let output = ProcessOutput {
        stdout: "hello".to_string(),
        stderr: "".to_string(),
        code: 0,
    };
    let display = format!("{}", output);
    assert!(display.contains("ProcessOutput"));
    assert!(display.contains("code=0"));
    assert!(display.contains("stdout_len=5"));
}

#[test]
fn create_process_output_fields_are_accessible() {
    let output = ProcessOutput {
        stdout: "output text".to_string(),
        stderr: "error text".to_string(),
        code: 1,
    };
    assert_eq!(output.stdout, "output text");
    assert_eq!(output.stderr, "error text");
    assert_eq!(output.code, 1);
}

#[test]
fn create_process_output_with_zero_exit_code_succeeds() {
    let output = ProcessOutput {
        stdout: "success".to_string(),
        stderr: "".to_string(),
        code: 0,
    };
    assert_eq!(output.code, 0);
}

#[test]
fn create_process_output_with_nonzero_exit_code_has_stderr() {
    let output = ProcessOutput {
        stdout: "".to_string(),
        stderr: "command failed".to_string(),
        code: 127,
    };
    assert_eq!(output.code, 127);
    assert!(!output.stderr.is_empty());
}

// ============================== execute_command (thin public wrapper) ==============================

#[tokio::test]
async fn execute_command_echo_returns_output() {
    #[cfg(unix)]
    let (cmd, args) = ("echo", vec!["hello"]);
    #[cfg(windows)]
    let (cmd, args) = ("cmd", vec!["/C", "echo hello"]);

    let result = execute_command(
        Path::new(cmd),
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        Duration::from_secs(10),
    )
    .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.code, 0);
    assert!(output.stdout.trim().contains("hello"));
}

#[tokio::test]
async fn execute_command_nonexistent_binary_returns_error() {
    let result = execute_command(
        Path::new("this_binary_does_not_exist_for_testing_12345"),
        &[],
        Duration::from_secs(5),
    )
    .await;
    assert!(result.is_err());
}
