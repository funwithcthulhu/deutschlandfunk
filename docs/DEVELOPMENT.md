# Development Guide

This project is a Rust desktop app plus CLI. The GUI is built with Slint, async
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

- `src/deutschlandfunk.rs` and `src/deutschlandfunk/` own source discovery,
  page parsing, section definitions, selectors, and article/audio extraction.
- `src/database.rs` owns SQLite schema migrations, article persistence,
  full-text search, exports, stats, and backup.
- `src/lingq.rs` owns LingQ login, collection listing, lesson creation, and
  existing lesson updates.
- `src/services/` contains higher-level workflows that compose clients and the
  database without coupling them directly to GUI code.
- `src/gui/` owns Slint state, callbacks, event handling, and syncing state to
  the UI.
- `src/main.rs` owns CLI argument parsing and command dispatch.

The intended dependency direction is:

```text
CLI / GUI
  -> services
    -> deutschlandfunk / lingq / transcribe / database
```

Try to keep HTTP parsing, database writes, and GUI event handling separated.
That makes it easier to test parser and upload behavior without launching the
desktop app.

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

## Installer Builds

The release installer is built with:

```powershell
.\scripts\build-installer.ps1
```

The public installer file is:

```text
installer\output\dlf-lingq-reader-setup.exe
```

The executable inside the installer intentionally remains:

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

That compatibility split is intentional.
