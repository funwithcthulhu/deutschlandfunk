# Processing Pipeline

Content flow and reliability checks:

## Browse And Search

1. The GUI asks `DeutschlandfunkClient` for a section or search page.
2. Requests are concurrency-limited and lightly spaced to avoid request bursts.
3. Recent browse/search responses are cached briefly for repeated UI actions.
4. HTML parsers extract summaries, dates, sections, article URLs, and audio
   candidates.
5. The GUI displays candidates without writing them until the user saves.

## Save Articles

1. `services::ingest` fetches the full article page.
2. Parser modules normalize title, subtitle, body text, section, date, and audio
   metadata.
3. SQLite upserts the article by source URL.
4. If audio auto-download is enabled, the MP3 is saved into the configured audio
   directory and metadata is recorded.
5. The library refreshes and full-text search indexes are maintained by SQLite
   triggers.

## Audio

Audio files are stored outside the database as MP3 files. SQLite stores paths,
source URLs, duration, byte size, MIME type, and download status.

Before attaching audio to LingQ, the app validates that the local file exists,
has a `.mp3` extension, and is not empty. This avoids sending obviously broken
attachments and gives a clearer failure before a network request is made.

## LingQ Uploads

1. The app validates the selected article ID and loads the article through a
   typed `ArticleId`.
2. Upload status moves through `pending` and `uploading`.
3. If the article already has a LingQ lesson ID, the app updates that lesson
   instead of creating a duplicate.
4. On success, SQLite records the lesson ID, URL, `succeeded` status, and a sync
   event.
5. On failure, SQLite records `failed`, the redacted error message, and a sync
   event.

## Transcription

Transcription is optional and depends on a local `whisper.cpp` executable and
model path. The app keeps transcription separate from upload so users can choose
whether MP3-heavy articles become text-rich LingQ lessons.

## Performance Guardrails

- Deutschlandfunk fetches use a shared concurrency gate.
- GUI background jobs use a bounded limiter.
- Browse/search caches are short-lived and capped.
- SQLite has targeted indexes for common library filters, audio lookups, upload
  state, and sync-event inspection.
- Heavy filesystem/database health work runs off the UI path.
