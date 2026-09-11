# page-networks

Network inventory and explicit isolated, Firemage-only, host-only, or unrestricted policies.

- `page` crate in the desktop web UI.
- Rendered by the authenticated app shell.
- Mutations use the server session and CSRF token.
- New networks suggest the first usable subnet address as their gateway until it is edited.
- Network names remain editable when attached to VMs; selectors and mutations use stable IDs.
