const targets = arguments[0].map(vm => ({ ...vm, state: 'paused' }));
const capture = arguments[1] !== 'restore';
const snapshot = arguments[2];
window.__snapshotTargets = { original: window.fetch, path: null };
window.fetch = async function(input, init) {
    const path = new URL(typeof input === 'string' ? input : input.url, location.origin).pathname;
    const method = init?.method || input.method || 'GET';
    if (capture && path === '/v1/vms' && method === 'GET') {
        return new Response(JSON.stringify(targets), { headers: { 'Content-Type': 'application/json' } });
    }
    if (capture && /^\/v1\/vms\/[^/]+\/snapshots$/.test(path) && method === 'POST') {
        window.__snapshotTargets.path = path;
        return new Response(JSON.stringify({ error: 'Capture target recorded' }), {
            status: 400, headers: { 'Content-Type': 'application/json' },
        });
    }
    if (!capture && path === `/v1/snapshots/${snapshot}/trust` && method === 'POST') {
        return new Response(JSON.stringify({ trusted: true }), { headers: { 'Content-Type': 'application/json' } });
    }
    if (!capture && /^\/v1\/vms\/[^/]+\/snapshots\/restore$/.test(path) && method === 'POST') {
        window.__snapshotTargets.path = path;
        return new Response(JSON.stringify({ error: 'Restore target recorded' }), {
            status: 400, headers: { 'Content-Type': 'application/json' },
        });
    }
    return window.__snapshotTargets.original.call(this, input, init);
};
