# Database And Storage

DLF LingQ Reader keeps user data locally. There is no hosted backend.

## App Data Directory

The app-data directory is:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\
```

This internal name is retained for compatibility after the public rename.

Important files and folders:

```text
deutschlandfunk_lingq_tool.db   SQLite library database
settings.json                   GUI and workflow settings
lingq_token                     Saved LingQ token
audio\                          Default downloaded MP3 folder
backups\                        Default database backup folder
```

## Schema Management

The database has a legacy `schema_version` table and a newer
`schema_migrations` ledger. New migrations should:

1. Run inside an explicit transaction.
2. Be safe for existing user databases.
3. Insert the new version into `schema_version`.
4. Call `record_migration` with a human-readable migration name.

SQLite foreign keys are enabled on connection open, and a busy timeout is set
to reduce transient lock errors.

## Core Tables

`articles` is the main library table. It stores source URL, article metadata,
clean text, word count, difficulty, upload state, LingQ lesson ID/URL, audio
metadata, and transcript fallback columns kept for compatibility.

`articles_fts` is an FTS5 index over title, subtitle, and cleaned text.
Triggers keep it synchronized with `articles`.

`audio_assets` mirrors article audio metadata in a related table keyed by
`article_id`.

`transcripts` stores transcript history by article. The latest transcript can
be used for previews and LingQ upload text.

`lingq_uploads` records LingQ lesson IDs and URLs by article so upload history
is not only stored on the article row.

`sync_events` records workflow events such as LingQ upload status changes. It is
used for diagnostics and for making failed/pending/uploading/succeeded states
durable across app restarts.

## Current Schema Notes

Schema version 11 adds durable LingQ upload status fields and sync events:

- `articles.lingq_upload_status`
- `articles.lingq_upload_error`
- `articles.lingq_upload_attempted_at`
- `sync_events`

It also adds indexes for common library filters, audio lookups, LingQ lesson
lookups, upload status inspection, and sync-event history.

## Health And Optimization

The app can report database health from the GUI. Health includes schema version,
migration count, journal mode, foreign-key status, integrity check, page counts,
database size, and latest backup path.

The Audio page Health panel exposes an Optimize action that runs SQLite
optimization pragmas. Migrations also run `ANALYZE` after adding the schema
version 11 indexes.

## Backups

Use the GUI Backup DB action.

Backups default to:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\backups\
```

The app creates backups through SQLite-safe `VACUUM INTO`. Prefer this over
manually copying the live database file.

## Restore

To restore manually:

1. Close the app.
2. Copy the backup `.db` file over:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\deutschlandfunk_lingq_tool.db
```

3. Reopen the app.

If you are unsure, make another copy of the current database before replacing
it.
