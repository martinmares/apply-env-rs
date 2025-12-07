# apply-env-rs

Rust port of the original [apply-env](https://github.com/martinmares/apply-env) utility: a tiny CLI tool that applies environment variables to text templates.

It’s intentionally small and predictable – essentially “just” replaces `{{VAR_NAME}}` placeholders with values from the environment. It’s handy for simple config templating (YAML, JSON, etc.), especially in container / CI environments.

---

## Features

- Replace `{{VAR_NAME}}` with values from the environment (`$VAR_NAME`)
- Optional JSON-style escaping for values (`-e / --escape`)
- Helm-compatible wrapping mode (`-m / --helm-only`)
  - Turns `{{FOO}}` into `{{`{{FOO}}`}}`
  - Does **not** double-wrap already wrapped expressions
- Default value for missing env variables (`-n / --if-not-found=VALUE`)
- Reads from **stdin** or from a **file** (`-f / --file`)
- Optional in-place file rewrite (`-w / --rewrite`)
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
apply-env [arguments]
```

Supported options:

```text
-f NAME, --file=NAME            Specifies template file name
-w, --rewrite                   Rewrite input file!
-m, --helm-only                 Make Helm template compatible!
-e, --escape                    Escape special string chars (needed for JSON)
-n VALUE, --if-not-found=VALUE  Use this value if env var was not found
-d, --debug                     Debug mode (verbose output)
-v, --version                   Show version
-h, --help                      Show this help
```

### Input / output

- If `-f/--file` is **not** specified, `apply-env` reads from **stdin** and writes to **stdout**.
- If `-f/--file` **is** specified:
  - Without `-w`: reads from the file, writes processed content to stdout.
  - With `-w`: reads from the file and rewrites the file in-place.

If a file path is given but the file does not exist, it is treated as empty.

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
  - If `$VAR_NAME` is set, it is substituted (optionally escaped if `-e` is on).
  - If `$VAR_NAME` is **not** set:
    - If `-n / --if-not-found=VALUE` is provided, `VALUE` is substituted.
    - Otherwise, the placeholder is left as-is.

- In **Helm mode** (`-m`), see below.

---

## Examples

### 1. Simple substitution from stdin

```bash
echo 'Hello {{FOO}}' | FOO=world ./target/release/apply-env
```

Output:

```text
Hello world
```

### 2. Using a template file

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

### 3. In-place rewrite

```bash
FOO=world ./target/release/apply-env -f template.yaml -w
```

This will overwrite `template.yaml` with the processed content.

---

## JSON escaping mode

When you know the output will be interpreted as JSON, it’s sometimes useful to escape special characters in env values.

With `-e / --escape`, the following characters are escaped in env values:

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

The output will be:

```json
{ "value": "a\\\"b\\\\c\\n" }
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

---

## Default value for missing variables

If you want a consistent fallback for missing vars, you can use `-n` / `--if-not-found`:

```bash
# template.yaml:  message: "Hello {{NAME}}!"
./target/release/apply-env -f template.yaml -n "anonymous"
```

If `$NAME` is not set, you get:

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
- Unknown flags cause the program to print an error and the help text, then exit with a non-zero code.
- `-v / --version` prints the package name and version as defined by `Cargo.toml`.

---

## Relationship to the Crystal version

This project is a Rust rewrite of the original Crystal [apply-env](https://github.com/martinmares/apply-env) utility. The goal is to keep runtime behaviour as close as possible to the Crystal implementation, while being easier to integrate in Rust-based ecosystems and tooling.

---

## License

MIT – see [`LICENSE`](./LICENSE).
