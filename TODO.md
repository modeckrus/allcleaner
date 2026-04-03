# TODO

## Platform support

- [ ] **Windows support** — currently Linux and macOS only. Need to:
  - Handle `sudo` alternative (or skip system update on Windows)
  - Detect package manager (e.g. winget, scoop, choco)
  - Ensure `flutter` and `rustup` work correctly on Windows
  - Re-enable `windows-latest` in `.github/workflows/release.yml`
