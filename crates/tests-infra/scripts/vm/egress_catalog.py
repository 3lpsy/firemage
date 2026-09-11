"""Shared catalog changes reach two running guests and revoke existing connections."""
import copy
import http.server
import json
import subprocess
import threading

from .harness import wait_for


class HeldTunnels:
    def __init__(self):
        self.lock = threading.Lock()
        self.connections = {}

    def started(self, name, size):
        with self.lock:
            row = self.connections.setdefault(name, {"opened": 0, "closed": 0, "bytes": 0})
            row["opened"] += 1
            row["bytes"] += size

    def received(self, name, size):
        with self.lock:
            self.connections[name]["bytes"] += size

    def closed(self, name):
        with self.lock:
            self.connections[name]["closed"] += 1

    def snapshot(self):
        with self.lock:
            return copy.deepcopy(self.connections)

    def is_closed(self):
        rows = self.snapshot()
        return set(rows) == {"held-one", "held-two"} and all(
            row["opened"] == 1 and row["closed"] == 1 for row in rows.values())


def reject(harness, method, path, body=None, status=409):
    try:
        harness.request(method, path, body)
    except AssertionError as error:
        assert f"HTTP {status}:" in str(error), error
        return
    raise AssertionError(f"{method} {path} unexpectedly succeeded")


def shared_catalog(harness, target, gateway, network, policy, plain, upstream, echo, host_port):
    tracker = HeldTunnels()
    echo.catalog_tracker = tracker
    secondary = http.server.ThreadingHTTPServer(("127.0.0.1", 0), upstream.RequestHandlerClass)
    secondary.targets, secondary.seen = upstream.targets.copy(), []
    thread = threading.Thread(target=secondary.serve_forever, daemon=True)
    thread.start()
    try:
        exercise(harness, target, gateway, network, policy, plain, upstream, secondary, tracker, host_port)
    finally:
        del echo.catalog_tracker
        secondary.shutdown()
        secondary.server_close()
        thread.join(timeout=5)


