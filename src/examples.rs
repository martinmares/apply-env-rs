use clap::ValueEnum;
use std::io::{self, IsTerminal, Write};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

pub fn use_color(mode: ColorMode, stdout: &io::Stdout) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            stdout.is_terminal()
                && std::env::var_os("NO_COLOR").is_none()
                && std::env::var("TERM").as_deref() != Ok("dumb")
        }
    }
}

struct FileExample {
    name: &'static str,
    content: &'static str,
}

struct Example {
    title: &'static str,
    description: &'static str,
    files: &'static [FileExample],
    shell: &'static str,
}

struct Section {
    title: &'static str,
    examples: &'static [Example],
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Start here: stdin and files",
        examples: &[
            Example {
                title: "Render a template from stdin",
                description: "The trailing - is shorthand for -f -. Output goes to stdout.",
                files: &[],
                shell: "printf 'Hello {{NAME}}!\\n' | NAME=world apply-env -",
            },
            Example {
                title: "Render a YAML file",
                description: "Without -o or -w, the template is preserved and output goes to stdout.",
                files: &[FileExample {
                    name: "template.yaml (YAML)",
                    content: "app: \"{{APP_NAME}}\"\nport: {{APP_PORT}}",
                }],
                shell: "APP_NAME=demo APP_PORT=8080 apply-env -f template.yaml",
            },
        ],
    },
    Section {
        title: "Output and environment files",
        examples: &[
            Example {
                title: "Load variables from a .env file and save the result",
                description: "-E uses only this file, without falling back to process ENV. -o creates or overwrites the output file; its directory must exist.",
                files: &[
                    FileExample {
                        name: "production.env (DOT-ENV)",
                        content: "APP_NAME=demo\nAPP_PORT=8080",
                    },
                    FileExample {
                        name: "template.yaml (YAML)",
                        content: "app: \"{{APP_NAME}}\"\nport: {{APP_PORT}}",
                    },
                ],
                shell: "apply-env -E production.env -f template.yaml -o config.yaml",
            },
            Example {
                title: "Rewrite a file in place",
                description: "-w overwrites the input template. It cannot be combined with -o.",
                files: &[FileExample {
                    name: "config.yaml (YAML)",
                    content: "app: \"{{APP_NAME}}\"",
                }],
                shell: "APP_NAME=demo apply-env -f config.yaml -w",
            },
        ],
    },
    Section {
        title: "Missing values and prefix selection",
        examples: &[
            Example {
                title: "Supply a fallback for missing variables",
                description: "Without -n, unresolved placeholders stay unchanged. Here -E selects an empty environment file.",
                files: &[FileExample {
                    name: "empty.env (DOT-ENV)",
                    content: "# No variables defined",
                }],
                shell: "printf 'Hello {{NAME}}!\\n' | apply-env -E empty.env -n anonymous -",
            },
            Example {
                title: "Replace only selected variable prefixes",
                description: "Prefixes are case-sensitive and repeatable. Other placeholders stay unchanged, even with -n or check.",
                files: &[],
                shell: "printf '{{APP_NAME}} {{CI_JOB}} {{ release_name }}\\n' | \\\n  APP_NAME=demo CI_JOB=build apply-env --prefix APP_ --prefix CI_ -",
            },
        ],
    },
    Section {
        title: "Check before rendering",
        examples: &[
            Example {
                title: "Validate a template without writing rendered output",
                description: "check (alias validate) prints OK or reports unresolved variables and exits with status 1. Rendering flags -o, -w, -m, -e and -d are not supported.",
                files: &[
                    FileExample {
                        name: "production.env (DOT-ENV)",
                        content: "APP_NAME=demo\nAPP_PORT=8080",
                    },
                    FileExample {
                        name: "template.yaml (YAML)",
                        content: "app: \"{{APP_NAME}}\"\nport: {{APP_PORT}}",
                    },
                ],
                shell: "apply-env check -E production.env -f template.yaml\napply-env validate -E production.env -f template.yaml",
            },
            Example {
                title: "Check stdin in a pipeline",
                description: "An empty value counts as resolved. Missing variables are reported once with occurrence counts.",
                files: &[],
                shell: "printf '{{APP_NAME}} {{APP_NAME}}\\n' | APP_NAME=demo apply-env check -",
            },
        ],
    },
    Section {
        title: "JSON escaping and Helm templates",
        examples: &[
            Example {
                title: "Escape values inside JSON strings",
                description: "-e escapes quotes, backslashes and control characters in substituted values. Keep placeholders inside JSON strings.",
                files: &[FileExample {
                    name: "template.json (JSON)",
                    content: r#"{"message": "{{MESSAGE}}"}"#,
                }],
                shell: "MESSAGE='Hello \"world\"' apply-env -e -f template.json",
            },
            Example {
                title: "Preserve placeholders through Helm rendering",
                description: "-m wraps placeholders for Helm instead of substituting ENV values. Already wrapped placeholders are preserved.",
                files: &[],
                shell: "printf '{{APP_NAME}}\\n' | apply-env -m -",
            },
        ],
    },
    Section {
        title: "Help and diagnostics",
        examples: &[Example {
            title: "Inspect replacements and available options",
            description: "-d adds replacement diagnostics to stdout; use it when inspecting output interactively.",
            files: &[],
            shell: "printf '{{APP_NAME}}\\n' | APP_NAME=demo apply-env -d -\napply-env --help\napply-env check --help\napply-env examples --color never",
        }],
    },
];

fn paint(text: &str, style: &str, color: bool) -> String {
    if color {
        format!("\x1b[{style}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

pub fn write_examples(writer: &mut impl Write, color: bool) -> io::Result<()> {
    writeln!(writer, "{}", paint("APPLY-ENV EXAMPLES", "1;36", color))?;
    writeln!(
        writer,
        "Commands are marked SHELL; input files are marked FILE."
    )?;
    for section in SECTIONS {
        writeln!(
            writer,
            "\n{}",
            paint(&format!("## {}", section.title), "1;36", color)
        )?;
        for example in section.examples {
            writeln!(
                writer,
                "\n  {}",
                paint(&format!("› {}", example.title), "1;33", color)
            )?;
            writeln!(writer, "    {}", example.description)?;
            for file in example.files {
                writeln!(
                    writer,
                    "\n    {}",
                    paint(&format!("FILE · {}", file.name), "1;34", color)
                )?;
                for line in file.content.lines() {
                    writeln!(writer, "      {}", paint(line, "34", color))?;
                }
            }
            writeln!(writer, "\n    {}", paint("SHELL", "1;32", color))?;
            for line in example.shell.lines() {
                writeln!(writer, "      {}", paint(line, "32", color))?;
            }
        }
    }
    writeln!(
        writer,
        "\nRun 'apply-env --help' or 'apply-env <command> --help' for all flags. Full reference: README.md"
    )
}
