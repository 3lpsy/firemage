"""Inspect actual jailed VMM identities and kernel-enforced process limits."""
import grp
import json
from pathlib import Path
import pwd
import socket
import struct


def unused_identity_range(count=1024):
    occupied = [(entry.pw_uid, entry.pw_uid + 1) for entry in pwd.getpwall()]
    occupied += [(entry.gr_gid, entry.gr_gid + 1) for entry in grp.getgrall()]
    for source in ("/etc/subuid", "/etc/subgid"):
        if Path(source).exists():
            for line in Path(source).read_text().splitlines():
                if not line or line.startswith("#"):
                    continue
                _, start, length = line.rsplit(":", 2)
                occupied.append((int(start), int(start) + int(length)))
    for base in range(1_000_000, 2_000_000_000, count):
        if all(base + count <= start or base >= end for start, end in occupied):
            return base, count
    raise AssertionError("No unused CI jail identity range")


def socket_process(path):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
        stream.settimeout(5)
        stream.connect(str(path))
        return struct.unpack("3i", stream.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))[0]


def isolated_processes(harness):
    vms = [harness.define(f"jail-boundary-{index}", "sleep 300\n") for index in range(2)]
    try:
        for vm in vms:
            assert harness.action(vm, "prepare")["state"] == "ready"
            assert harness.action(vm, "start")["state"] == "running"
        processes = [socket_process(path) for path in (harness.data / "sockets").glob("*.sock")
                     if path.exists()]
        assert len(processes) == 2, processes
        evidence = []
        for pid in processes:
            process = Path(f"/proc/{pid}")
            status = dict(line.split(":", 1) for line in (process / "status").read_text().splitlines())
            uid = int(status["Uid"].split()[0])
            gid = int(status["Gid"].split()[0])
            assert harness.uid_base <= uid < harness.uid_base + harness.uid_count, uid
            assert gid == uid and int(status["CapEff"], 16) == 0, status
            for thread in (process / "task").iterdir():
                thread_status = dict(line.split(":", 1) for line in (thread / "status").read_text().splitlines())
                assert thread_status["NoNewPrivs"].strip() == "1", thread_status
                assert thread_status["Seccomp"].strip() == "2", thread_status
            roots = [harness.data / "jailer/firecracker" / vm / "root" for vm in vms]
            root = next((expected for expected in roots if (process / "root").samefile(expected)), None)
            assert root is not None, (process / "root").readlink()
            assert not (process / "root/etc/shadow").exists(), root
            assert not (process / "root/var/lib/firemage").exists(), root
            path = (process / "cgroup").read_text().strip().removeprefix("0::")
            assert path.startswith(f"/{harness.cgroup_parent}/"), path
            group = Path("/sys/fs/cgroup") / path.lstrip("/")
            limits = {name: (group / name).read_text().strip()
                      for name in ("memory.max", "memory.swap.max", "pids.max", "cpu.max")}
            assert int(limits["memory.max"]) == (128 + 256) * 1024 * 1024, limits
            assert limits["memory.swap.max"] == "0" and limits["pids.max"] == "128", limits
            quota, period = map(int, limits["cpu.max"].split())
            assert quota == period, limits
            evidence.append({"pid": pid, "uid": uid, "gid": gid, "root": str(root), "limits": limits})
        assert evidence[0]["uid"] != evidence[1]["uid"], evidence
        (harness.results / "jail-isolation.json").write_text(json.dumps(evidence, indent=2))
        for vm in vms:
            try:
                harness.request("POST", f"/v1/vms/{vm}/firecracker", {
                    "method": "PUT", "path": "/drives/escape",
                    "body": {"drive_id": "escape", "path_on_host": "/etc/shadow",
                             "is_root_device": False, "is_read_only": True},
                })
            except AssertionError as error:
                assert "HTTP 4" in str(error), str(error)
            else:
                raise AssertionError("Jailed VM accepted a raw host disk attachment")
        print("PASS jailed processes: distinct non-root identities, chroot, seccomp, cgroup limits, raw path denial", flush=True)
    finally:
        for vm in vms:
            harness.action(vm, "stop")


def cleanup_cgroup(parent):
    directory = Path("/sys/fs/cgroup") / parent
    if directory.exists():
        for child in directory.iterdir():
            if child.is_dir():
                if (child / "cgroup.kill").exists():
                    (child / "cgroup.kill").write_text("1")
                try:
                    child.rmdir()
                except OSError:
                    pass
        try:
            directory.rmdir()
        except OSError:
            pass
