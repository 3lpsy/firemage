const original = window.fetch;
const pending = [];
window.__egressFetch = { original, pending };
window.fetch = function(input, init) {
    const path = new URL(typeof input === 'string' ? input : input.url, location.origin).pathname;
    const method = init?.method || input.method || 'GET';
    if (method === 'GET' && path.startsWith('/v1/egress/')) {
        return new Promise((resolve, reject) => pending.push({
            path,
            release(body, status = 200) {
                if (body === null) original.call(window, input, init).then(resolve, reject);
                else resolve(new Response(JSON.stringify(body), {
                    status, headers: { 'Content-Type': 'application/json' },
                }));
            },
        }));
    }
    return original.call(this, input, init);
};
