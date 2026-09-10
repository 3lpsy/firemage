Authenticated Firemage API client for HTTPS and Unix sockets.

- Remote HTTP is restricted to loopback; HTTPS supports a private CA certificate.
- JSON requests and binary uploads share bearer authentication and error handling.
- Binary downloads enforce the caller's byte limit, including chunked responses.
- Redirects are disabled to keep credentials on the configured endpoint.
