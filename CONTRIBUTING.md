# Contributing

DLF LingQ Reader is small enough that clear, focused changes are easier to
review than broad rewrites.

## Local Checks

Run these before opening a pull request:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Good First Changes

- Add parser fixtures for Deutschlandfunk pages with unusual markup.
- Improve troubleshooting docs when a real failure mode is discovered.
- Add tests around LingQ response shapes.
- Tighten GUI wording where the app status could be clearer.

## Architecture Expectations

- Keep GUI orchestration in `src/gui/`.
- Keep reusable workflows in `src/services/`.
- Keep HTTP clients and parsers outside GUI code.
- Keep database migrations backward compatible with existing user databases.
- Do not rename `deutschlandfunk_lingq_tool` storage/package identifiers without
  a migration plan.

## Pull Request Notes

Include:

- What changed.
- How you tested it.
- Any migration or compatibility risk.
- Screenshots for UI changes when possible.
