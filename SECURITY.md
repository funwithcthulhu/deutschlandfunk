# Security

## Supported Versions

Only the current `main` branch is actively maintained.

## Reporting A Vulnerability

Do not open a public issue for sensitive reports involving token exposure,
credential handling, or unsafe file access.

Use GitHub private vulnerability reporting if it is enabled on the repository.
If it is not enabled, contact the repository owner directly.

## Token Handling

LingQ tokens are stored locally in:

```text
%LOCALAPPDATA%\deutschlandfunk_lingq_tool\lingq_token
```

The `doctor` command reports token presence and length only. It must never print
the token value.
