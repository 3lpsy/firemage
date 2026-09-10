#!/usr/bin/env python3
"""Run real Firecracker guests through a released Firemage HTTPS server."""
import signal

from vm.cases import host_only, lifecycle, offline
from vm.harness import Harness
from vm.egress import isolated_egress
from vm.security import isolated_processes
from vm.oci import private_oci


def interrupted(number, _frame):
    raise SystemExit(128 + number)


def main():
    signal.signal(signal.SIGTERM, interrupted)
    with Harness() as harness:
        isolated_processes(harness)
        offline(harness)
        private_oci(harness)
        lifecycle(harness)
        host_only(harness)
        isolated_egress(harness)
    print("Real Firecracker VM acceptance passed", flush=True)


if __name__ == "__main__":
    main()
