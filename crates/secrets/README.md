# Secrets

Owner-scoped, write-only named secrets encrypted in SQLite.

- XChaCha20-Poly1305 binds each ciphertext to its owner and name.
- `secrets.key` is created atomically with mode 0600; symlinks and public permissions are rejected.
- Keep the key with database backups. Losing it makes stored secrets unreadable.
- Public responses contain names and update timestamps. `resolve` is internal only.
- `scoped` supplies the proxy with a resolver restricted to one owner.
