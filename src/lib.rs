use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use colored::Colorize;
use regex::Regex;

/// Stejná verze jako v Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// Konfigurace ekvivalentní OptionParseru v Crystal kódu.
#[derive(Debug, Clone)]
pub struct TemplateConfig {
    pub file_name: Option<String>,
    pub rewrite: bool,
    pub helm_only: bool,
    pub escape: bool,
    pub default: Option<String>,
    pub debug: bool,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        TemplateConfig {
            file_name: None,
            rewrite: false,
            helm_only: false,
            escape: false,
            default: None,
            debug: false,
        }
    }
}

/// Port metody `escape_special_chars`
///
/// Crystal:
///   orig
///     # .gsub("/", "\\/")
///     .gsub("\\", "\\\\")
///     .gsub("\"", "\\\"")
///     .gsub("\n", "\\n")
///     .gsub("\r", "\\r")
///     .gsub("\b", "\\b")
///     .gsub("\f", "\\f")
///     .gsub("\t", "\\t")
fn escape_special_chars(orig: &str) -> String {
    // Pořadí zachováno
    let mut s = orig.replace('\\', "\\\\");
    s = s.replace('"', "\\\"");
    s = s.replace('\n', "\\n");
    s = s.replace('\r', "\\r");
    s = s.replace('\u{0008}', "\\b"); // \b
    s = s.replace('\u{000C}', "\\f"); // \f
    s = s.replace('\t', "\\t");
    s
}

fn is_helm_wrapped(content: &str, start: usize, end: usize) -> bool {
    // `content[start..end]` je vnitřní "{{FOO}}"
    // chceme detekovat obal: {{` {{FOO}} `}}
    if start < 3 || end + 3 > content.len() {
        return false;
    }

    &content[start - 3..start] == "{{`" && &content[end..end + 3] == "`}}"
}

pub fn render_template_with_lookup<F>(template: &str, cfg: &TemplateConfig, mut lookup: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    if template.is_empty() {
        return String::new();
    }

    let re = Regex::new(r"(?ix)(\{\{\s*)(\w+)(\s*\}\})")
        .expect("invalid regex for render_template_with_lookup");

    let mut result = String::with_capacity(template.len());
    let mut last_end = 0usize;

    for (i, caps) in re.captures_iter(template).enumerate() {
        let m = caps.get(0).unwrap();
        let start = m.start();
        let end = m.end();

        result.push_str(&template[last_end..start]);

        let orig = m.as_str();
        let name = caps.get(2).unwrap().as_str();

        let replacement = if cfg.helm_only {
            if is_helm_wrapped(template, start, end) {
                orig.to_string()
            } else {
                let val = format!("{{{{`{}`}}}}", orig);
                if cfg.debug {
                    println!(
                        "Found [{}], orig: \"{}\", apply with: \"{}\"",
                        i,
                        orig.yellow(),
                        val.green()
                    );
                }
                val
            }
        } else {
            match lookup(name) {
                Some(v) => {
                    let v2 = if cfg.escape {
                        escape_special_chars(&v)
                    } else {
                        v
                    };
                    if cfg.debug {
                        println!(
                            "Found [{}], orig: \"{}\", apply with: \"{}\"",
                            i,
                            orig.yellow(),
                            v2.green()
                        );
                    }
                    v2
                }
                None => {
                    if let Some(ref default) = cfg.default {
                        if cfg.debug {
                            println!(
                                "Found [{}], orig: \"{}\", apply with default: \"{}\"",
                                i,
                                orig.yellow(),
                                default.green()
                            );
                        }
                        default.clone()
                    } else {
                        if cfg.debug {
                            println!(
                                "Found [{}], orig: \"{}\", not found and no default, keeping as-is",
                                i,
                                orig.yellow()
                            );
                        }
                        orig.to_string()
                    }
                }
            }
        };

        result.push_str(&replacement);
        last_end = end;
    }

    result.push_str(&template[last_end..]);

    result
}

/// Čistá funkce pro templating - obdoba `Template#render`
/// bez I/O (užitečné pro testy a embedování).
pub fn render_template_str(template: &str, cfg: &TemplateConfig) -> String {
    render_template_with_lookup(template, cfg, |name| env::var(name).ok())
}

