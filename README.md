# Firemage

**Status**: Work In Progress / Proof of Concept

Firemage manages Firecracker microVMs through one binary, an HTTP JSON API, a CLI, and a desktop web UI.

- Launch OCI images or local and remote VM assets with TOML configuration.
- Control VM lifecycle, snapshots, boot files, environment, and userdata.
- Restrict guest networking with HTTP/TLS policies, credential injection, and TCP tunnels.
- Use jailed host isolation by default, with explicit opt-in for trusted workloads.

Linux with KVM is required to run VMs. Jailed operation also requires the Firecracker jailer, cgroup v2, and a reserved host UID/GID range.

To build, copy `.env.example` to `.env` and set `FIREMAGE_DEPENDENCY_INDEX` to your Cargo dependency proxy's HTTPS sparse index. Install `just` and Docker or Podman, then run:

```sh
just docker-build
just docker-verify
just docker-release
```

Builds reuse `target/podman` and copy the binary to `dist/firemage`. Release builds embed the web UI. Public source snapshots omit private deployment workflows and their checks.

**Security Warning**: This application has not undergone a third party security review. It is not recommended to deploy the server on adversarial or public networks.
