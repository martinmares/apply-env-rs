use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_apply-env"));
    command
        .args(args)
        .env_remove("CHECK_MISSING_ALPHA")
        .env_remove("CHECK_MISSING_ZED")
        .env_remove("CHECK_DEFAULTED")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().expect("failed to start apply-env");

    child
        .stdin
        .take()
        .expect("child stdin is missing")
        .write_all(input.as_bytes())
        .expect("failed to write template to stdin");

    child
        .wait_with_output()
        .expect("failed to wait for apply-env")
}

fn unique_missing_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "apply-env-check-missing-{}-{timestamp}.tmpl",
        std::process::id()
    ))
}

#[test]
fn check_reports_unique_unresolved_variables_and_fails() {
    let output = run_with_stdin(
        &["check", "-f", "-"],
        "{{CHECK_MISSING_ZED}} {{CHECK_MISSING_ALPHA}} {{CHECK_MISSING_ZED}}",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr is not UTF-8"),
        concat!(
            "ERROR: unresolved template variables:\n",
            "  CHECK_MISSING_ALPHA (1 occurrence)\n",
            "  CHECK_MISSING_ZED (2 occurrences)\n"
        )
    );
}

#[test]
fn check_succeeds_when_variable_is_present() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_apply-env"))
        .args(["check", "-f", "-"])
        .env("APPLY_ENV_CHECK_PRESENT", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start apply-env");

    child
        .stdin
        .take()
        .expect("child stdin is missing")
        .write_all(b"{{APPLY_ENV_CHECK_PRESENT}}")
        .expect("failed to write template to stdin");

    let output = child
        .wait_with_output()
        .expect("failed to wait for apply-env");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout is not UTF-8"),
        "OK: all template variables resolved\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn validate_alias_honors_the_default_value() {
    let output = run_with_stdin(
        &["validate", "--if-not-found", "fallback", "-f", "-"],
        "{{CHECK_DEFAULTED}}",
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout is not UTF-8"),
        "OK: all template variables resolved\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn check_rejects_a_missing_template_file() {
    let missing_path = unique_missing_path();
    assert!(!missing_path.exists());

    let output = Command::new(env!("CARGO_BIN_EXE_apply-env"))
        .args(["check", "--file"])
        .arg(&missing_path)
        .output()
        .expect("failed to start apply-env");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr is not UTF-8");
    assert!(stderr.starts_with("ERROR: failed to check template: "));
    assert!(stderr.contains("No such file or directory"));
}
