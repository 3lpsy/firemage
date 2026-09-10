"""Refresh a private guest fixture without mounting its filesystem on the host."""
from pathlib import Path
import re
import shutil
import subprocess


def prepare(source, destination):
    shutil.copyfile(source, destination)
    with destination.open("r+b") as disk:
        disk.truncate(128 * 1024 * 1024)
    check = subprocess.run(["e2fsck", "-f", "-y", str(destination)], capture_output=True, timeout=60)
    assert check.returncode in (0, 1), "fixture filesystem check failed"
    subprocess.run(["resize2fs", str(destination)], check=True, capture_output=True, timeout=60)
    curl = Path(shutil.which("curl") or "/usr/bin/curl").resolve()
    assert curl.is_file(), "curl is required for TLS interception acceptance"
    output = subprocess.run(["ldd", str(curl)], check=True, text=True, capture_output=True, timeout=10).stdout
    paths = {Path(match) for match in re.findall(r"(/[^\s()]+)", output)}
    copies = [(curl, Path("/usr/bin/curl")), (Path(__file__).with_name("init.sh"), Path("/init"))]
    copies.extend((path.resolve(), path) for path in sorted(paths))
    directories = set()
    for _, target in copies:
        directories.update(parent for parent in target.parents if parent != Path("/"))
    commands = [f"mkdir {path}" for path in sorted(directories, key=lambda p: len(p.parts))]
    for source_path, target in copies:
        assert not any(ch in str(source_path) for ch in '\n\r" '), "unsafe fixture source path"
        commands.extend([f"rm {target}", f"write {source_path} {target}", f"set_inode_field {target} mode 0100755"])
    batch = destination.with_suffix(".debugfs")
    batch.write_text("\n".join(commands) + "\n")
    subprocess.run(["debugfs", "-w", "-f", str(batch), str(destination)], check=True, capture_output=True, timeout=60)
    for target in ("/init", "/usr/bin/curl"):
        result = subprocess.run(["debugfs", "-R", f"stat {target}", str(destination)], check=True, capture_output=True, text=True, timeout=10)
        assert "Inode:" in result.stdout and "0755" in result.stdout, f"fixture file missing: {target}"
