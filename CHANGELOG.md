# Changelog

All notable changes to DLF LingQ Reader are tracked here.

## Unreleased

- Remove CLI subcommands; DLF LingQ Reader is now GUI-only.
- Rename public project branding from Deutschlandfunk Reader to DLF LingQ Reader.
- Document the compatibility split between public branding and the existing
  `deutschlandfunk_lingq_tool` package, executable, database, and app-data path.
- Add user, development, database, troubleshooting, contributing, and security
  documentation.
- Add Dependabot configuration for Cargo and GitHub Actions updates.
- Add typed article IDs for upload paths and database access.
- Add durable LingQ upload status, sync-event history, and database schema
  version 11.
- Add database health reporting and redacted diagnostics export from the GUI.
- Add GUI-triggered SQLite optimization from the Health panel.
- Add Deutschlandfunk request limiting plus short-lived browse/search caching.
- Add bounded GUI background job execution to reduce concurrent workload spikes.
- Validate local MP3 attachments before sending LingQ upload requests.
- Add architecture, design decision, processing, privacy, limitations, release,
  and screenshot documentation.

## 1.0.0

- Initial Windows desktop app with CLI support.
- Browse and search Deutschlandfunk article sources.
- Save articles into a local SQLite library.
- Download MP3 audio and store audio metadata.
- Upload text and optional audio to LingQ.
- Update existing LingQ lessons without creating duplicates.
- Optional `whisper.cpp` transcription support.
- Database backup support.
- Windows installer build script.
