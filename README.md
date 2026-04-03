# allcleaner

One-command updater for Linux, Flutter, and Rust. Updates your system and cleans project caches in parallel.

## What it does

Running `allcleaner` performs three tasks **in parallel** using Tokio:

1. **System update** — detects your package manager (`apt`, `dnf`, or `pacman`) and runs a system-wide update
2. **Flutter update** — runs `flutter upgrade` (only if updates are available), then finds all Flutter projects in `~/dev` and runs `flutter clean` in each
3. **Rust update** — runs `rustup update` (only if updates are available), then finds all Rust projects in `~/dev` and runs `cargo clean && rm Cargo.lock` in each

All stdout/stderr from update commands is streamed to the terminal in real time so you can track progress.

## Installation

### Via `cargo-binstall` (recommended — fast, no compilation)

```bash
cargo binstall allcleaner
```

### Via `cargo` (builds from source)

```bash
cargo install allcleaner
```

## Usage

```bash
allcleaner <sudo_password>
```

### Example

```bash
allcleaner mypassword123
```

### Logging

By default, `allcleaner` logs at `info` level for external crates and `debug` for itself. You can override this with the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug allcleaner <sudo_password>
```

## Requirements

- **Linux** with one of: `apt`, `dnf`, or `pacman`
- **Flutter** (optional — skipped if not installed)
- **Rust** via rustup (optional — skipped if not installed)

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.
