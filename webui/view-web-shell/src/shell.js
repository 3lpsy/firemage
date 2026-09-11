import "/assets/vendor/xterm.js";
import "/assets/vendor/addon-fit.js";

export function mountShell(containerId, statusId, actionId, id, csrf) {
  const container = document.getElementById(containerId);
  const status = document.getElementById(statusId);
  const action = document.getElementById(actionId);
  const terminal = new globalThis.Terminal({
    cursorBlink: true, disableStdin: true, scrollback: 3000,
    fontSize: 13, fontFamily: "ui-monospace, monospace",
    theme: { background: "#000000", foreground: "#ffffff" },
  });
  const fit = new globalThis.FitAddon.FitAddon();
  terminal.loadAddon(fit);
  terminal.open(container);
  let disposed = false;
  let session;
  const encoder = new TextEncoder();
  const isCurrent = current => !disposed && session === current && !current.controller.signal.aborted;
  const show = (state, message, label, detail = message) => {
    status.dataset.state = state;
    status.textContent = message;
    status.title = detail;
    action.textContent = label;
  };
  const disconnect = () => {
    const current = session;
    session = undefined;
    if (current) {
      current.controller.abort();
      clearTimeout(current.timer);
      if (current.socket) {
        current.socket.onmessage = current.socket.onerror = current.socket.onclose = null;
        current.socket.close();
      }
    }
    terminal.options.disableStdin = true;
  };
  const fail = (current, error) => {
    if (!isCurrent(current)) return;
    disconnect();
    show("failed", "Connection failed", "Reconnect", error.message);
  };
  const send = message => {
    const current = session;
    if (!current || !current.ready || !isCurrent(current)) return;
    if (current.socket.bufferedAmount > 65536) {
      fail(current, new Error("Shell input queue is full"));
      return;
    }
    try { current.socket.send(JSON.stringify(message)); }
    catch (error) { fail(current, error); }
  };
  const sendSize = () => send({ type: "resize", rows: Math.min(terminal.rows, 500), cols: Math.min(terminal.cols, 500) });
  const resize = new ResizeObserver(() => { if (!disposed) { fit.fit(); sendSize(); } });
  resize.observe(container);
  fit.fit();
  document.fonts.load('13px "Firemage Mono"').then(() => {
    if (!disposed) { terminal.options.fontFamily = '"Firemage Mono", monospace'; fit.fit(); sendSize(); }
  }).catch(() => {});
  const input = terminal.onData(data => {
    if (terminal.options.disableStdin) return;
    const bytes = encoder.encode(data);
    for (let offset = 0; offset < bytes.length; offset += 4096) {
      send({ type: "input", data: btoa(String.fromCharCode(...bytes.subarray(offset, offset + 4096))) });
    }
  });
  const connect = async () => {
    disconnect();
    if (disposed) return;
    const current = { controller: new AbortController(), queued: 0, ready: false };
    session = current;
    show("connecting", "Connecting…", "Disconnect");
    current.timer = setTimeout(() => fail(current, new Error("Guest helper did not respond. Check the guest kernel and initialization.")), 15000);
    try {
      const response = await fetch(`/v1/vms/${encodeURIComponent(id)}/shell/sessions`, {
        method: "POST", credentials: "same-origin", signal: current.controller.signal,
        headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf }, body: "{}",
      });
      if (response.status === 401) window.dispatchEvent(new Event("firemage-session-expired"));
      const value = await response.json();
      if (!response.ok) throw new Error(value.error || `Request failed (${response.status})`);
      if (!isCurrent(current)) return;
      if (typeof value.url !== "string" || !value.url.startsWith("/v1/")) throw new Error("Invalid shell connection URL");
      const url = new URL(value.url, location.href);
      if (url.origin !== location.origin || url.pathname !== `/v1/vms/${encodeURIComponent(id)}/shell/ws` || url.search || url.hash) throw new Error("Invalid shell connection URL");
      url.protocol = location.protocol === "https:" ? "wss:" : "ws:";
      if (typeof value.ticket !== "string" || !/^fm_shell_[A-Za-z0-9_-]+$/.test(value.ticket)) throw new Error("Invalid shell ticket");
      const socket = new WebSocket(url, ["firemage-shell", value.ticket]);
      current.socket = socket;
      socket.onerror = () => fail(current, new Error("Unable to connect to the guest shell"));
      socket.onclose = () => {
        if (isCurrent(current)) { disconnect(); show("disconnected", "Disconnected", "Reconnect"); }
      };
      socket.onmessage = event => {
        if (!isCurrent(current)) return;
        try {
          if (typeof event.data !== "string" || event.data.length > 65536) throw new Error("Invalid shell frame");
          const message = JSON.parse(event.data);
          switch (message.type) {
            case "ready":
              current.ready = true;
              clearTimeout(current.timer);
              terminal.options.disableStdin = false;
              show("connected", "Connected", "Disconnect");
              sendSize(); terminal.focus();
              break;
            case "output": {
              const bytes = Uint8Array.from(atob(message.data), character => character.charCodeAt(0));
              current.queued += bytes.length;
              if (current.queued > 1024 * 1024) throw new Error("Shell output queue is full");
              terminal.write(bytes, () => { current.queued -= bytes.length; });
              break;
            }
            case "exit":
              disconnect();
              show("exited", message.code === null ? "Shell exited" : `Shell exited (${message.code})`, "Reconnect");
              break;
            case "error": throw new Error(message.message);
            default: throw new Error("Unknown shell frame");
          }
        } catch (error) { fail(current, error); }
      };
    } catch (error) { fail(current, error); }
  };
  const toggle = () => {
    if (session) { disconnect(); show("disconnected", "Disconnected", "Reconnect"); }
    else connect();
  };
  action.addEventListener("click", toggle);
  connect();
  return { dispose() {
    disposed = true;
    disconnect();
    action.removeEventListener("click", toggle);
    resize.disconnect(); input.dispose(); terminal.dispose();
  } };
}
export function disposeShell(handle) { handle.dispose(); }
