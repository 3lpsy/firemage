const vmId = arguments[0];
const original = window.fetch;
const fixture = window.__guestFiles = { original, stopped: false, reads: [] };
const entry = (inode, name, kind, size_bytes = null) => ({
    inode, name, kind, size_bytes, uid: 0, gid: 0, mode: kind === "directory" ? 493 : 420
});
const directories = {
    2: [entry(10, "review files", "directory")],
    10: [entry(20, "nested", "directory")],
    20: [entry(31, "résumé notes.txt", "file", 12), entry(32, "large.bin", "file", 2048), entry(33, "link", "symlink")]
};
window.fetch = async function(input, options) {
    const url = new URL(typeof input === "string" ? input : input.url, location.href);
    if (url.pathname === `/v1/vms/${vmId}/files/download` && url.searchParams.get("inode") === "31") {
        return new Response("review notes", {status: 200, headers: {"content-type": "application/octet-stream"}});
    }
    if (url.pathname === `/v1/vms/${vmId}/directory`) {
        const inode = Number(url.searchParams.get("inode") || 2);
        fixture.reads.push(inode);
        return new Response(JSON.stringify({ inode, entries: directories[inode] || [], max_file_bytes: 1024 }), {
            status: 200, headers: { "content-type": "application/json" }
        });
    }
    const response = await original.call(this, input, options);
    if (fixture.stopped && response.ok && (url.pathname === "/v1/vms" || url.pathname === `/v1/vms/${vmId}`)) {
        const data = await response.clone().json();
        const values = Array.isArray(data) ? data : [data];
        for (const vm of values) if (vm.id === vmId) vm.state = "stopped";
        return new Response(JSON.stringify(data), { status: response.status, headers: response.headers });
    }
    return response;
};
