# User Guide

DLF LingQ Reader is an unofficial tool for turning Deutschlandfunk articles and
audio into LingQ lessons. It has three main surfaces: Browse, Library + LingQ,
and Audio.

## First Run

Start the app:

```powershell
cargo run
```

The app stores its database, settings, token file, backups, and default audio
folder in:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\
```

That path intentionally keeps the old internal name so existing installations
continue to work after the public rename to DLF LingQ Reader.

## Browse

Use Browse to discover and save source articles.

1. Pick a section, such as Nachrichten or Hintergrund.
2. Click Refresh to load candidates from `deutschlandfunk.de`.
3. Use the filters and date fields if you want a narrower batch.
4. Select articles and save them to the local library.
5. If audio auto-download is enabled, MP3 files are downloaded when available.

The search box uses the public Deutschlandfunk search endpoint. Press Enter to
apply the search.

## Library + LingQ

Use Library + LingQ to manage saved articles and upload them.

1. Log in with LingQ credentials or paste an existing LingQ API token.
2. Refresh or select the LingQ course/collection.
3. Filter the library by heading, section, upload state, or word count.
4. Select one or more articles.
5. Upload the selected articles.

If an article has never been uploaded, the app creates a new LingQ lesson. If
the article already has a LingQ lesson ID, the app updates that existing LingQ
lesson instead of creating a duplicate.

## Audio

Use Audio for MP3-oriented workflows.

- Download missing audio for saved articles.
- Open or reveal the configured audio folder.
- Play local MP3 files with the OS default player.
- Transcribe downloaded MP3 files when `whisper.cpp` is configured.

Audio defaults to:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\audio\
```

You can point the app at another folder from settings.

## LingQ Authentication

The app can get a LingQ token from:

- CLI flag: `--api-key`
- Environment variable: `LINGQ_API_KEY`
- GUI token field
- GUI username/password login, which saves the returned token locally

The token is stored separately from `settings.json` in:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\lingq_token
```

## Backups

Use the GUI Backup DB action or:

```powershell
cargo run -- backup
```

Backups default to:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\backups\
```

Backups are created through SQLite's backup API, so they are safer than copying
the database file while the app is running.

## Diagnostics

Run:

```powershell
cargo run -- doctor
cargo run -- doctor --json
```

The doctor command prints local paths, selected settings, token presence, and
library stats. It never prints the token value.
