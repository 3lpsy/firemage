"""Preserve guest egress policy across daemon recovery and managed snapshots."""
import hashlib
import json
import urllib.request
import urllib.error

from .harness import wait_for


def recovery(harness, target, gateway, network, policy, plain, tls, host_port):
    plain_port, tls_port = plain.server_address[1], tls.server_address[1]
    definition = next(item for item in harness.request("GET", "/v1/networks") if item["name"] == network)
    definition.pop("id", None)
    network = "snapshot-source-network"
    definition["name"] = network
    harness.request("POST", "/v1/networks", definition)
    harness.networks.append(network)
    tls.stage = "initial"
    script = guest_script(target, gateway, plain_port, tls_port, host_port)
    attachment = harness.request("GET", f"/v1/networks/{network}/suggestion")
    vm = harness.define("egress-recovery", script, network=attachment, egress=policy)
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
    seed = harness.data / "vms" / vm / "seed.ext4"
    with seed.open("rb") as source:
        seed_digest = hashlib.file_digest(source, "sha256").digest()
    saved = harness.request("POST", f"/v1/vms/{vm}/snapshots", {"alias": "egress-checkpoint"})
    assert saved["source_vm_id"] == vm and saved["trusted"], saved
    spec = harness.request("GET", f"/v1/vms/{vm}")["spec"]
    harness.action(vm, "stop")
    harness.request("DELETE", f"/v1/vms/{vm}")
    harness.request("DELETE", f"/v1/networks/{network}")
    assert any(item["id"] == saved["id"] for item in harness.request("GET", "/v1/snapshots"))
    assert not (harness.data / "vms" / vm).exists(), "VM deletion retained managed disks"
    request = urllib.request.Request(f"{harness.base}/v1/snapshots/{saved['id']}/download",
                                     headers={"Authorization": f"Bearer {harness.token}"})
    with harness.opener.open(request, timeout=120) as response:
        bundle = response.read()
    assert len(bundle) == saved["size_bytes"]
    request = urllib.request.Request(f"{harness.base}/v1/snapshots?alias=imported-egress", method="POST", data=bundle,
                                     headers={"Authorization": f"Bearer {harness.token}", "Content-Type": "application/octet-stream"})
    with harness.opener.open(request, timeout=120) as response:
        uploaded = json.load(response)
    assert uploaded["source_vm_id"] is None and not uploaded["trusted"]
    spec["name"] = "restored-egress-copy"
    definition["name"] = "snapshot-target-network"
    target_network = harness.request("POST", "/v1/networks", definition)
    harness.networks.append(definition["name"])
    spec["network"]["network"] = definition["name"]
    vm = harness.request("POST", "/v1/vms", spec)["id"]
    harness.vms.append(vm)
    try:
        harness.request("POST", f"/v1/vms/{vm}/snapshots/restore", {"snapshot_id": uploaded["id"]})
        raise AssertionError("untrusted snapshot restored")
    except AssertionError as error:
        assert "must be trusted" in str(error), error
    harness.request("POST", f"/v1/snapshots/{uploaded['id']}/trust")
    restored_vm = harness.request("POST", f"/v1/vms/{vm}/snapshots/restore", {"snapshot_id": uploaded["id"]})
    assert restored_vm["state"] == "paused" and restored_vm["spec"]["network"]["network"] == target_network["id"], restored_vm
    seed = harness.data / "vms" / vm / "seed.ext4"
    harness.request("DELETE", f"/v1/snapshots/{saved['id']}")
    harness.request("DELETE", f"/v1/snapshots/{uploaded['id']}")
    with seed.open("rb") as source:
        assert hashlib.file_digest(source, "sha256").digest() == seed_digest, "restore changed the guest's mounted seed disk"
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
    print("PASS egress recovery: daemon restart, stable CA, unchanged seed disk, snapshot upload/download, source VM/network deletion, renamed target network, cross-VM restore, independent disks, HTTP/TLS allow and deny", flush=True)


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
