# Troubleshooting

## LingQ Upload Says It Updated An Existing Lesson

That means the local article already had a LingQ lesson ID. The app sent an
update request to LingQ for that lesson instead of creating a duplicate.

If LingQ returns a successful status with an empty or partial response, the app
trusts the known lesson ID and marks the update as successful.

## LingQ Upload Fails

Check:

- The app says LingQ is connected.
- The selected language is correct, usually `de`.
- A LingQ course/collection is selected if you want lessons grouped.
- The article has non-empty text or a transcript.
- The token was not revoked in LingQ.
- The Health panel does not report database integrity problems.

Open Library + LingQ and check that the status badge says LingQ is connected.
If not, open Settings, save a token or log in again, then refresh courses.

## Audio Did Not Attach To LingQ

LingQ lesson creation can succeed even if the audio part is not stored by
LingQ. Check:

- The local MP3 path exists.
- The Library + LingQ setting `Attach to LingQ upload` is enabled.
- The file is a normal MP3 and not zero bytes.
- The lesson did not already exist with a server-side restriction.

The app logs a warning when it sent audio but LingQ's response did not include
an audio URL.

## Article Text Looks Short

Many Deutschlandfunk pages are primarily audio pieces and only publish a short
intro as text. The app marks these as truncated internally and surfaces them in
the UI. Download and transcribe the MP3 if you want fuller lesson text.

## Search Or Browse Finds Nothing

Try:

- A broader section.
- A smaller or empty date filter.
- The site search box instead of section browsing.
- Running again later if `deutschlandfunk.de` changed markup or rate limited
  requests.

Parser regressions should be covered by adding fixtures under `tests/fixtures/`.

## Database Is Locked

Close extra running instances of the app. The database has a short busy timeout,
but a long-running copy, backup, or external SQLite viewer can still hold a
lock.

Use the built-in GUI Backup DB action instead of copying the live database
manually.

Open the Audio page and refresh the Health panel. If `integrity_check` is not
`ok`, create a backup copy before doing more troubleshooting.

## Diagnostics Export

Use Export Diagnostics from the GUI when you need a compact support snapshot.
The bundle is written under:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\diagnostics\
```

It is redacted, but still review it before sharing because local paths and
library counts may be personal.

## Where Are My Files?

The important default directory is:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\
```

The public app name changed to DLF LingQ Reader, but this internal storage path
is intentionally unchanged.

## Installer Build Cannot Find Inno Setup

Install Inno Setup:

```powershell
winget install JRSoftware.InnoSetup
```

Then rerun:

```powershell
.\scripts\build-installer.ps1
```
