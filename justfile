set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

macos_target := "aarch64-apple-darwin"
linux_target := "x86_64-unknown-linux-musl"
windows_target := "x86_64-pc-windows-gnu"

build-macos:
    cargo build --release --target {{ macos_target }}

build-linux:
    cargo zigbuild --release --target {{ linux_target }}

build-windows:
    cargo build --release --target {{ windows_target }}