def exercise(harness, target, gateway, network, inline, plain, upstream, secondary, tracker, host_port):
    proxy = harness.request("POST", "/v1/egress/proxies", {
        "alias": "shared-acceptance-upstream", "proxy": inline["upstream"],
    })
    definition = copy.deepcopy(inline)
    definition.pop("upstream")
    definition["inherit_upstream"] = False
    policy = harness.request("POST", "/v1/egress/policies", {
        "alias": "shared-acceptance-policy", "policy": definition,
        "upstream_proxy_id": proxy["id"],
    })
    policy_path = f"/v1/egress/policies/{policy['id']}"
    proxy_path = f"/v1/egress/proxies/{proxy['id']}"
    plain.stage = "initial"
    vms = []
    for name in ("one", "two"):
        attachment = harness.request("GET", f"/v1/networks/{network}/suggestion")
        vms.append(harness.define(f"shared-egress-{name}", guest_script(target, gateway,
            plain.server_address[1], name), network=attachment, egress_policy=policy["id"]))
    for path in (policy_path, proxy_path):
        reject(harness, "DELETE", path)
    for vm in vms:
        harness.action(vm, "start")
    phase(harness, vms, "INITIAL")
    detail = harness.request("GET", policy_path)
    assert {vm["id"] for vm in detail["vms"]} == set(vms), detail
    assert detail["vm_count"] == 2, detail
    detail = harness.request("GET", proxy_path)
    assert detail["vm_count"] == 2 and detail["policy_count"] == 1, detail
    for vm in vms:
        assert harness.request("GET", f"/v1/vms/{vm}/egress")["active"]
    reject(harness, "DELETE", policy_path)
    reject(harness, "DELETE", proxy_path)

    denied = copy.deepcopy(definition)
    denied["http"]["rules"] = []
    denied["tunnels"] = []
    held = tracker.snapshot()
    assert set(held) == {"held-one", "held-two"}, held
    assert all(row["opened"] == 1 and row["closed"] == 0 and row["bytes"] > 0 for row in held.values()), held
    policy = harness.request("PUT", policy_path, policy_input(policy, denied))
    # Observe EOF at the controlled origin before the upstream's 20-second idle timeout.
    try:
        wait_for("both held tunnel origin sockets revoked", tracker.is_closed, timeout=10)
    except AssertionError as error:
        raise AssertionError(f"{error}; observed {json.dumps(tracker.snapshot())}") from error
    revoked = tracker.snapshot()
    phase(harness, vms, "DENIED")
    print(f"Revoked held tunnel origin sockets: {json.dumps(revoked)}", flush=True)
    assert all(harness.request("GET", f"/v1/vms/{vm}")["state"] == "running" for vm in vms)
    plain.stage = "restored"
    policy = harness.request("PUT", policy_path, policy_input(policy, definition))
    phase(harness, vms, "RESTORED")
    assert tracker.snapshot() == revoked, "revoked tunnel delivered more bytes after policy restoration"

    # A failed port change must preserve the revision and restore guest access.
    conflict = copy.deepcopy(definition)
    conflict["tunnels"].append({"name": "occupied", "listen_port": host_port,
                              "target_host": target, "target_port": inline["tunnels"][0]["target_port"]})
    reject(harness, "PUT", policy_path, policy_input(policy, conflict), status=400)
    assert harness.request("GET", policy_path)["revision"] == policy["revision"]
    plain.stage = "after-conflict"
    phase(harness, vms, "AFTER_CONFLICT")

    updated_proxy = copy.deepcopy(proxy["proxy"])
    updated_proxy["url"] = f"http://127.0.0.1:{secondary.server_address[1]}"
    proxy = harness.request("PUT", proxy_path, {
        "alias": proxy["alias"], "revision": proxy["revision"], "proxy": updated_proxy,
        "ca_secret": proxy.get("ca_secret"),
    })
    old_seen = len(upstream.seen)
    plain.stage = "proxy-updated"
    phase(harness, vms, "PROXY_UPDATED")
    assert tracker.snapshot() == revoked, "revoked tunnel delivered more bytes after upstream replacement"
    assert len(secondary.seen) >= 2, secondary.seen
    assert len(upstream.seen) == old_seen, "guests kept using the old upstream after the update"
    for vm in vms:
        harness.action(vm, "pause")
    reject(harness, "DELETE", policy_path)
    reject(harness, "DELETE", proxy_path)
    plain.stage = "finish"
    for vm in vms:
        harness.action(vm, "resume")
        harness.state(vm, "stopped", timeout=60)
        assert harness.output(vm, "exit-code") == b"0\n", harness.console(vm)
        assert harness.output(vm, "result") == b"shared-egress-ok\n", harness.console(vm)
    reject(harness, "DELETE", policy_path)
    reject(harness, "DELETE", proxy_path)
    assert tracker.snapshot() == revoked, "revoked tunnel changed before guest shutdown"
    for vm in vms:
        harness.request("DELETE", f"/v1/vms/{vm}")
    harness.request("DELETE", policy_path)
    harness.request("DELETE", proxy_path)
    print("PASS shared egress catalogs: two running guests, prebind rollback, live deny/restore, old tunnel revocation, live upstream replacement, reference-safe deletion in defined/running/paused/stopped states", flush=True)


def policy_input(row, definition):
    return {"alias": row["alias"], "revision": row["revision"], "policy": definition,
            "upstream_proxy_id": row["upstream_proxy_id"]}


def phase(harness, vms, marker):
    try:
        for vm in vms:
            wait_for(f"{vm} catalog phase {marker}",
                     lambda: reached_phase(harness, vm, marker), timeout=60)
    except AssertionError:
        for vm in vms:
            try:
                diagnostics(harness, vm, marker)
            except Exception as error:
                print(f"Catalog diagnostics unavailable for {vm}: {error}", flush=True)
        raise


