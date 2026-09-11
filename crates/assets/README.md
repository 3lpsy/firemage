# Assets

Materialize isolated VM disks and prepare guest boot inputs.

- OCI conversion uses `firemage-oci` to pull and unpack a digest-pinned image, then creates an ext4 disk with managed init. Registry access is native and uses resolved vault credentials.
- Seed disks are rebuilt from private staging. Destination paths, numeric ownership and mode apply inside the guest.
- Managed init mounts the seed, loads environment, installs files, runs userdata, then starts the image command or literal argv override. It reports exit status and stops, or waits in keep-alive mode.
- Stopped-guest browsing uses numeric inodes, limits directory listings, and streams regular-file downloads through an isolated read-only parser. Symlinks and special files are never downloaded.
- Jailed output extraction requires Bubblewrap namespaces and seccomp; only the disk, parser and required libraries are visible, all read-only. Output and parser resources are bounded.
- Anonymous materialization rejects unresolved registry settings; authenticated pulls require resolved options. Registry secrets never enter the guest disk.
- OCI simple boot requires root USER and `/bin/sh`; BYO init implements the seed protocol for files, environment and userdata. Temporary extraction trees are discarded after success or failure.
