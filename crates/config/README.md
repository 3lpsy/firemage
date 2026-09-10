Server/client TOML configuration, CLI/environment precedence, and managed edits.

- Explicit CLI values override environment, then file settings and defaults.
- Server edits use revision checks, redact private values, and report restart requirements.
- Session TTL applies live; upstream proxy defaults and listener changes require restart.
- Executables, host paths, identity ranges and privilege permissions are deployment-owned; API edits cannot change them.
- Unix listeners default to `0600`; shared access requires explicit `0660` and a numeric group ID.