def diagnostics(harness, vm, marker):
    sections = [f"catalog phase {marker}; VM {vm}"]
    interface = "fm" + vm.replace("-", "")[:10]
    commands = [["ip", "-details", "address", "show", "dev", interface],
                ["ip", "neigh", "show", "dev", interface],
                ["nft", "list", "table", "netdev", interface],
                ["nft", "list", "table", "netdev", interface + "pending"]]
    try:
        row = harness.request("GET", f"/v1/vms/{vm}")
        spec = row["spec"]
        sections.append(json.dumps({"state": row["state"], "error": row.get("error"),
            "network": spec.get("network"), "egress_policy": spec.get("egress_policy"),
            "egress_http_port": spec.get("egress_http_port")}))
        if spec.get("network"):
            commands.append(["ip", "route", "get", spec["network"]["address"]])
    except Exception as error:
        sections.append(f"VM status unavailable: {error}")
    for command in commands:
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=5, check=False)
            sections.append(f"{' '.join(command)}: exit {result.returncode}\n{result.stdout[:16384]}{result.stderr[:4096]}")
        except (OSError, subprocess.SubprocessError) as error:
            sections.append(f"{' '.join(command)}: {error}")
    sections.append(harness.console(vm)[-8192:])
    output = "\n".join(sections)
    path = harness.results / f"vm-{vm}-egress-diagnostics.log"
    path.write_text(output)
    path.chmod(0o644)
    print(output, flush=True)


def reached_phase(harness, vm, marker):
    console = harness.console(vm)
    assert "FIREMAGE_CATALOG_TUNNEL_TIMEOUT" not in console, console
    return f"FIREMAGE_CATALOG_{marker}" in console


def guest_script(target, gateway, port, name):
    return f"""set -eu
(
    set +e
    (while printf 'held-{name}\\n'; do sleep 1; done) | timeout 300 nc {gateway} 19090 > /firemage/output/held-tunnel
    status=$?
    case "$status" in
        124|137|142|143) echo FIREMAGE_CATALOG_TUNNEL_TIMEOUT ;;
        *) echo FIREMAGE_CATALOG_TUNNEL_CLOSED ;;
    esac
) &
for attempt in $(seq 1 50); do
    grep -qx 'held-{name}' /firemage/output/held-tunnel && break
    sleep 0.2
done
grep -qx 'held-{name}' /firemage/output/held-tunnel
seen=''
last_code=''
for attempt in $(seq 1 300); do
    code="$(curl -sS --max-time 3 -o /firemage/output/stage -w '%{{http_code}}' http://{target}:{port}/allowed/stage 2>/firemage/output/curl-error || true)"
    if [ "$code" != "$last_code" ]; then
        echo "FIREMAGE_CATALOG_HTTP code=$code attempt=$attempt"
        if [ "$code" != 200 ] && [ "$code" != 403 ]; then
            cat /firemage/output/curl-error
            ip address show dev eth0 || true
            ip route || true
            ip neigh || true
        fi
        last_code="$code"
    fi
    if [ $((attempt % 10)) -eq 0 ]; then echo "FIREMAGE_CATALOG_POLL code=$code attempt=$attempt"; fi
    if [ "$code" = 403 ]; then
        if [ "$seen" != denied ]; then
            echo FIREMAGE_CATALOG_DENIED
            cat /proc/net/tcp
            seen=denied
        fi
    elif [ "$code" = 200 ]; then
        stage="$(cat /firemage/output/stage)"
        if [ "$stage" != "$seen" ]; then
            case "$stage" in
                initial) echo FIREMAGE_CATALOG_INITIAL ;;
                after-conflict) echo FIREMAGE_CATALOG_AFTER_CONFLICT ;;
                restored) echo FIREMAGE_CATALOG_RESTORED ;;
                proxy-updated) echo FIREMAGE_CATALOG_PROXY_UPDATED ;;
                finish)
                    printf 'shared-egress-ok\\n' > /firemage/output/result
                    exit 0 ;;
                *) echo "unexpected catalog stage: $stage" >&2; exit 1 ;;
            esac
            seen="$stage"
        fi
    fi
    sleep 1
done
echo 'catalog guest stages timed out' >&2
exit 1
"""
