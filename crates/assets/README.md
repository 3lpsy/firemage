# Assets

Materialize isolated VM disks and prepare guest boot inputs.

- OCI conversion creates a root disk with a managed init; no host directory is shared into the guest.
- Seed disks are rebuilt from private staging. Destination paths, numeric ownership and mode apply inside the guest.
- Managed init mounts the seed, loads image then VM environment, installs files, runs userdata and starts the image command.
- BYO init must implement the seed protocol to consume environment, files and userdata.
- Jailed output extraction requires Bubblewrap namespaces and seccomp; only the disk, parser and required libraries are visible, all read-only. Output and parser resources are bounded.
- Trusted output extraction and OCI import helpers run directly on the host; neither mounts a guest filesystem in the host kernel.