/// Port `Template#load_content` + `Template#rewrite?`
/// používané `main` funkcí - čte ze stdin/souboru a
/// buď vypíše, nebo přepíše soubor.
pub fn run_from_stdio(cfg: TemplateConfig) -> io::Result<()> {
    let content = load_content(&cfg)?;
    let rendered = render_template_str(&content, &cfg);
    rewrite_or_print(&rendered, &cfg)
}

fn load_content(cfg: &TemplateConfig) -> io::Result<String> {
    // Crystal default: čte ze STDIN, dokud není -f/--file
    if let Some(ref file_name) = cfg.file_name {
        if Path::new(file_name).exists() {
            std::fs::read_to_string(file_name)
        } else {
            // Crystal: File.read jen když File.exists?
            // jinak @content zůstane nil -> "".
            Ok(String::new())
        }
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    }
}

fn rewrite_or_print(content: &str, cfg: &TemplateConfig) -> io::Result<()> {
    if cfg.rewrite && !content.is_empty() {
        if let Some(ref file_name) = cfg.file_name {
            if cfg.debug {
                println!("Try rewrite content:");
            }
            let mut file = File::create(file_name)?;
            // TADY KLIDNĚ MŮŽE ZŮSTAT writeln! - při přepisu souboru
            // se chová stejně jako Crystal (puts).
            writeln!(file, "{}", content)?;
            if cfg.debug {
                println!(" => rewrited: {}", file_name.green());
            }
            Ok(())
        } else {
            // fallback - radši jen vypiš obsah bez dalšího \n
            print!("{}", content);
            Ok(())
        }
    } else {
        // PŮVODNĚ: println!("{}", content);
        // → změnit na:
        print!("{}", content);
        Ok(())
    }
}

//
// ---------- CLI parsing (port OptionParseru) ----------
//

#[derive(Debug)]
pub enum CliResult {
    Run(TemplateConfig),
    Help,
    Version,
    InvalidOption(String),
}

/// Ručně napsaný help, aby byl co nejvíc podobný výstupu OptionParseru v Crystal.
pub const HELP_TEXT: &str = "\
Usage: apply-env [arguments]
  -f NAME, --file=NAME            Specifies template file name
  -w, --rewrite                   Rewrite input file!
  -m, --helm-only                 Make HEML template compatible!
  -e, --escape                    Escape special string chars (need for JSON)
  -n VALUE, --if-not-found=VALUE  Apply this 'if-not-found' value for 'env' that was not exists
  -d, --debug                     Debug?
  -v, --version                   App version
  -h, --help                      Show this help
";

/// Port logiky OptionParseru - nepoužívám clap, aby chování bylo
/// co nejvíc pod kontrolou.
pub fn parse_args<I, S>(args: I) -> CliResult
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut cfg = TemplateConfig::default();
    let mut iter = args.into_iter().map(|s| s.into());

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return CliResult::Help,
            "-v" | "--version" => return CliResult::Version,
            "-w" | "--rewrite" => {
                cfg.rewrite = true;
            }
            "-m" | "--helm-only" => {
                cfg.helm_only = true;
            }
            "-e" | "--escape" => {
                cfg.escape = true;
            }
            "-d" | "--debug" => {
                cfg.debug = true;
            }
            "-f" => {
                if let Some(name) = iter.next() {
                    cfg.file_name = Some(name);
                } else {
                    // v Crystal by to hodilo MissingOption/InvalidOption
                    return CliResult::InvalidOption("-f".to_string());
                }
            }
            s if s.starts_with("--file=") => {
                let name = s.trim_start_matches("--file=").to_string();
                cfg.file_name = Some(name);
            }
            "-n" => {
                if let Some(val) = iter.next() {
                    cfg.default = Some(val);
                } else {
                    return CliResult::InvalidOption("-n".to_string());
                }
            }
            s if s.starts_with("--if-not-found=") => {
                let val = s.trim_start_matches("--if-not-found=").to_string();
                cfg.default = Some(val);
            }
            other => {
                // invalid_option v Crystal
                return CliResult::InvalidOption(other.to_string());
            }
        }
    }

    CliResult::Run(cfg)
}

pub fn print_version() {
    // "apply-env 1.3.0"
    println!("{} {} (rust)", PKG_NAME, VERSION);
}

//
// ---------- Jednoduché unit testy pro core logiku (víc než v původním Crystal repu :) ) ----------
//

#[cfg(test)]
mod tests {
    use super::*;

    fn render_with_fake_env<F>(template: &str, cfg: &TemplateConfig, f: F) -> String
    where
        F: FnMut(&str) -> Option<String>,
    {
        render_template_with_lookup(template, cfg, f)
    }

