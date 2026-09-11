const id = arguments[0];
const original = window.fetch;
const OriginalSocket = window.WebSocket;
const fixture = window.__webShell = { original, OriginalSocket, sockets: [], fail: true, sent: [] };
window.fetch = async function(input, init) {
  const url = new URL(typeof input === 'string' ? input : input.url, location.origin);
  if (url.pathname === `/v1/vms/${id}/shell/sessions`) {
    fixture.posts = (fixture.posts || 0) + 1;
    if (init?.credentials !== 'same-origin' || !init?.headers?.['X-CSRF-Token']) throw Error('shell ticket requires session and CSRF');
    const respond = () => new Response(JSON.stringify({url: `/v1/vms/${id}/shell/ws`, ticket: 'fm_shell_fixture'}), {headers: {'Content-Type': 'application/json'}});
    if (fixture.deferNext) {
      fixture.deferNext = false;
      fixture.pendingSignal = init.signal;
      return new Promise(resolve => { fixture.resolvePending = () => resolve(respond()); });
    }
    return respond();
  }
  const response = await original.call(this, input, init);
  if (url.pathname === `/v1/vms/${id}` && (init?.method || 'GET') === 'GET' && response.ok) {
    const value = await response.json();
    value.state = 'running';
    return new Response(JSON.stringify(value), {headers: {'Content-Type': 'application/json'}});
  }
  return response;
};
window.WebSocket = class {
  bufferedAmount = 0;
  constructor(url, protocols) {
    const address = new URL(url);
    if (address.pathname !== `/v1/vms/${id}/shell/ws` || address.search || protocols?.[0] !== 'firemage-shell' || protocols?.[1] !== 'fm_shell_fixture') throw Error('unexpected shell socket ticket transport');
    fixture.sockets.push(this);
    setTimeout(() => {
      if (this.closed) return;
      if (fixture.fail) { fixture.fail = false; this.onerror?.({}); }
      else {
        this.onmessage?.({data: JSON.stringify({type: 'ready'})});
        this.onmessage?.({data: JSON.stringify({type: 'output', data: btoa('\x1b[31mguest-shell-ready\x1b[0m\r\n$ ')})});
      }
    }, 20);
  }
  send(text) { fixture.sent.push(JSON.parse(text)); }
  close() { this.closed = true; }
};
