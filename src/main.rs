use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;

use apply_env::{TemplateConfig, run_from_stdio};
use clap::{CommandFactory, Parser};

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

    /// Load variables from a .env-style file (instead of process ENV)
    #[arg(short = 'E', long = "env-file", value_name = "FILE")]
    env_file: Option<String>,
}

fn normalize_args(raw_args: Vec<String>) -> Vec<String> {
    // Bez argumentů nic neupravujeme (řeší se v main)
    if raw_args.len() <= 1 {
        return raw_args;
    }

    // Když uživatel explicitně použil -f / --file, nic nepřemapováváme.
    let has_file_flag = raw_args
        .iter()
        .skip(1)
        .any(|arg| arg == "-f" || arg == "--file" || arg.starts_with("--file="));

    if has_file_flag {
        return raw_args;
    }

    // Když není -f/--file a poslední argument je "-", interpretujeme to
    // jako alias pro "-f -".
    if let Some(last) = raw_args.last() {
        if last == "-" {
            let last_index = raw_args.len() - 1;
            let mut v = Vec::with_capacity(raw_args.len() + 1);
            for (i, arg) in raw_args.into_iter().enumerate() {
                if i == last_index {
                    // před původní "-" vložíme "-f"
                    v.push("-f".to_string());
                    v.push(arg);
                } else {
                    v.push(arg);
                }
            }
            v
        } else {
            raw_args
        }
    } else {
        raw_args
    }
}

fn main() {
    let raw_args: Vec<String> = env::args().collect();

    // 1) Bez argumentů -> vytiskni help (stejné jako -h), exit 0
    if raw_args.len() == 1 {
        let mut cmd = Cli::command();
        cmd.print_help().expect("Failed to print help");
        println!();
        return;
    }

    // 2) Přemapujeme alias "apply-env -" na "apply-env -f -"
    let args_for_clap = normalize_args(raw_args);

    // 3) Necháme clap zparsovat argumenty (včetně -h / -v)
    let cli = Cli::parse_from(args_for_clap);

    // 4) Musí být nějaký zdroj vstupu, tj. -f NAME nebo alias "-"
    if cli.file.is_none() {
        let mut cmd = Cli::command();
        cmd.print_help().expect("Failed to print help");
        println!();
        process::exit(1);
    }

    // 5) env-file (pokud je)
    let env_vars = match cli.env_file {
        Some(path) => match load_env_file(&path) {
            Ok(map) => Some(map),
            Err(err) => {
                eprintln!("ERROR: failed to read env file {path}: {err}");
                process::exit(1);
            }
        },
        None => None,
    };

    // 6) Config pro core logiku
    let cfg = TemplateConfig {
        file_name: cli.file,
        rewrite: cli.rewrite,
        helm_only: cli.helm_only,
        escape: cli.escape,
        default: cli.if_not_found,
        debug: cli.debug,
        env_vars,
    };

    // 7) Templating (stdin / soubor podle file_name)
    if let Err(err) = run_from_stdio(cfg) {
        eprintln!("ERROR: {err}");
        process::exit(1);
    }
}

/// Jednoduchý loader "dot-env" / properties souboru:
///
/// - ignoruje prázdné řádky a ty začínající '#'
/// - podporuje volitelné "export " na začátku (jako shell)
/// - očekává KEY=VALUE
/// - VALUE může být v uvozovkách "..." nebo '...'
fn load_env_file(path: &str) -> std::io::Result<HashMap<String, String>> {
    let text = fs::read_to_string(path)?;
    let mut map = HashMap::new();

    for (line_no, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Volitelně stripneme "export "
        let without_export = trimmed.strip_prefix("export ").unwrap_or(trimmed);

        let mut parts = without_export.splitn(2, '=');
        let key = match parts.next() {
            Some(k) if !k.trim().is_empty() => k.trim(),
            _ => {
                // Nekorektní řádek, prostě přeskočíme
                eprintln!(
                    "WARNING: ignoring malformed line {} in env file {}",
                    line_no + 1,
                    path
                );
                continue;
            }
        };

        let value_raw = parts.next().unwrap_or("").trim();

        // Podpora jednoduchých a dvojitých uvozovek kolem hodnoty
        let value_unquoted = if (value_raw.starts_with('"') && value_raw.ends_with('"'))
            || (value_raw.starts_with('\'') && value_raw.ends_with('\''))
        {
            &value_raw[1..value_raw.len() - 1]
        } else {
            value_raw
        };

        map.insert(key.to_string(), value_unquoted.to_string());
    }

    Ok(map)
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn dash_alias_maps_to_file_dash() {
        // Simuluje: apply-env -
        let raw_args = vec!["apply-env".to_string(), "-".to_string()];

        let normalized = normalize_args(raw_args);
        assert_eq!(normalized, vec!["apply-env", "-f", "-"]);

        let cli = Cli::parse_from(normalized);
        assert_eq!(cli.file.as_deref(), Some("-"));
    }

    #[test]
    fn explicit_file_dash_is_preserved() {
        // Simuluje: apply-env -f -
        let args = vec!["apply-env".to_string(), "-f".to_string(), "-".to_string()];

        let normalized = normalize_args(args.clone());
        // tady by se nic přemapovat nemělo
        assert_eq!(normalized, args);

        let cli = Cli::parse_from(normalized);
        assert_eq!(cli.file.as_deref(), Some("-"));
    }

    #[test]
    fn file_argument_is_propagated_normally() {
        // Simuluje: apply-env -f template.yaml
        let args = vec![
            "apply-env".to_string(),
            "-f".to_string(),
            "template.yaml".to_string(),
        ];

        let normalized = normalize_args(args.clone());
        assert_eq!(normalized, args);

        let cli = Cli::parse_from(normalized);
        assert_eq!(cli.file.as_deref(), Some("template.yaml"));
    }
}
