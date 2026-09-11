# page-assets

Uploaded file library at the desktop `assets` route.

- Page crate hosted by the authenticated app shell; lists the current owner's assets.
- Searches typed asset records by alias or original filename and shows VM reference counts.
- Administrators upload files up to the configured server limit, default 1 GiB, and edit aliases unique to their library.
- Add asset opens a dialog for file upload or server download from a public HTTPS URL, with an optional SHA-256 checksum.
- Alias and download guidance lives in information popups beside field labels; catalog refresh uses an icon.
- Deletion requires confirmation and is disabled for attached files; the server enforces references and ownership.
