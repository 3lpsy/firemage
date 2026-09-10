"""Preserve guest egress policy across daemon recovery and managed snapshots."""
from .harness import wait_for


def recovery(harness, target, gateway, guest, network, policy, plain, tls, host_port):
    plain_port, tls_port = plain.server_address[1], tls.server_address[1]
    tls.stage = "initial"
    script = guest_script(target, gateway, plain_port, tls_port, host_port)
    vm = harness.define("egress-recovery", script, network={
        "network": network, "address": guest, "mac": "02:fc:00:00:00:06",
    }, egress=policy)
    harness.action(vm, "start")
    wait_for("initial guest egress", lambda: "FIREMAGE_EGRESS_INITIAL_OK" in harness.console(vm))
    initial = harness.request("GET", f"/v1/vms/{vm}/egress")
    assert initial["active"] and initial["ca_fingerprint"], initial

    harness.stop_server()
    harness.start_server()
    harness.state(vm, "running", timeout=20)
    recovered = harness.request("GET", f"/v1/vms/{vm}/egress")
    assert recovered["active"] and recovered["ca_fingerprint"] == initial["ca_fingerprint"], recovered
    tls.stage = "restarted"
    wait_for("guest egress after daemon restart", lambda: "FIREMAGE_EGRESS_RESTART_OK" in harness.console(vm))

    harness.action(vm, "pause")
    snapshots = harness.data / "vms" / vm / "snapshots"
    snapshots.mkdir(exist_ok=True)
    state, memory = snapshots / "egress.vmstate", snapshots / "egress.memory"
    action = {"snapshot_path": str(state), "memory_path": str(memory)}
    assert harness.request("POST", f"/v1/vms/{vm}/actions", {"action": "snapshot", **action})["state"] == "paused"
    assert state.is_file() and memory.is_file()
    harness.action(vm, "stop")
    assert not harness.request("GET", f"/v1/vms/{vm}/egress")["active"]
    assert harness.request("POST", f"/v1/vms/{vm}/actions", {"action": "restore", **action})["state"] == "paused"
    restored = harness.request("GET", f"/v1/vms/{vm}/egress")
    assert restored["active"] and restored["ca_fingerprint"] == initial["ca_fingerprint"], restored
    harness.action(vm, "resume")
    tls.stage = "restored"
    harness.state(vm, "stopped", timeout=120)
    assert harness.output(vm, "exit-code") == b"0\n", harness.console(vm)
    assert harness.output(vm, "result") == b"egress-recovery-ok\n"
    for phase in ("initial", "restart", "restore"):
        assert f"/allowed/{phase}" in plain.paths, plain.paths
        assert f"/allowed/{phase}" in tls.paths, tls.paths
    print("PASS egress recovery: daemon restart, stable CA, managed snapshot stop/restore, HTTP/TLS allow and deny", flush=True)


def guest_script(target, gateway, plain_port, tls_port, host_port):
    return f"""set -eu
wait_stage() {{
    for attempt in $(seq 1 90); do
        stage="$(curl -fsS --max-time 3 https://{target}:{tls_port}/allowed/stage 2>/dev/null || true)"
        [ "$stage" = "$1" ] && return 0
        sleep 1
    done
    echo 'egress recovery stage timed out' >&2
    return 1
}}
probe() {{
    curl -fsS --max-time 10 http://{target}:{plain_port}/allowed/$1 > /firemage/output/http
    curl -fsS --max-time 10 https://{target}:{tls_port}/allowed/$1 > /firemage/output/https
    grep -qx 'egress-origin-ok' /firemage/output/http
    grep -qx 'egress-origin-ok' /firemage/output/https
    [ "$(curl -sS --max-time 10 -o /dev/null -w '%{{http_code}}' https://{target}:{tls_port}/denied)" = 403 ]
    [ "$(curl -sS --max-time 10 -X POST -o /dev/null -w '%{{http_code}}' https://{target}:{tls_port}/allowed)" = 403 ]
    if curl -sS --max-time 3 --noproxy '*' http://{gateway}:{host_port}/; then exit 1; fi
}}
probe initial
echo FIREMAGE_EGRESS_INITIAL_OK
wait_stage restarted
probe restart
echo FIREMAGE_EGRESS_RESTART_OK
wait_stage restored
probe restore
printf 'egress-recovery-ok\\n' > /firemage/output/result
"""
