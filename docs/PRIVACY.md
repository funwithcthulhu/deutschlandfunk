# Privacy

DLF LingQ Reader stores data locally and has no hosted backend.

## Local Data

The default data directory is:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\
```

It can contain:

- The SQLite library database.
- GUI settings.
- A saved LingQ token file.
- Downloaded MP3 files.
- Database backups.
- Optional diagnostics bundles exported by the user.

## Network Requests

The app contacts:

- `deutschlandfunk.de` for article pages, search, section browsing, and MP3
  downloads.
- LingQ endpoints for login, course listing, lesson creation, and lesson
  updates when the user connects LingQ.

No project-operated server receives app data.

## LingQ Token Handling

The LingQ token is stored separately from `settings.json` in:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\lingq_token
```

The UI shows whether a token exists, but it does not display the token value
after saving. Diagnostics report token presence only.

## Diagnostics

Diagnostics exports are redacted. They include app health, schema version,
counts, configured paths, and whether a LingQ token is present. They do not
include LingQ tokens, passwords, or article body text.

Review a diagnostics bundle before sharing it publicly if your local paths or
library counts are sensitive.
