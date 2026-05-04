# Development Guide

DLF LingQ Reader is a Rust desktop app. The GUI is built with Slint, async
networking uses Tokio and Reqwest, and local storage uses SQLite through
Rusqlite.

## Prerequisites

- Rust stable with the 2024 edition toolchain support
- Windows for the primary desktop/installer workflow
- Inno Setup 6 for installer builds
- Optional: `whisper.cpp` for transcription testing

Install Inno Setup:

```powershell
winget install JRSoftware.InnoSetup
```

## Common Commands

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run
cargo build --release
.\scripts\build-installer.ps1
```

## Architecture

The high-level map and rationale live in:

- [Architecture](ARCHITECTURE.md)
- [Design decisions](DECISIONS.md)
- [Processing pipeline](PROCESSING.md)

- `src/deutschlandfunk.rs` and `src/deutschlandfunk/` own source discovery,
  page parsing, section definitions, selectors, and article/audio extraction.
- `src/database.rs` owns SQLite schema migrations, article persistence,
  full-text search, exports, diagnostics, stats, and backup.
- `src/diagnostics.rs` owns redacted health reports and diagnostics bundles.
- `src/lingq.rs` owns LingQ login, collection listing, lesson creation, and
  existing lesson updates.
- `src/services/` contains higher-level workflows that compose clients and the
  database without coupling them directly to GUI code.
- `src/gui/` owns Slint state, callbacks, event handling, and syncing state to
  the UI.
- `src/main.rs` stays small and only launches the GUI.

The intended dependency direction is:

```text
GUI
  -> services
    -> deutschlandfunk / lingq / transcribe / database
```

Keep HTTP parsing, database writes, and GUI event handling separated. That makes
parser and upload behavior testable without launching the desktop app.

## Testing Strategy

- Parser regressions use offline fixtures in `tests/fixtures/`.
- Database tests use in-memory SQLite where possible.
- LingQ response handling is tested through pure parser helpers instead of live
  network calls.
- GUI helper tests cover formatting, parsing, and state-label helpers.

Before pushing, run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

GitHub Actions runs the same checks on Windows.

## Documentation Workflow

When changing user-visible behavior, update at least one of:

- [User guide](USER_GUIDE.md)
- [Troubleshooting](TROUBLESHOOTING.md)
- [Limitations](LIMITATIONS.md)
- [Changelog](../CHANGELOG.md)

When changing architecture, storage, release flow, or privacy behavior, update
the corresponding document in `docs/` during the same change.

## Installer Builds

The release installer is built with:

```powershell
.\scripts\build-installer.ps1
```

The public installer file is:

```text
installer\output\dlf-lingq-reader-setup.exe
```

The executable inside the installer remains:

```text
deutschlandfunk_lingq_tool.exe
```

Do not rename the executable or Rust package unless you also plan a settings and
database migration for existing users.

## Branding Notes

Use `DLF LingQ Reader` for public-facing documentation and installer display
name. Use `dlf-lingq-reader` for the GitHub repository name.

Keep `deutschlandfunk_lingq_tool` for:

- Cargo package name
- Windows executable name
- app-data directory
- SQLite database filename

That compatibility split protects existing installations.
