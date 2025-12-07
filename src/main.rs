use std::process;

use apply_env::{TemplateConfig, run_from_stdio};
use clap::Parser;

/// Apply environment variables to templates (Rust port of apply-env).
#[derive(Parser, Debug)]
#[command(
    name = "apply-env",
    author,
    version,
    about = "Apply environment variables to templates (Rust port of apply-env)",
    long_about = None
)]
struct Cli {
    /// Specifies template file name
    #[arg(short = 'f', long = "file", value_name = "NAME")]
    file: Option<String>,

    /// Rewrite input file!
    #[arg(short = 'w', long = "rewrite")]
    rewrite: bool,

    /// Make Helm template compatible!
    #[arg(short = 'm', long = "helm-only")]
    helm_only: bool,

    /// Escape special string chars (needed for JSON)
    #[arg(short = 'e', long = "escape")]
    escape: bool,

    /// Apply this 'if-not-found' value for 'env' that was not exists
    #[arg(short = 'n', long = "if-not-found", value_name = "VALUE")]
    if_not_found: Option<String>,

    /// Debug?
    #[arg(short = 'd', long = "debug")]
    debug: bool,
}

fn main() {
    // clap se postará o -h/--help a -v/--version
    let cli = Cli::parse();

    let cfg = TemplateConfig {
        file_name: cli.file,
        rewrite: cli.rewrite,
        helm_only: cli.helm_only,
        escape: cli.escape,
        default: cli.if_not_found,
        debug: cli.debug,
    };

    if let Err(err) = run_from_stdio(cfg) {
        eprintln!("ERROR: {err}");
        process::exit(1);
    }
}

#[cfg(test)]
mod cli_tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn cli_parses_basic_flags() {
        let cli = Cli::parse_from([
            "apply-env",
            "-w",
            "-m",
            "-e",
            "-d",
            "-f",
            "file.txt",
            "-n",
            "DEFAULT",
        ]);

        assert!(cli.rewrite);
        assert!(cli.helm_only);
        assert!(cli.escape);
        assert!(cli.debug);
        assert_eq!(cli.file.as_deref(), Some("file.txt"));
        assert_eq!(cli.if_not_found.as_deref(), Some("DEFAULT"));
    }
}
