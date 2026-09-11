"""Real managed OCI guest shell sessions through browser authentication and vsock."""
from .harness import wait_for
from .websocket import WebSocket


def verify(harness, rootfs):
    script = """set -eu
[ -x /firemage/input/firemage/firemage-guest ]
/firemage/input/firemage/firemage-guest --version
printf 'FIREMAGE_WEB_SHELL_MAIN_READY\\n'
while [ ! -f /firemage/output/finish ]; do sleep 1; done
printf 'main-complete\\n' > /firemage/output/result
exit 7
"""
    vm = harness.define("oci-web-shell", script, rootfs=rootfs, web_terminal={}, terminal=True,
        boot_args="console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw",
        workload={"mode": "one-shot"})
    harness.action(vm, "start")
    wait_for("Web Shell main workload", lambda: "FIREMAGE_WEB_SHELL_MAIN_READY" in harness.console(vm))
    assert harness.request("GET", f"/v1/vms/{vm}/terminal")["state"] == "available"

    failures = []

    def connect():
        try:
            return WebSocket(harness, vm)
        except (AssertionError, OSError) as error:
            failures[:] = [str(error)]
            return None

    try:
        shell = wait_for("guest vsock shell", connect, timeout=30)
    except AssertionError as error:
        raise AssertionError(f"{error}; last connection: {failures}; serial: {harness.console(vm)}") from error
    try:
        shell.input("stty -echo; printf 'shell-%s\\n' ready\n")
        shell.until("shell-ready")
        shell.send({"type": "resize", "rows": 37, "cols": 91})
        shell.input("test -t 0 && test -t 1 && test -t 2 && printf 'pty-%s\\n' ok; "
                    "printf 'stdout-%s\\n' ok; printf 'stderr-%s\\n' ok >&2; stty size; "
                    "echo $$ > /firemage/output/shell.pid; sleep 300 & "
                    "echo $! > /firemage/output/shell-job.pid; printf 'checks-%s\\n' done\n")
        output = shell.until("checks-done")
        for expected in ("pty-ok", "stdout-ok", "stderr-ok", "37 91"):
            assert expected in output, (expected, output)
    finally:
        shell.close()
    assert harness.request("GET", f"/v1/vms/{vm}")["state"] == "running", "closing shell stopped main workload"
    replacement = wait_for("replacement shell", connect, timeout=15)
    try:
        replacement.input("stty -echo; printf 'replacement-%s\\n' ready\n")
        replacement.until("replacement-ready")
        replacement.input("for n in shell shell-job; do p=$(cat /firemage/output/$n.pid); "
                          "i=0; while [ -r /proc/$p/stat ]; do "
                          "s=$(cut -d ' ' -f 3 /proc/$p/stat); [ \"$s\" = Z ] && break; "
                          "i=$((i+1)); [ $i -lt 5 ] || break; sleep 1; done; "
                          "if [ ! -r /proc/$p/stat ] || [ \"$(cut -d ' ' -f 3 /proc/$p/stat)\" = Z ]; "
                          "then printf 'clean-%s\\n' $n; else printf 'live-%s\\n' $n; fi; done; "
                          "printf 'cleanup-%s\\n' done\n")
        output = replacement.until("cleanup-done")
        assert "clean-shell" in output and "clean-shell-job" in output, output
    finally:
        replacement.close()
    harness.stop_server()
    harness.start_server()
    harness.state(vm, "running", timeout=20)
    recovered = wait_for("shell after daemon restart", connect, timeout=15)
    try:
        recovered.input("printf 'recovery-%s\\n' ok\n")
        recovered.until("recovery-ok")
    finally:
        recovered.close()
    harness.action(vm, "pause")
    saved = harness.request("POST", f"/v1/vms/{vm}/snapshots", {"alias": "web-shell-checkpoint"})
    assert saved["source_vm_id"] == vm and saved["trusted"], saved
    harness.action(vm, "stop")
    restored = harness.request("POST", f"/v1/vms/{vm}/snapshots/restore", {"snapshot_id": saved["id"]})
    assert restored["state"] == "paused", restored
    harness.action(vm, "resume")
    final = wait_for("shell after snapshot restore", connect, timeout=15)
    try:
        final.input("printf 'restore-%s\\n' ok\n")
        final.until("restore-ok")
        final.input("touch /firemage/output/finish\n")
        harness.state(vm, "stopped")
    finally:
        final.close()
        harness.request("DELETE", f"/v1/snapshots/{saved['id']}")
    assert harness.output(vm, "exit-code") == b"7\n", harness.console(vm)
    assert harness.output(vm, "result") == b"main-complete\n"
    completed = harness.console(vm).count("FIREMAGE_WEB_SHELL_MAIN_READY")
    harness.action(vm, "start")
    harness.state(vm, "stopped")
    assert harness.console(vm).count("FIREMAGE_WEB_SHELL_MAIN_READY") == completed + 1, "Web Shell enabled one-shot VM did not restart"
    assert harness.output(vm, "exit-code") == b"7\n", harness.console(vm)
    assert "stdout-ok" not in harness.console(vm), "Web Shell output leaked into serial"
    print("PASS Web Shell: opt-in static helper, browser session, vsock PTY, input/output/resize, disconnect cleanup, serial coexistence, daemon recovery, snapshot restore, one-shot main exit and restart", flush=True)
