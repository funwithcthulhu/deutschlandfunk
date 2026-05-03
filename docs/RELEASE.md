# Release Checklist

Use this checklist before publishing a Windows build.

## Verify

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run a GUI smoke test:

```powershell
cargo build --release
Start-Process .\target\release\deutschlandfunk_lingq_tool.exe
```

Check:

- The app opens without a console window.
- Browse refresh returns current articles.
- Library filters still work with the local database.
- LingQ settings can show connected/disconnected state without exposing tokens.
- Audio folder, Backup DB, Health refresh, Optimize, and Diagnostics export
  buttons work.

## Build Installer

```powershell
.\scripts\build-installer.ps1
```

Expected output:

```text
installer\output\dlf-lingq-reader-setup.exe
```

## Release Notes

Mention:

- User-facing changes.
- Parser or LingQ behavior changes.
- Database migration version, if changed.
- Installer filename and compatibility note that the internal executable remains
  `deutschlandfunk_lingq_tool.exe`.

## Rollback

If a release is bad:

1. Keep the previous installer available.
2. Ask users to create a Backup DB from the app before downgrading.
3. Avoid downgrading across irreversible schema changes unless a migration plan
   exists.
