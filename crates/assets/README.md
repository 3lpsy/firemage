# Assets

Materialize isolated VM disks and prepare guest boot inputs.

- OCI conversion creates a root disk with a managed init; no host directory is shared into the guest.
- Seed disks are rebuilt from private staging. Destination paths, numeric ownership and mode apply inside the guest.
- Managed init mounts the seed, loads image then VM environment, installs files, runs userdata and starts the image command.
- BYO init must implement the seed protocol to consume environment, files and userdata.
- Guest output extraction uses bounded `debugfs` reads without mounting guest filesystems on the host.
