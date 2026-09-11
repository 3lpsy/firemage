# firemage-guest-protocol

Bounded JSON framing shared by the guest shell service and its host connection.

- Frames carry a four-byte big-endian length followed by tagged JSON, limited to 64 KiB.
- Open validates protocol version, command arguments, and terminal dimensions.
- Input and output use base64 to preserve terminal bytes; input is limited to 16 KiB.
