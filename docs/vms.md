# Virtual machines

Create and Edit use the same page. Expand a section to configure its settings;
Apply in a section editor updates the draft. Create VM saves the full definition.
It does not start a guest. Start prepares missing resources and boots the VM.
Prepare creates the resources and leaves Firecracker ready without booting.
Stop a prepared or running VM before editing its configuration.

The VM list opens a side panel; Open full page shows the same controls and tabs
on a dedicated page. Duplicate VM copies configuration and references, assigns
fresh network addresses, and leaves the new VM unprepared. It does not copy disks
or snapshots. Delete VM removes managed VM files and optionally its snapshots.
Shared kernels, file assets and secrets remain in their libraries.

## Attachments and configuration

Upload files in Assets with a unique alias. In the VM form, attach an asset or
secret and choose its absolute guest destination, UID, GID and mode. Secret files
default to mode 0600. Their values are resolved when preparing the private seed
disk; configuration exports contain secret names. The Attachments tab lists what
was configured, without displaying secret contents.

Configuration exports use kernel and file aliases. Import resolves those aliases
and secret/network names in the destination account. Local host paths and external
Firecracker sockets are not portable and cannot be exported. Import assigns fresh
network addresses for a new VM. Applying configuration to an existing VM uses the
addresses in the imported document and validates them against reserved addresses.

## Workloads and guest output

For OCI images, Firemage supplies init. It mounts the seed, configures networking,
exports environment variables, installs attachments, runs userdata, then runs the
image's Entrypoint and Cmd. A command override replaces both using a literal array
of arguments, for example `["/bin/sh", "-c", "make check"]`.

One-shot mode stops the guest when the main program exits. Keep-alive records the
exit status and leaves the guest running without restarting the program. The exit
status is written to `/firemage/output/exit-code`; init progress and process output
go to the Serial tab. Prepared OCI disks from older versions require a new VM to
use a command override or keep-alive mode.

For non-OCI images, the image supplies init and owns its process lifecycle. Run the
main command from that init, record its exit status, sync the disk and shut down or
reboot the guest when it finishes. Userdata runs only when the image's init handles
the Firemage seed convention. Firemage cannot override an arbitrary image's init
or discover its main process.

Applications can write results anywhere on the root disk. The conventional output
location is `/firemage/output/result`; Firemage does not redirect application
stdout there automatically. The Files tab browses the root disk of a stopped VM
and downloads regular files. It does not follow symlinks or browse extra drives.

## Serial and terminal

Serial shows guest ttyS0 output. Firecracker shows API and process diagnostics.
Enable terminal input in the VM configuration and provide a console program in the
guest, such as a serial getty. Enabling terminal input does not install a shell or
bypass guest login. Use `console=ttyS0` in the kernel arguments.

Terminal input is available while the server owns the running Firecracker process.
After restarting the server, stop and start the VM to reconnect input. Serial logs
remain readable. Older combined logs are labeled as a legacy stream.

## Snapshots

Pause a managed jailed VM to save a snapshot. The snapshot library stores a
`.fmsnap` bundle with Firecracker state, memory, kernel and independent disk copies.
Its source VM name is informational. Deleting the source VM keeps its snapshots
unless the delete dialog's snapshot option is selected.

Upload, download and delete bundles from Snapshots. Bundles can contain guest data
and resolved credentials. Only trust an uploaded bundle when its producer and
contents are trusted; Firecracker snapshot state is executable VM state.

Restore replaces a stopped target VM's managed disks and leaves it paused. Resume
continues the saved guest. The target must have compatible CPU count, memory,
architecture, Firecracker version, drive IDs and guest network configuration.
The target network may have a different catalog name, but its subnet, gateway and
policy must match, along with the saved guest IP and MAC.
Firecracker also checks host CPU compatibility. A different VM name is allowed;
a saved network address cannot be reused while another definition reserves it.
Target host isolation and egress policy remain authoritative.

Snapshot storage defaults to `snapshots` under `data_dir`. `snapshot_dir` overrides
it, and `snapshot_max_bytes` limits both compressed and expanded bundle size,
defaulting to 64 GiB. File uploads use `asset_max_bytes`, defaulting to 1 GiB.
Restart after changing these host settings, and match the reverse proxy's upload
limits. Restore needs temporary space for extraction and the target's previous
disks until it succeeds.
