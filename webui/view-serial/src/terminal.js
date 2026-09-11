import "/assets/vendor/xterm.js";
import "/assets/vendor/addon-fit.js";

export function mountTerminal(containerId, statusId, actionId, id, csrf) {
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
  const readText = event => {
    const buffer = terminal.buffer.active;
    let text = "";
    for (let index = 0; index < buffer.length; index++) {
      const line = buffer.getLine(index);
      if (index && !line.isWrapped) text += "\n";
      const nextWrapped = buffer.getLine(index + 1)?.isWrapped === true;
      text += line.translateToString(!nextWrapped);
    }
    event.detail.text = text.replace(/\n+$/, "");
  };
  container.addEventListener("firemage-copy-content", readText);
  const resize = new ResizeObserver(() => fit.fit());
  resize.observe(container);
  fit.fit();
  const path = `/v1/vms/${encodeURIComponent(id)}`;
  const encoder = new TextEncoder();
  let disposed = false;
  let session;
  let offset;
  const isCurrent = current => !disposed && session === current && !current.controller.signal.aborted;
  const show = (state, message, label, detail = message) => {
    status.textContent = message;
    status.dataset.state = state;
    status.title = detail;
    action.textContent = label;
  };
  const disconnect = () => {
    if (session) {
      clearTimeout(session.timer);
      session.controller.abort();
      session = undefined;
    }
    terminal.options.disableStdin = true;
  };
  const fail = (current, error) => {
    if (!isCurrent(current)) return;
    disconnect();
    show("failed", "Connection failed", "Reconnect", error.message);
  };
  const request = async (current, url, options = {}) => {
    if (!isCurrent(current)) throw new DOMException("Session closed", "AbortError");
    const response = await fetch(url, {
      credentials: "same-origin", signal: current.controller.signal, ...options,
    });
    if (response.status === 401) window.dispatchEvent(new Event("firemage-session-expired"));
    if (!response.ok) {
      const value = await response.json().catch(() => ({}));
      throw new Error(value.error || `Request failed (${response.status})`);
    }
    return response.status === 204 ? null : response.json();
  };
  document.fonts.load('13px "Firemage Mono"').then(() => {
    if (!disposed) {
      terminal.options.fontFamily = '"Firemage Mono", monospace';
      fit.fit();
    }
  }).catch(() => {});
  const input = terminal.onData((data) => {
    const current = session;
    if (!current || !isCurrent(current) || terminal.options.disableStdin) return;
    const bytes = encoder.encode(data).length;
    if (current.queuedBytes + bytes > 65536) return fail(current, new Error("Serial input queue is full"));
    current.queuedBytes += bytes;
    current.pending = current.pending.then(async () => {
      let chunk = "";
      let size = 0;
      const send = () => request(current, `${path}/terminal`, {
        method: "POST", headers: { "Content-Type": "application/json", "X-CSRF-Token": csrf },
        body: JSON.stringify({ input: chunk }),
      });
      for (const character of data) {
        const length = encoder.encode(character).length;
        if (size + length > 4096) { await send(); chunk = ""; size = 0; }
        chunk += character;
        size += length;
      }
      if (chunk) await send();
      current.queuedBytes -= bytes;
    }).catch(error => fail(current, error));
  });
  const poll = async current => {
    if (!isCurrent(current)) return;
    try {
      const capability = await request(current, `${path}/terminal`);
      if (!isCurrent(current)) return;
      const available = capability.state === "available";
      terminal.options.disableStdin = !available;
      const message = {
        available: "Connected",
        disabled: "Serial input disabled",
        "not-running": "Waiting for the VM to run.",
        "restart-required": "VM restart required",
      }[capability.state] || "Serial input unavailable";
      const detail = capability.state === "restart-required"
        ? "Stop and start this VM to restore serial input after the server restart."
        : message;
      const query = offset === undefined ? "" : `&offset=${offset}`;
      const output = await request(current, `${path}/logs?stream=serial${query}`);
      if (!isCurrent(current)) return;
      if (output.reset) terminal.reset();
      const data = Uint8Array.from(atob(output.base64), character => character.charCodeAt(0));
      offset = output.offset;
      if (data.length) await new Promise(resolve => terminal.write(data, resolve));
      if (!isCurrent(current)) return;
      show(available ? "connected" : "waiting", message, "Disconnect", detail);
      current.timer = setTimeout(() => poll(current), 500);
    } catch (error) { fail(current, error); }
  };
  const connect = () => {
    disconnect();
    if (disposed) return;
    session = { controller: new AbortController(), queuedBytes: 0, pending: Promise.resolve() };
    show("connecting", "Connecting…", "Disconnect");
    poll(session);
    terminal.focus();
  };
  const toggle = () => {
    if (session) { disconnect(); show("disconnected", "Disconnected", "Connect"); }
    else connect();
  };
  action.addEventListener("click", toggle);
  connect();
  return {
    dispose() {
      disposed = true;
      disconnect();
      action.removeEventListener("click", toggle);
      container.removeEventListener("firemage-copy-content", readText);
      resize.disconnect();
      input.dispose();
      terminal.dispose();
    },
  };
}

export function disposeTerminal(handle) { handle.dispose(); }
