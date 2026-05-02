# Deutschlandfunk Reader

Desktop app and CLI for discovering articles from `deutschlandfunk.de`,
saving them into a local library, downloading their audio, and uploading
text + audio to LingQ as lessons.

Many Deutschlandfunk pages are primarily *audio* pieces, so the
scraper extracts the MP3 URL alongside the text and the app can both
download the file locally and attach it to the LingQ lesson on upload.

## Highlights

- Browse built-in Deutschlandfunk sections (Nachrichten, Hintergrund,
  Interview, Forschung aktuell, Kultur heute, Hörspiel, Essay, …) and load
  more candidates per section.
- Search across the public `deutschlandfunk.de/suche/` endpoint.
- Auto-fetch recent articles on startup if you enable it in the GUI.
- Save articles locally with metadata, clean text, word counts, and the
  associated audio metadata (URL, duration, file size, kicker, sophora id).
- Download the MP3 to a configurable folder; default is the app data dir.
- Optionally transcribe downloaded MP3s via `whisper.cpp` and prefer that
  transcript for LingQ uploads and reading preview.
- Filter the library by heading, section, upload status, and word count.
- Preview cleaned article text before uploading.
- Upload selected articles to a LingQ course/collection. When the local
  MP3 is present and the *Attach to LingQ upload* setting is on, the audio
  is sent in the same multipart request so the lesson has playable audio.
- Save LingQ credentials/settings in the local app data area.
- Build a Windows installer with Inno Setup.

## Tech Stack

- Rust 2024
- Slint for the desktop UI
- Tokio + Reqwest for async networking and streaming MP3 downloads
- Scraper + Regex + serde_json for HTML and `js-client-queries` JSON extraction
- Rusqlite for the local library
- Inno Setup for the Windows installer

## Running The App

Launch the GUI:

```powershell
cargo run -- gui
```

Or just:

```powershell
cargo run
```

## CLI Commands

```powershell
# List built-in section shortcuts
cargo run -- sections

# Browse a built-in Deutschlandfunk section
cargo run -- browse --section nachrichten --limit 15

# Browse an arbitrary Deutschlandfunk URL directly
cargo run -- browse-url --url https://www.deutschlandfunk.de/hintergrund-100.html --limit 15

# Fetch a single article and print the cleaned text
cargo run -- fetch --url https://www.deutschlandfunk.de/<slug>-100.html

# Fetch and also save it into the local library
cargo run -- fetch --url https://www.deutschlandfunk.de/<slug>-100.html --save

# Same, but also download the MP3 audio
cargo run -- fetch --url https://www.deutschlandfunk.de/<slug>-100.html --save --with-audio

# Just download the audio for an article
cargo run -- audio --url https://www.deutschlandfunk.de/<slug>-100.html

# Show saved articles (♪ marks rows that have audio)
cargo run -- library --limit 20

# Upload a saved article to LingQ (text only)
cargo run -- upload --id 1 --api-key YOUR_LINGQ_API_KEY

# Upload with audio attached as a multipart `audio` field
cargo run -- upload --id 1 --api-key YOUR_LINGQ_API_KEY --with-audio

# Print local app paths, DB stats, and LingQ token presence
cargo run -- doctor

# Transcribe a saved article's local MP3 with whisper.cpp
cargo run -- transcribe --id 1
```

## LingQ Authentication

The app supports multiple ways to get a LingQ token:

- pass `--api-key` on the CLI
- set `LINGQ_API_KEY`
- save a token in the GUI settings
- log in from the GUI and let the app save the token locally

## Local Storage

App data is stored under:

`%LOCALAPPDATA%\deutschlandfunk_lingq_tool\`

That includes:

- the SQLite database (`deutschlandfunk_lingq_tool.db`)
- GUI/settings data (`settings.json`)
- saved LingQ token information (`lingq_token`)
- downloaded audio files (`audio/<sophora-id-or-slug>.mp3`) — unless you
  point `audio_dir` somewhere else in settings
- Whisper transcript text + model/source tags inside the SQLite database

## Project Layout

```text
src/
  audio.rs                Audio path helpers + size/duration formatting
  database.rs             SQLite storage and queries (now with audio columns)
  deutschlandfunk.rs      deutschlandfunk.de discovery, article + audio extraction
  gui/                    Slint GUI state, callbacks, actions, sync
  lingq.rs                LingQ login, course listing, upload (with audio multipart)
  lib.rs                  Module declarations + app data directory helper
  main.rs                 CLI subcommands + GUI entry point
  settings.rs             Persistent app settings (incl. audio_dir, toggles)
  transcribe.rs           Optional whisper.cpp integration for local transcripts
ui/
  app-window.slint        Main Slint UI definition
assets/
  deutschlandfunk.ico     Embedded Windows app icon
  deutschlandfunk.png     Slint window/taskbar icon
installer/
  deutschlandfunk-reader.iss   Inno Setup installer definition
scripts/
  build-installer.ps1     Release + installer build helper
```

## Building

Debug build:

```powershell
cargo build
```

Release build:

```powershell
cargo build --release
```

## Building The Windows Installer

One-time prerequisite:

```powershell
winget install JRSoftware.InnoSetup
```

Then build the installer:

```powershell
.\scripts\build-installer.ps1
```

Expected output:

`installer\output\deutschlandfunk-reader-setup.exe`

## Notes

- The executable embeds the Deutschlandfunk app icon on Windows via
  `build.rs`, while the Slint window uses `assets/deutschlandfunk.png`.
- The release binary is configured to hide the console window on Windows.
- The app is designed as a native desktop executable, not a local web server.
- Many Deutschlandfunk pieces have only a short text intro and rely on the
  audio for their full content. The library marks those as `paywalled`
  (used purely as a "truncated" flag) so the GUI can surface them.
