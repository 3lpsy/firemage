Linux TAP creation and nftables guest policy enforcement.

- Filters TAP ingress and egress before host routing; blocks spoofed source IP/MAC and IPv6.
- Policies cover isolation, one host address, Firemage-only TCP listeners, or unrestricted IPv4.
- Installs firewall rules before bringing the interface up; cleanup brings it down before removing rules.
