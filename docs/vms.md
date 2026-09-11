# Virtual machines

Create and Edit use the same page. Expand a section to configure its settings;
Apply in a section editor updates the draft. Create VM saves the full definition.
It does not start a guest. Start prepares missing resources and boots the VM.
Prepare creates the resources and leaves Firecracker ready without booting.
Stop a prepared or running VM before editing its general configuration. Egress
policy assignment and shared policy/proxy edits also apply to running VMs.

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

Configuration exports use kernel, file and egress policy aliases. Import resolves those aliases
and secret/network names in the destination account. Local host paths and external
Firecracker sockets are not portable and cannot be exported. Import assigns fresh
network addresses for a new VM. Applying configuration to an existing VM uses the
addresses in the imported document and validates them against reserved addresses.

## Shared egress

Egress below Networks contains policies and upstream proxies. Create a policy on
its full page, using the section index and collapsible sections for HTTP rules,
signing, tunnels, TLS and upstream routing. Select that policy in Create/Edit VM;
the plus action opens its creation page and preserves the VM draft on return.
The VM must use a Firemage-only network. No policy means no outbound access.

Each policy can select a named upstream proxy, use the host default, or connect
directly from Firemage. Proxy passwords and private CA bundles reference Secrets.
Policy and proxy detail panes list associated VMs. Deletion is blocked while any
VM references a policy or any policy references a proxy, including inactive VMs.

Saving shared rules or an upstream proxy updates all affected running VMs. Existing
connections close so revoked rules cannot remain active; applications must retry.
Changing a VM's selected policy affects only that VM. Other VM settings still
require a stopped VM. Failed updates restore previous access where possible and
otherwise leave the affected network blocked for repair and retry.

Guest HTTP proxy addresses stay stable while host rules and upstream routing
change. Newly prepared Firemage-only guests receive proxy bootstrap even without
an initial policy. Older guests that never had HTTP egress need stop and prepare
once before enabling it. Non-OCI guests must follow the seed convention to use the
provided proxy environment and CA. Existing inline policies migrate separately;
VMs that previously had independent configurations are not merged automatically.

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

## Web Shell

Enable **Web Terminal** in the create/edit form's Terminal section. The Web Shell
view opens a separate interactive guest shell; guest serial input still controls
ttyS0. Shell access requires an administrator browser session. The default command
is `["/bin/sh", "-i"]`; configure another absolute guest executable and its arguments
when needed. Leaving the tab terminates the shell and its processes. Reconnecting
starts a new shell. The main workload keeps its existing lifecycle; a one-shot VM
still stops when its main program exits.

Firemage adds its embedded static `firemage-guest` executable to the input disk
only when enabled. Managed OCI initialization starts it over a private vsock
connection before userdata. Existing OCI disks with Firemage's generated init
use the same seed setup without replacing their root disk. The guest kernel must be Linux 5.3 or newer and
support virtio-vsock and PTYs, and the configured shell must exist in the image.

Custom root disks must mount the seed disk at `/firemage/input`, mount devpts,
and start `/firemage/input/firemage/firemage-guest` from their init. They may run
Firemage's seed setup script as part of their normal initialization. Firemage
does not replace a custom image's init. Releases also provide the static guest
binary for custom image builds. The optional server setting
`firemage_guest_bin_path` selects an alternative compatible static executable
for injection; the embedded helper remains the default.

When Firemage sits behind a reverse proxy, forward WebSocket upgrades on
`/v1/vms/{id}/shell/ws`. For nginx, add a location such as this alongside your
existing API route (adjust the upstream to match your deployment):

```nginx
location ~ ^/v1/vms/[^/]+/shell/ws$ {
    proxy_http_version 1.1;
    proxy_set_header Host $http_host;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 12h;
    proxy_pass http://127.0.0.1:8080;
}
```
