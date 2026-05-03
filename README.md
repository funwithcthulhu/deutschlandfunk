# DLF LingQ Reader

[![CI](https://github.com/funwithcthulhu/dlf-lingq-reader/actions/workflows/ci.yml/badge.svg)](https://github.com/funwithcthulhu/dlf-lingq-reader/actions/workflows/ci.yml)

Unofficial Windows-first desktop app and CLI for collecting articles and MP3
audio from `deutschlandfunk.de`, keeping them in a local SQLite library, and
turning them into LingQ lessons.

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
- Back up the SQLite database from the GUI or CLI.
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

Or explicitly:

```powershell
cargo run -- gui
```

The first useful flow is:

1. Open the app.
2. Choose a Browse section and click Refresh.
3. Save a few articles into the library.
4. Open Library + LingQ.
5. Log in to LingQ or paste a token.
6. Pick a LingQ course and upload selected articles.

See [docs/USER_GUIDE.md](docs/USER_GUIDE.md) for the full GUI workflow.

## CLI Examples

```powershell
# List built-in Deutschlandfunk section shortcuts
cargo run -- sections

# Browse a built-in section
cargo run -- browse --section nachrichten --limit 15

# Browse an arbitrary Deutschlandfunk URL
cargo run -- browse-url --url https://www.deutschlandfunk.de/hintergrund-100.html --limit 15

# Fetch one article and print cleaned text
cargo run -- fetch --url https://www.deutschlandfunk.de/<slug>-100.html

# Fetch, save, and download MP3 audio when available
cargo run -- fetch --url https://www.deutschlandfunk.de/<slug>-100.html --save --with-audio

# Download only the article audio
cargo run -- audio --url https://www.deutschlandfunk.de/<slug>-100.html

# Show saved articles
cargo run -- library --limit 20

# Upload a saved article to LingQ
cargo run -- upload --id 1 --api-key YOUR_LINGQ_API_KEY

# Upload with local audio attached
cargo run -- upload --id 1 --api-key YOUR_LINGQ_API_KEY --with-audio

# Show paths, settings, token presence, and library stats
cargo run -- doctor
cargo run -- doctor --json

# Create a SQLite-safe database backup
cargo run -- backup

# Transcribe a downloaded MP3 with whisper.cpp
cargo run -- transcribe --id 1
```

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
- [Development guide](docs/DEVELOPMENT.md)
- [Database and storage](docs/DATABASE.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)

## Project Layout

```text
src/
  audio.rs                Audio path helpers and size/duration formatting
  database.rs             SQLite storage, migrations, backup, export, search
  deutschlandfunk.rs      deutschlandfunk.de discovery and article extraction
  deutschlandfunk/        Parser modules, selectors, sections, models
  gui/                    Slint GUI state, callbacks, actions, sync
  lingq.rs                LingQ login, course listing, upload/update client
  main.rs                 CLI subcommands and GUI entry point
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
