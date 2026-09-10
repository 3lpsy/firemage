Server/client TOML configuration, CLI/environment precedence, and managed edits.

- Explicit CLI values override environment, then file settings and defaults.
- Server edits use revision checks, redact private values, and report restart requirements.
- Session TTL applies live; upstream proxy defaults and listener changes require restart.
