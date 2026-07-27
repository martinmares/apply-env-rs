use std::collections::{BTreeMap, HashMap};
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
/// Konfigurace ekvivalentní OptionParseru v Crystal kódu.
#[derive(Debug, Clone)]
pub struct TemplateConfig {
    pub file_name: Option<String>,
    pub rewrite: bool,
    pub helm_only: bool,
    pub escape: bool,
    pub default: Option<String>,
    pub debug: bool,
    /// Volitelná mapa proměnných prostředí - pokud je Some,
    /// používá se místo skutečného process ENV.
    pub env_vars: Option<HashMap<String, String>>,
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
            env_vars: None,
        }
    }
}

/// Výsledek renderování včetně placeholderů, které nebylo možné rozřešit.
///
/// `unresolved` obsahuje unikátní názvy proměnných a počet jejich výskytů.
/// `BTreeMap` zajišťuje stabilní pořadí vhodné pro CLI reporty a CI logy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutcome {
    pub rendered: String,
    pub unresolved: BTreeMap<String, usize>,
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

pub fn render_template_with_lookup_report<F>(
    template: &str,
    cfg: &TemplateConfig,
    mut lookup: F,
) -> RenderOutcome
where
    F: FnMut(&str) -> Option<String>,
{
    if template.is_empty() {
        return RenderOutcome {
            rendered: String::new(),
            unresolved: BTreeMap::new(),
        };
    }

    let re = Regex::new(r"(?ix)(\{\{\s*)(\w+)(\s*\}\})")
        .expect("invalid regex for render_template_with_lookup_report");

    let mut result = String::with_capacity(template.len());
    let mut unresolved = BTreeMap::new();
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
                        *unresolved.entry(name.to_string()).or_insert(0) += 1;
                        orig.to_string()
                    }
                }
            }
        };

        result.push_str(&replacement);
        last_end = end;
    }

    result.push_str(&template[last_end..]);

    RenderOutcome {
        rendered: result,
        unresolved,
    }
}

pub fn render_template_with_lookup<F>(template: &str, cfg: &TemplateConfig, lookup: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    render_template_with_lookup_report(template, cfg, lookup).rendered
}

/// Čistá funkce pro templating - obdoba `Template#render`
/// bez I/O (užitečné pro testy a embedování).
pub fn render_template_str(template: &str, cfg: &TemplateConfig) -> String {
    render_template_str_report(template, cfg).rendered
}

/// Stejné renderování jako `render_template_str`, navíc s reportem
/// nerozřešených proměnných.
pub fn render_template_str_report(template: &str, cfg: &TemplateConfig) -> RenderOutcome {
    if let Some(ref map) = cfg.env_vars {
        // Použijeme pouze hodnoty z mapy (např. načtené z .env souboru)
        render_template_with_lookup_report(template, cfg, |name| map.get(name).cloned())
    } else {
        // Výchozí chování: čteme z process ENV
        render_template_with_lookup_report(template, cfg, |name| env::var(name).ok())
    }
}

/// Port `Template#load_content` + `Template#rewrite?`
/// používané `main` funkcí - čte ze stdin/souboru a
/// buď vypíše, nebo přepíše soubor.
pub fn run_from_stdio(cfg: TemplateConfig) -> io::Result<()> {
    let content = load_content(&cfg)?;
    let rendered = render_template_str(&content, &cfg);
    rewrite_or_print(&rendered, &cfg)
}

/// Provede stejnou substituci jako běžný běh, ale nic nevypisuje ani
/// nepřepisuje. Na rozdíl od historického renderování je neexistující
/// vstupní soubor chyba, aby kontrola nemohla skončit falešným úspěchem.
pub fn check_from_stdio(cfg: &TemplateConfig) -> io::Result<RenderOutcome> {
    let content = load_content_strict(cfg)?;
    Ok(render_template_str_report(&content, cfg))
}

fn load_content(cfg: &TemplateConfig) -> io::Result<String> {
    if let Some(ref file_name) = cfg.file_name {
        if file_name == "-" {
            // Unix idiom: "-" znamená STDIN
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        } else if Path::new(file_name).exists() {
            std::fs::read_to_string(file_name)
        } else {
            Ok(String::new())
        }
    } else {
        // knihovní použití -> stdin
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    }
}

fn load_content_strict(cfg: &TemplateConfig) -> io::Result<String> {
    if let Some(ref file_name) = cfg.file_name {
        if file_name == "-" {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        } else {
            std::fs::read_to_string(file_name)
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

    #[test]
    fn uses_env_vars_map_instead_of_process_env() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert("FOO".to_string(), "from_file".to_string());

        let cfg = TemplateConfig {
            env_vars: Some(map),
            ..Default::default()
        };

        let rendered = render_template_str("x {{FOO}} y", &cfg);
        assert_eq!(rendered, "x from_file y");
    }

    #[test]
    fn report_collects_unresolved_variables_and_occurrence_counts() {
        let cfg = TemplateConfig::default();

        let outcome = render_template_with_lookup_report(
            "{{ZED}} {{ALPHA}} {{ZED}} {{PRESENT}}",
            &cfg,
            |name| match name {
                "PRESENT" => Some("resolved".to_string()),
                _ => None,
            },
        );

        assert_eq!(outcome.rendered, "{{ZED}} {{ALPHA}} {{ZED}} resolved");
        assert_eq!(
            outcome.unresolved,
            BTreeMap::from([("ALPHA".to_string(), 1), ("ZED".to_string(), 2)])
        );
    }

    #[test]
    fn report_treats_default_value_as_resolved() {
        let cfg = TemplateConfig {
            default: Some("fallback".to_string()),
            ..Default::default()
        };

        let outcome = render_template_with_lookup_report("{{MISSING}}", &cfg, |_name| None);

        assert_eq!(outcome.rendered, "fallback");
        assert!(outcome.unresolved.is_empty());
    }

    #[test]
    fn report_does_not_rescan_placeholders_introduced_by_values() {
        let cfg = TemplateConfig::default();

        let outcome = render_template_with_lookup_report("{{OUTER}}", &cfg, |_name| {
            Some("{{INNER}}".to_string())
        });

        assert_eq!(outcome.rendered, "{{INNER}}");
        assert!(outcome.unresolved.is_empty());
    }

    #[test]
    fn report_treats_an_empty_existing_value_as_resolved() {
        let cfg = TemplateConfig::default();

        let outcome = render_template_with_lookup_report("before{{EMPTY}}after", &cfg, |_name| {
            Some(String::new())
        });

        assert_eq!(outcome.rendered, "beforeafter");
        assert!(outcome.unresolved.is_empty());
    }
}
