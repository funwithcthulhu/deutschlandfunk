# DLF LingQ Reader

Personal Rust/Slint desktop tool for saving Deutschlandfunk articles locally and
sending selected items to LingQ.

This is a personal utility, not an official app. I use it to collect text and
optional MP3 audio from `deutschlandfunk.de`, keep that material in a local
SQLite library, and push selected items into LingQ.

Not affiliated with Deutschlandradio, Deutschlandfunk, or LingQ.

## What It Does

- Browse or search Deutschlandfunk articles.
- Save article text, metadata, audio info, transcripts, and LingQ upload state
  in a local SQLite database.
- Download article MP3s.
- Optionally transcribe MP3s with `whisper.cpp`.
- Upload selected articles to LingQ.
- Update an existing LingQ lesson when the article already has a saved LingQ
  lesson ID.
- Back up the database from the GUI.

## Compatibility

The repo/display name is **DLF LingQ Reader**, but the Rust package,
executable, database, settings, token file, and app-data folder still use the
old internal name:

```text
deutschlandfunk_lingq_tool
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\
```

Do not rename those without a migration plan, or existing local data may stop
being found.

## Run

From this folder:

```powershell
cargo run
```

Typical use:

1. Open the app.
2. Choose a Browse section and click Refresh.
3. Save a few articles into the library.
4. Open Library + LingQ.
5. Log in to LingQ or paste a token.
6. Pick a LingQ course and upload selected articles.

## Checks

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

Install Inno Setup once:

```powershell
winget install JRSoftware.InnoSetup
```

Build the installer:

```powershell
.\scripts\build-installer.ps1
```

Installer output:

```text
installer\output\dlf-lingq-reader-setup.exe
```

The installer display name is `DLF LingQ Reader`. The executable inside the
installer remains `deutschlandfunk_lingq_tool.exe` for compatibility.

## Local Data

Default app data:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\
```

The LingQ token is stored locally at:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\lingq_token
```

Downloaded MP3s default to:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\audio\
```

## License

This repo is public, but the code is not open source. See [LICENSE](LICENSE).
