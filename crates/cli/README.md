The `firemage` server and remote-management command line.

- Server/client options resolve from CLI, environment, and TOML.
- Remote commands share the authenticated JSON API; credentials stay separate from config.
- Production builds omit `tests-infra`; opt-in development builds expose its confirmed test command.
- Named secrets use a hidden terminal prompt or stdin; VM input uploads remain independent of guest networking.
- `kernel list/upload/download/alias/delete` manages the server catalog by filename. Downloads require SHA-256; writes require an administrator.
- `asset list/upload/download/alias/delete` manages your reusable VM input files; upload requires a unique `--alias`.
