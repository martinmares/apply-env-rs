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

fn main() {
    let raw_args: Vec<String> = env::args().collect();

    // 1) Bez argumentů -> vytiskni help (stejné jako -h)
    if raw_args.len() == 1 {
        let mut cmd = Cli::command();
        cmd.print_help().expect("Failed to print help");
        println!();
        return;
    }

    // 2) Zjistíme, jestli uživatel explicitně chce číst ze stdin přes `--`
    let stdin_mode = raw_args.iter().skip(1).any(|a| a == "--");

    // 3) Odstraníme `--` z argumentů, než je předáme clap-u
    let filtered_args: Vec<String> = raw_args
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if i > 0 && s == "--" {
                None
            } else {
                Some(s.clone())
            }
        })
        .collect();

    // 4) Necháme clap zparsovat zbylé argumenty (včetně -h / -v)
    let cli = Cli::parse_from(filtered_args);

    // 5) Musí být buď soubor (-f), nebo explicitní stdin (`--`)
    if cli.file.is_none() && !stdin_mode {
        let mut cmd = Cli::command();
        cmd.print_help().expect("Failed to print help");
        println!();
        // neplatné použití -> vrátíme nenulový exit code
        process::exit(1);
    }

    // 6) Pokud je zadán env-file, načteme proměnné z něj
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

    // 7) Sestavíme TemplateConfig pro core logiku
    let cfg = TemplateConfig {
        file_name: cli.file,
        rewrite: cli.rewrite,
        helm_only: cli.helm_only,
        escape: cli.escape,
        default: cli.if_not_found,
        debug: cli.debug,
        env_vars,
    };

    // 8) Spustíme templating (čtení buď ze souboru nebo ze stdin
    //    podle toho, zda cfg.file_name je Some / None)
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
