use std::process::{Command, Stdio};

fn examples(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_apply-env"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("failed to start apply-env")
}

#[test]
fn guide_and_alias_work_without_reading_template_or_environment_files() {
    let output = examples(&[
        "examples",
        "--file",
        "/nonexistent/apply-env-template",
        "--env-file",
        "/nonexistent/apply-env-vars",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let guide = String::from_utf8(output.stdout).unwrap();
    assert!(guide.starts_with("APPLY-ENV EXAMPLES\n"));
    assert!(guide.contains("FILE · template.yaml (YAML)"));
    assert!(guide.contains("SHELL"));
    assert!(!guide.contains('\x1b'));
    let alias = examples(&["example", "--color", "never"]);
    assert!(alias.status.success());
    assert_eq!(alias.stdout, guide.as_bytes());
}

#[test]
fn explicit_color_preserves_the_plain_guide() {
    let plain = examples(&["examples", "--color", "never"]);
    let colored = examples(&["examples", "--color", "always"]);
    assert!(plain.status.success());
    assert!(colored.status.success());
    let colored = String::from_utf8(colored.stdout).unwrap();
    assert!(colored.contains("\x1b[1;36m"));
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    assert_eq!(ansi.replace_all(&colored, "").as_bytes(), plain.stdout);
}

#[test]
fn examples_rejects_extra_arguments_and_invalid_color_modes() {
    for args in [
        &["examples", "unexpected"][..],
        &["examples", "--color", "invalid"][..],
    ] {
        let output = examples(args);
        assert_eq!(output.status.code(), Some(2));
    }
}
