# Limitations And Edge Cases

DLF LingQ Reader depends on two external sites and local desktop resources.
Known limitations:

## Deutschlandfunk Markup Can Change

If `deutschlandfunk.de` changes HTML structure, section browse, search, article
text, or audio extraction can regress. Add an offline fixture under
`tests/fixtures/` when fixing parser changes so the behavior stays covered.

## Some Articles Are Audio-First

Some pages publish only a short teaser plus audio. The app can save and upload
that short text, but a better LingQ lesson may require downloading and
transcribing the MP3.

## LingQ Responses Can Be Partial

LingQ sometimes accepts an update while returning a small or partial response.
For existing lessons, the app trusts the known local lesson ID when the response
status indicates success.

## Audio Uploads Are Best Effort

LingQ lesson creation can succeed even if the uploaded audio is not reflected
back in the API response. The app validates local MP3 files before upload and
logs/report status, but server-side media behavior can still vary.

## Local Database Locks

SQLite handles normal app use well, but external SQLite viewers, backup tools,
or multiple running app instances can temporarily lock the database. Prefer the
built-in Backup DB action.

## Windows Is The Primary Target

The code is Rust and mostly cross-platform, but packaging and installer testing
are Windows-first.
