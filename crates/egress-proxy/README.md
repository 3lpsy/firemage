Embedded HTTP proxy and TCP tunnel listeners for isolated VM egress.

- Listener identity is gateway address, port and registered guest source IP; unregister revokes connections and releases unused ports.
- HTTPS CONNECT always terminates TLS for per-request policy checks. The persisted private CA signs guest-facing certificates; upstream TLS remains verified.
- DNS resolves once per connection, and approved numeric destinations are used through authenticated HTTP/HTTPS CONNECT or SOCKS5 proxies. Upstream failure never falls back to direct access.
- Credential injection and request signing run after authorization. Request bodies are limited to 16 MiB; responses stream, including server-sent events.
- Gateway firewall and source-IP enforcement belong to the runtime. Guests must explicitly trust the exported public CA.
