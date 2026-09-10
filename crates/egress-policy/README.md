Serializable per-VM egress policy, shared by the service and desktop UI.

- Exact HTTP authority, scheme, method and path-prefix allowlists; empty rules deny.
- HTTP private destinations require explicit IP/CIDR approval. TCP tunnels use fixed targets.
- Named credential references, AWS SigV4/HMAC signing, and optional HTTP/HTTPS/SOCKS5 upstream proxy.
- `validate()` rejects malformed rules and conflicting listener ports; `secret_names()` collects owner-scoped references.
- `inherit_upstream = false` selects direct transport when the server has a default upstream.
