use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "apply-env-output-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_apply-env"));
        command
            .current_dir(&self.0)
            .env("OUTPUT_TEST_VALUE", "world");
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn output_file_matches_stdout_and_preserves_template() {
    let fixture = Fixture::new();
    let template = "hello {{OUTPUT_TEST_VALUE}}";
    fs::write(fixture.0.join("input.yaml"), template).unwrap();
    let stdout = fixture
        .command()
        .args(["-f", "input.yaml"])
        .output()
        .unwrap();
    assert!(stdout.status.success());

    for flag in ["-o", "--output"] {
        fs::write(fixture.0.join("output.yaml"), "previous longer content").unwrap();
        let output = fixture
            .command()
            .args(["-f", "input.yaml", flag, "output.yaml"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stdout.is_empty());
        assert_eq!(
            fs::read(fixture.0.join("output.yaml")).unwrap(),
            stdout.stdout
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("input.yaml")).unwrap(),
            template
        );
    }
}

#[test]
fn stdin_can_be_rendered_to_a_new_file() {
    let fixture = Fixture::new();
    let mut child = fixture
        .command()
        .args(["-f", "-", "-o", "output.yaml"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{{OUTPUT_TEST_VALUE}}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(fixture.0.join("output.yaml")).unwrap(), b"world\n");
}

#[test]
fn empty_result_truncates_output_file() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("input.yaml"), "").unwrap();
    fs::write(fixture.0.join("output.yaml"), "old content").unwrap();
    let output = fixture
        .command()
        .args(["-f", "input.yaml", "-o", "output.yaml"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(fs::read(fixture.0.join("output.yaml")).unwrap().is_empty());
}

#[test]
fn missing_input_does_not_overwrite_output() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("output.yaml"), "keep me").unwrap();
    let output = fixture
        .command()
        .args(["-f", "missing.yaml", "-o", "output.yaml"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
    assert_eq!(fs::read(fixture.0.join("output.yaml")).unwrap(), b"keep me");
}

#[test]
fn output_write_error_is_reported() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("input.yaml"), "hello").unwrap();
    let output = fixture
        .command()
        .args(["-f", "input.yaml", "-o", "missing/output.yaml"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
    assert!(output.stdout.is_empty());
}

#[test]
fn incompatible_output_modes_are_rejected_without_writing() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("input.yaml"), "original").unwrap();
    for args in [
        vec!["-f", "input.yaml", "-o", "output.yaml", "-w"],
        vec!["-o", "output.yaml", "check", "-f", "input.yaml"],
        vec!["validate", "-f", "input.yaml", "-o", "output.yaml"],
    ] {
        let output = fixture.command().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!fixture.0.join("output.yaml").exists());
        assert_eq!(fs::read(fixture.0.join("input.yaml")).unwrap(), b"original");
    }
}
