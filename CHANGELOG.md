# Changelog

All notable changes to DLF LingQ Reader are tracked here.

## Unreleased

- Rename public project branding from Deutschlandfunk Reader to DLF LingQ Reader.
- Document the compatibility split between public branding and the existing
  `deutschlandfunk_lingq_tool` package, executable, database, and app-data path.
- Add user, development, database, troubleshooting, contributing, and security
  documentation.
- Add Dependabot configuration for Cargo and GitHub Actions updates.

## 1.0.0

- Initial Windows desktop app and CLI.
- Browse and search Deutschlandfunk article sources.
- Save articles into a local SQLite library.
- Download MP3 audio and store audio metadata.
- Upload text and optional audio to LingQ.
- Update existing LingQ lessons without creating duplicates.
- Optional `whisper.cpp` transcription support.
- Database backup command and GUI action.
- Windows installer build script.
