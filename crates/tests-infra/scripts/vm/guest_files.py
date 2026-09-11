"""Browse a stopped guest by inode and download its binary output."""
import urllib.request


def verify(harness, vm, payload):
    inode = 2
    for name in ["firemage", "output"]:
        directory = harness.request("GET", f"/v1/vms/{vm}/directory?inode={inode}")
        entry = next(item for item in directory["entries"] if item["name"] == name)
        assert entry["kind"] == "directory", entry
        inode = entry["inode"]
    directory = harness.request("GET", f"/v1/vms/{vm}/directory?inode={inode}")
    entry = next(item for item in directory["entries"] if item["name"] == "payload.bin")
    assert entry["kind"] == "file" and entry["size_bytes"] == len(payload), entry
    request = urllib.request.Request(
        f"{harness.base}/v1/vms/{vm}/files/download?inode={entry['inode']}&filename=payload.bin",
        headers={"Authorization": f"Bearer {harness.token}"},
    )
    with harness.opener.open(request, timeout=60) as response:
        assert response.headers["Content-Type"] == "application/octet-stream"
        assert "attachment;" in response.headers["Content-Disposition"]
        assert response.read() == payload
