# Design Decisions

This file captures the reasoning behind important project choices. Keep entries
short and practical; if a decision affects compatibility or user data, document
the tradeoff here before changing it.

## GUI-Only App

DLF LingQ Reader is a desktop GUI app, not a CLI plus GUI bundle.

Rationale:

- The user workflow is visual: browse articles, select rows, inspect upload
  state, download audio, and manage LingQ settings.
- Removing CLI entry points reduces support surface and keeps release testing
  focused on one product.
- Automation can still be added later through explicit GUI-safe workflows if
  there is a real need.

## Compatibility Names Stay Internal

Public name: `DLF LingQ Reader`.

Repository name: `dlf-lingq-reader`.

Compatibility name: `deutschlandfunk_lingq_tool`.

Rationale:

- Existing users already have app data under
  `%LOCALAPPDATA%\deutschlandfunk_lingq_tool\`.
- Keeping the executable, Cargo package, database file, settings file, and token
  file stable avoids data-loss-prone migrations.
- The public name avoids looking like an official Deutschlandfunk property.

## SQLite As The Local Source Of Truth

The app uses a local SQLite database instead of loose JSON files or a hosted
backend.

Rationale:

- Article text, audio metadata, upload state, transcript history, and FTS search
  all fit SQLite well.
- Offline use matters; users should keep control over their library.
- Schema migrations let us evolve storage while preserving existing installs.

## Service Boundary Between GUI And Clients

GUI code should call workflow services instead of directly orchestrating every
database, Deutschlandfunk, LingQ, audio, and transcription operation.

Rationale:

- Services are easier to test without launching Slint.
- Error handling can stay close to the workflow that created the error.
- GUI state remains mostly presentation and event handling, not business logic.

## Redacted Diagnostics

Diagnostics bundles should expose health and environment shape without exposing
LingQ tokens or credentials.

Rationale:

- Support needs schema version, paths, counts, and storage health.
- Tokens are secrets and should never appear in logs, diagnostics, screenshots,
  or issue templates.