    #[test]
    fn replaces_env_without_escape() {
        let cfg = TemplateConfig {
            escape: false,
            ..Default::default()
        };

        let rendered = render_with_fake_env("hello {{FOO}} world", &cfg, |name| match name {
            "FOO" => Some("bar".to_string()),
            _ => None,
        });

        assert_eq!(rendered, "hello bar world");
    }

    #[test]
    fn leaves_placeholder_when_missing_and_no_default() {
        let cfg = TemplateConfig::default();

        let rendered = render_with_fake_env("x {{MISSING}} y", &cfg, |_name| None);

        assert_eq!(rendered, "x {{MISSING}} y");
    }

    #[test]
    fn uses_default_when_missing() {
        let cfg = TemplateConfig {
            default: Some("42".to_string()),
            ..Default::default()
        };

        let rendered = render_with_fake_env("x {{MISSING}} y", &cfg, |_name| None);

        assert_eq!(rendered, "x 42 y");
    }

    #[test]
    fn helm_only_wraps_placeholder() {
        let cfg = TemplateConfig {
            helm_only: true,
            ..Default::default()
        };

        let rendered = render_template_str("{{FOO}}", &cfg);
        assert_eq!(rendered, "{{`{{FOO}}`}}");
    }

    #[test]
    fn parse_args_basic_flags() {
        let args = ["-w", "-m", "-e", "-d", "-f", "file.txt", "-n", "DEFAULT"];
        match parse_args(args.iter().map(|s| s.to_string())) {
            CliResult::Run(cfg) => {
                assert!(cfg.rewrite);
                assert!(cfg.helm_only);
                assert!(cfg.escape);
                assert!(cfg.debug);
                assert_eq!(cfg.file_name.as_deref(), Some("file.txt"));
                assert_eq!(cfg.default.as_deref(), Some("DEFAULT"));
            }
            _ => panic!("expected Run(...)"),
        }
    }

    #[test]
    fn parse_args_help_and_version() {
        match parse_args(["-h"].iter().map(|s| s.to_string())) {
            CliResult::Help => {}
            _ => panic!("expected Help"),
        }
        match parse_args(["-v"].iter().map(|s| s.to_string())) {
            CliResult::Version => {}
            _ => panic!("expected Version"),
        }
    }

    #[test]
    fn parse_args_invalid_option() {
        match parse_args(["--no-such-flag"].iter().map(|s| s.to_string())) {
            CliResult::InvalidOption(flag) => assert_eq!(flag, "--no-such-flag"),
            _ => panic!("expected InvalidOption"),
        }
    }

    #[test]
    fn helm_only_does_not_double_wrap() {
        let cfg = TemplateConfig {
            helm_only: true,
            ..Default::default()
        };

        let rendered = render_template_str("hello: {{`{{FOO}}`}}", &cfg);
        assert_eq!(rendered, "hello: {{`{{FOO}}`}}");
    }

    #[test]
    fn helm_only_multiple_occurrences_in_one_string() {
        let cfg = TemplateConfig {
            helm_only: true,
            ..Default::default()
        };
        let rendered = render_template_str(r#"hello: "{{FOO}} -> {{FOO}} -> {{FOO}}""#, &cfg);
        assert_eq!(
            rendered,
            r#"hello: "{{`{{FOO}}`}} -> {{`{{FOO}}`}} -> {{`{{FOO}}`}}""#
        );
    }

    #[test]
    fn replaces_multiple_occurrences_in_one_string_no_helm() {
        let cfg = TemplateConfig {
            ..Default::default()
        };

        let rendered =
            render_with_fake_env(r#"hello: "{{FOO}} -> {{FOO}} -> {{FOO}}""#, &cfg, |name| {
                match name {
                    "FOO" => Some("hello".to_string()),
                    _ => None,
                }
            });

        assert_eq!(rendered, r#"hello: "hello -> hello -> hello""#);
    }

    #[test]
    fn escape_mode_escapes_special_chars() {
        let cfg = TemplateConfig {
            escape: true,
            ..Default::default()
        };

        let rendered = render_with_fake_env("{{FOO}}", &cfg, |name| match name {
            "FOO" => Some("a\"b\\c\n\r\t".to_string()),
            _ => None,
        });

        assert_eq!(rendered, "a\\\"b\\\\c\\n\\r\\t");
    }
}
