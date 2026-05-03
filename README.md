# DLF LingQ Reader

[![CI](https://github.com/funwithcthulhu/dlf-lingq-reader/actions/workflows/ci.yml/badge.svg)](https://github.com/funwithcthulhu/dlf-lingq-reader/actions/workflows/ci.yml)

Unofficial Windows-first desktop app for collecting articles and MP3 audio from
`deutschlandfunk.de`, keeping them in a local SQLite library, and turning them
into LingQ lessons.

This project is not affiliated with, endorsed by, or sponsored by
Deutschlandradio, Deutschlandfunk, or LingQ.

## What It Does

- Browse built-in Deutschlandfunk sections and save selected articles.
- Search `deutschlandfunk.de/suche/` from the app.
- Keep a local library with title, metadata, cleaned text, word count, audio
  metadata, transcripts, and LingQ upload state.
- Download article MP3 files into a configurable local folder.
- Optionally transcribe downloaded audio with `whisper.cpp`.
- Upload text-only or text-plus-audio lessons to LingQ.
- Update existing LingQ lessons in place instead of creating duplicates.
- Back up the SQLite database from the GUI.
- View local database/app health, optimize SQLite, and export redacted
  diagnostics.
- Build a Windows installer with Inno Setup.

## Naming And Compatibility

The public project name is **DLF LingQ Reader** and the intended GitHub repo
name is `dlf-lingq-reader`.

For compatibility, the Rust package, executable, and app-data directory remain:

```text
deutschlandfunk_lingq_tool
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\
```

Do not rename those lightly. They are how existing installations find the
current database, settings, audio folder, and LingQ token.

## Quick Start

Run the GUI from source:

```powershell
cargo run
```

The first useful flow is:

1. Open the app.
2. Choose a Browse section and click Refresh.
3. Save a few articles into the library.
4. Open Library + LingQ.
5. Log in to LingQ or paste a token.
6. Pick a LingQ course and upload selected articles.

See [docs/USER_GUIDE.md](docs/USER_GUIDE.md) for the full GUI workflow.

## Build And Test

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Build a release binary:

```powershell
cargo build --release
```

## Windows Installer

One-time prerequisite:

```powershell
winget install JRSoftware.InnoSetup
```

Build the installer:

```powershell
.\scripts\build-installer.ps1
```

Expected output:

```text
installer\output\dlf-lingq-reader-setup.exe
```

The installer display name is `DLF LingQ Reader`. The executable inside the
installer remains `deutschlandfunk_lingq_tool.exe` for compatibility.

## Documentation

- [User guide](docs/USER_GUIDE.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Design decisions](docs/DECISIONS.md)
- [Processing pipeline](docs/PROCESSING.md)
- [Development guide](docs/DEVELOPMENT.md)
- [Database and storage](docs/DATABASE.md)
- [Privacy](docs/PRIVACY.md)
- [Limitations](docs/LIMITATIONS.md)
- [Release checklist](docs/RELEASE.md)
- [Screenshots](docs/SCREENSHOTS.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)

## Project Layout

```text
src/
  audio.rs                Audio path helpers and size/duration formatting
  database.rs             SQLite storage, migrations, backup, export, search
  diagnostics.rs          Redacted health reports and diagnostics bundles
  deutschlandfunk.rs      deutschlandfunk.de discovery and article extraction
  deutschlandfunk/        Parser modules, selectors, sections, models
  gui/                    Slint GUI state, callbacks, actions, sync
  ids.rs                  Typed database identifiers
  lingq.rs                LingQ login, course listing, upload/update client
  main.rs                 GUI entry point
  services/               Ingest, upload, and transcription workflows
  settings.rs             Persistent app settings and LingQ token storage
  transcribe.rs           Optional whisper.cpp integration
tests/
  fixtures/               Offline parser fixtures
ui/
  app-window.slint        Main Slint UI
assets/
  deutschlandfunk.ico     Embedded Windows app icon
  deutschlandfunk.png     Window/taskbar icon
installer/
  deutschlandfunk-reader.iss   Inno Setup definition
scripts/
  build-installer.ps1     Release and installer build helper
```

## License

No open-source license has been selected yet. See [LICENSE](LICENSE).
