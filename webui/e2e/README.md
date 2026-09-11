Desktop browser acceptance journeys against an isolated Firemage server.

- `just test-e2e` embeds the UI in the binary, then runs the ignored tests serially through Chromium and chromedriver.
- Covers VM/network management, local sessions, user permissions, API tokens, TOML validation and conflicts, and OIDC login through a fixture HTTPS identity provider.
- Covers Firemage-only network rules, HTTP credential injection/signing, upstream proxy selection, secret replacement, guest environment references, and boot file ownership/permissions.
- Covers complete Create/Edit pages, draft section Apply/Cancel, kernel and file catalogs, secret attachments, portable configuration import/export, duplication and fresh network identities.
- Covers snapshot upload, download, search and deletion, restore trust controls, and file-browser state guards. Populated tree navigation uses a browser fetch fixture; `tests-infra` covers real ext4 reads.
- Each journey uses embedded assets without a source directory and owns its SQLite database and child processes. VM start deliberately uses a missing Firecracker executable to verify error handling; real guests belong to `tests-infra`.
- PNGs capture intermediate states and failures. Server/driver logs and failure HTML stay in the CI results directory, including when an assertion panics.
- OIDC uses a generated test CA and strict server trust. The fixture browser accepts its test certificate; bundled signing material is only for the mock identity provider.
