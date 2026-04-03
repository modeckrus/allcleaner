# TODO

## Platform support

- [ ] **Windows support** — currently Linux and macOS only. Need to:
  - Handle `sudo` alternative (or skip system update on Windows)
  - Detect package manager (e.g. winget, scoop, choco)
  - Ensure `flutter` and `rustup` work correctly on Windows
  - Re-enable `windows-latest` in `.github/workflows/release.yml`

## Done

- [x] **musl support** — Alpine Linux support via `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`
