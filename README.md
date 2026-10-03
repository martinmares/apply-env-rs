# apply-env-rs

Rust port of the original [apply-env](https://github.com/martinmares/apply-env) utility: a tiny CLI tool that applies environment variables to text templates.

It’s intentionally small and predictable – essentially “just” replaces `{{VAR_NAME}}` placeholders with values from the environment. It’s handy for simple config templating (YAML, JSON, etc.), especially in container / CI environments.

---

## Features

- Replace `{{VAR_NAME}}` with values from the environment
- Optional JSON-style escaping for values (`-e / --escape`)
- Helm-compatible wrapping mode (`-m / --helm-only`)
  - Turns `{{FOO}}` into `{{`{{FOO}}`}}`
  - Does **not** double-wrap already wrapped expressions
- Default value for missing env variables (`-n / --if-not-found=VALUE`)
- Reads from **stdin** (via `-` or `-f -`) or from a **file** (`-f / --file`)
- Optional in-place file rewrite (`-w / --rewrite`)
- Optional `.env` / properties-style file as the **only** source of variables (`-E / --env-file`)
- Optional case-sensitive variable prefix selection (`--prefix`, repeatable)
- Dry-run validation with `check` (alias `validate`) that reports unresolved variables
- Debug output to see what’s being replaced (`-d / --debug`)
- No errors for missing template variables by default – placeholders are left as-is (unless a default is provided)

---

## Installation

### Build from source

You need a working Rust toolchain (via [rustup](https://rustup.rs/)).

```bash
git clone https://github.com/martinmares/apply-env-rs.git
cd apply-env-rs

# Build debug binary
cargo build

# Or optimized release binary
cargo build --release
```

The resulting binary will be at:

- `target/debug/apply-env` or  
- `target/release/apply-env`

You can also install it into your local Cargo bin directory from the cloned repo:

```bash
cargo install --path .
```

---

## Usage

Basic synopsis:

```bash
apply-env [OPTIONS]
apply-env check [OPTIONS]
```

Supported options:

```text
-f NAME, --file=NAME            Specifies template file name
-o FILE, --output=FILE          Write the rendered document to a file instead of stdout
-w, --rewrite                   Rewrite input file!
-m, --helm-only                 Make Helm template compatible!
-e, --escape                    Escape special string chars (needed for JSON)
-n VALUE, --if-not-found=VALUE  Use this value if env var was not found
--prefix PREFIX               Only process matching variable names (repeatable)
-d, --debug                     Debug mode (verbose output)
-E FILE, --env-file=FILE        Load variables from a .env-style file instead of process ENV
-v, --version                   Show version
-h, --help                      Show this help
```

Supported commands:

```text
check                           Check that every template variable can be resolved
validate                        Alias for check
```

### Input / output

- **No arguments** (`apply-env`):
  - Prints the help text (same as `-h`) and exits.
- If `-f/--file` **is** specified:
  - Without `-w` or `-o`: reads from the file, writes processed content to stdout.
  - With `-o FILE`: writes processed content to the specified file.
  - With `-w`: reads from the file and rewrites the file in-place.
- If you want to read from **stdin**, you can use either `-` (alias for `-f -`) or explicit `-f -`:

  ```bash
  echo 'Hello {{FOO}}' | FOO=world apply-env -
  # or explicitly
  echo 'Hello {{FOO}}' | FOO=world apply-env -f -
  ```

If a file path is given but the file does not exist, it is treated as empty.

To write a file without shell redirection, use `-o / --output`:

```bash
apply-env -f config.pipeline.template.yaml -o config.pipeline.yaml
```

The output file is created or overwritten with the exact rendered content,
without an extra newline. Input from `-f -` is also supported. `--output` cannot
be combined with `--rewrite` or `check` / `validate`. With `--output`, an unreadable
or missing input file is an error; the output is only opened after the input has
been read and rendered. Missing output directories are not created automatically.

### Checking templates

The `check` command performs the same variable lookup and substitution as a
normal run, but it does not print or rewrite the rendered document. Instead, it
reports variables that could not be resolved:

```bash
apply-env check -f template.yaml
apply-env validate -E production.env -f template.yaml
echo '{{FOO}} {{BAR}}' | apply-env check -f -
```

The shorter `apply-env check -` form remains supported as an alias for
`apply-env check -f -`.

Example failure:

```text
ERROR: unresolved template variables:
  API_TOKEN (1 occurrence)
  DATABASE_URL (2 occurrences)
```

Variable names are sorted and repeated placeholders are reported once with an
occurrence count. An existing variable with an empty value is considered
resolved. When `--if-not-found` is used, its fallback also counts as a resolved
value.

The command exits with status `0` when every variable is resolved and `1` when
variables remain unresolved or the input cannot be read. Invalid command-line
arguments use the non-zero status produced by `clap`.

`check` accepts `--file`, `--env-file`, `--if-not-found`, and `--prefix`. It does not accept
`--rewrite`, `--helm-only`, `--escape`, `--debug`, or `--output`. Unlike the normal rendering
mode, checking a file that does not exist is an error.

---

## Selecting variables by prefix

Use `--prefix` when a document also contains placeholders owned by another tool.
Matching is case-sensitive and uses the full variable name: `--prefix KRF_`
replaces `{{KRF_ROOT}}` by looking up `KRF_ROOT`, without stripping the prefix.
Repeat the option to select multiple prefixes (any matching prefix is accepted).
Without `--prefix`, existing behavior is unchanged: all variables are processed.
An empty CLI prefix is rejected.

Excluded placeholders stay exactly as written, even if their variables exist.
They are not looked up, reported as unresolved, given a fallback or Helm-wrapped.
Escaping and defaults still apply to selected variables normally. The same filter
is used for rendering, `check`/`validate`, process environment and `--env-file`.

For example, a KRF pipeline template can contain:

```yaml
workdir:
  root: "{{CI_PROJECT_DIR}}/.krf-work"
# KRF resolves these expressions later:
tag_template: "dev/{{ release_name }}.r{{ revision }}"
```

Check and render using the same prefix selection:

```sh
apply-env check --prefix KRF_ --prefix CI_ -f config.pipeline.template.yaml
apply-env --prefix KRF_ --prefix CI_ -f config.pipeline.template.yaml -o config.pipeline.yaml
```

The KRF placeholders remain intact; missing selected variables cause `check` to
exit with status 1. Configure Git authentication separately in the CI environment;
repository URLs in the template do not need to contain access tokens.

---

## Template syntax

`apply-env` looks for patterns of the form:

```text
{{ VAR_NAME }}
{{VAR_NAME}}
{{   VAR_NAME   }}
```

Where `VAR_NAME` matches `\w+` (letters, digits, underscore).

For each match:

- In **normal mode**:
  - If the variable is present in the active environment (see “Variable sources” below), it is substituted (optionally escaped if `-e` is on).
  - If the variable is **not** found:
    - If `-n / --if-not-found=VALUE` is provided, `VALUE` is substituted.
    - Otherwise, the placeholder is left as-is.

- In **Helm mode** (`-m`), see below.

---

## Variable sources

There are two mutually exclusive ways to provide values:

1. **Process environment (default)**  
   This is the usual `$FOO=bar apply-env ...` behavior.

2. **`.env` / properties file** via `-E / --env-file`  
   When `--env-file` is used, values are taken **only** from that file, and the process environment is ignored.

   The file format is intentionally simple:

   - blank lines are ignored,
   - lines starting with `#` are comments,
   - optional `export ` prefix is allowed (and ignored),
   - `KEY=VALUE` pairs,
   - `VALUE` may be wrapped in single or double quotes.

   Example `.env`:

   ```env
   # comment
   FOO=hello
   export BAR="world"
   ```

   Example usage:

   ```bash
   apply-env -E .env -f template.yaml
   ```

---

## Examples

### 1. Simple substitution from stdin (process ENV)

```bash
echo 'Hello {{FOO}}' | FOO=world ./target/release/apply-env -
```

Output:

```text
Hello world
```

### 2. Using a template file (process ENV)

`template.yaml`:

```yaml
hello: "{{FOO}} -> {{FOO}} -> {{FOO}}"
```

Run:

```bash
FOO=hello ./target/release/apply-env -f template.yaml
```

Output:

```yaml
hello: "hello -> hello -> hello"
```

### 3. Using an env-file instead of process ENV

`.env`:

```env
FOO=from_file
```

`template.yaml`:

```yaml
hello: "{{FOO}}"
```

Run:

```bash
./target/release/apply-env -E .env -f template.yaml
```

Output:

```yaml
hello: "from_file"
```

### 4. In-place rewrite

```bash
FOO=world ./target/release/apply-env -f template.yaml -w
```

This will overwrite `template.yaml` with the processed content.

### 5. Reading from stdin with env-file

```bash
echo 'Hello {{FOO}}' | ./target/release/apply-env -E .env -
```

---

## JSON escaping mode

When you know the output will be interpreted as JSON, it’s sometimes useful to escape special characters in values.

With `-e / --escape`, the following characters are escaped:

- `\` → `\\`
- `"` → `\"`
- newline `\n` → `\\n`
- carriage return `\r` → `\\r`
- backspace `\b` → `\\b`
- formfeed `\f` → `\\f`
- tab `\t` → `\\t`

Example:

```bash
FOO='a"b\c
' ./target/release/apply-env -e -f template.json
```

If `template.json` contains:

```json
{ "value": "{{FOO}}" }
```

The output will be something like:

```json
{ "value": "a\"b\\c\n" }
```

(plus the other escaped control characters if present).

---

## Helm-compatible mode

Helm templates often want to treat certain `{{ ... }}` segments literally. In `--helm-only` mode, `apply-env` doesn’t read from the environment at all; instead, it wraps raw `{{VAR}}` instances so that Helm can evaluate them later.

For each placeholder `{{FOO}}`, Helm mode wraps it as:

```text
{{`{{FOO}}`}}
```

So this:

```yaml
hello: "{{FOO}} -> {{FOO}} -> {{FOO}}"
```

becomes:

```yaml
hello: "{{`{{FOO}}`}} -> {{`{{FOO}}`}} -> {{`{{FOO}}`}}"
```

Important details:

- **No double wrapping**: if the template already contains a Helm-wrapped expression like:

  ```yaml
  hello: "{{`{{FOO}}`}}"
  ```

  it is left unchanged when you run with `-m`.

- Helm mode ignores `-n / --if-not-found` and does not read real env values – it’s purely a transformation of template syntax.

Usage:

```bash
./target/release/apply-env -m -f template.yaml
```

or (from stdin):

```bash
echo '{{FOO}}' | ./target/release/apply-env -m -
```

---

## Default value for missing variables

If you want a consistent fallback for missing vars, you can use `-n` / `--if-not-found`:

```bash
# template.yaml:  message: "Hello {{NAME}}!"
./target/release/apply-env -f template.yaml -n "anonymous"
```

If the variable is not set in the active environment (process ENV or `--env-file`), you get:

```yaml
message: "Hello anonymous!"
```

Without `-n`, the placeholder would stay as `{{NAME}}`.

---

## Debug mode

The `-d / --debug` flag prints extra information about what was found and how it is being replaced, e.g.:

```bash
FOO=world ./target/release/apply-env -d -f template.yaml
```

Typical debug lines look like:

```text
Found [0], orig: "{{FOO}}", apply with: "world"
```

This is useful when you’re troubleshooting why a particular placeholder isn’t being substituted or how escaping/Helm mode is behaving.

---

## Behaviour notes

- Only placeholders matching `{{\s*\w+\s*}}` are processed.
- Unknown flags are handled by `clap` and result in an error message + non-zero exit code.
- `-v / --version` prints the package name and version as defined by `Cargo.toml`.

---

## Relationship to the Crystal version

This project is a Rust rewrite of the original Crystal [apply-env](https://github.com/martinmares/apply-env) utility. The goal is to keep runtime behaviour as close as possible to the Crystal implementation, while being easier to integrate in Rust-based ecosystems and tooling.

---

## License

MIT – see [`LICENSE`](./LICENSE).

## Commercial Support

This software is provided under the MIT License and comes without
warranty or free community support. The MIT License permits commercial use,
modification, and redistribution; purchasing support is not required to use it.

Commercial support is available from **[DataLite](https://datalite.cz)**, including:

- technical assistance,
- verified releases,
- bug fixes,
- updates,
- deployment assistance,
- troubleshooting,
- long-term maintenance.

Contact [DataLite](https://datalite.cz) for commercial support options.

Public availability does not imply a commitment to respond to questions, fix
reported bugs, implement feature requests, or provide regular updates. External
pull requests may be accepted at the maintainer's discretion, but review,
response, and acceptance are not guaranteed. Upstream write and merge access
is reserved for the owner and explicitly authorized maintainers.
