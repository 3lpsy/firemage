import "/assets/vendor/xterm.js";
import "/assets/vendor/addon-fit.js";

export function mountTerminal(containerId, statusId, id, csrf) {
  const container = document.getElementById(containerId);
  const status = document.getElementById(statusId);
  const terminal = new globalThis.Terminal({
    cursorBlink: true, disableStdin: true, scrollback: 3000,
    fontSize: 13, fontFamily: "ui-monospace, monospace",
    theme: { background: "#000000", foreground: "#ffffff" },
  });
  const fit = new globalThis.FitAddon.FitAddon();
  terminal.loadAddon(fit);
  terminal.open(container);
  const resize = new ResizeObserver(() => fit.fit());
  resize.observe(container);
  fit.fit();
  const controller = new AbortController();
  const path = `/v1/vms/${encodeURIComponent(id)}`;
  let stopped = false;
  let timer;
  let offset;
  let queuedBytes = 0;
  let pending = Promise.resolve();
  const encoder = new TextEncoder();
  const fail = (error) => {
    if (stopped) return;
    status.textContent = `${error.message}. Disconnect and reconnect to retry.`;
    terminal.options.disableStdin = true;
    stopped = true;
    clearTimeout(timer);
    controller.abort();
  };
  const request = async (url, options = {}) => {
    const response = await fetch(url, {
      credentials: "same-origin", signal: controller.signal, ...options,
    });
    if (response.status === 401) window.dispatchEvent(new Event("firemage-session-expired"));
    if (!response.ok) {
      const value = await response.json().catch(() => ({}));
      throw new Error(value.error || `Request failed (${response.status})`);
    }
    return response.status === 204 ? null : response.json();
  };
  document.fonts.load('13px "Firemage Mono"').then(() => {
    if (!stopped) {
      terminal.options.fontFamily = '"Firemage Mono", monospace';
      fit.fit();
    }
  }).catch(fail);
  const input = terminal.onData((data) => {
    if (stopped || terminal.options.disableStdin) return;
    const bytes = encoder.encode(data).length;
    if (queuedBytes + bytes > 65536) return fail(new Error("Terminal input queue is full"));
    queuedBytes += bytes;
    pending = pending.then(async () => {
      let chunk = "";
      let size = 0;
      const send = () => request(`${path}/terminal`, {
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
      queuedBytes -= bytes;
    }).catch(fail);
  });
  const poll = async () => {
    if (stopped) return;
    try {
      const capability = await request(`${path}/terminal`);
      const available = capability.state === "available";
      terminal.options.disableStdin = !available;
      status.textContent = {
        available: "Connected to guest ttyS0. A guest console program is required.",
        disabled: "Terminal input is disabled.",
        "not-running": "Waiting for the VM to run.",
        "restart-required": "Stop and start this VM to restore terminal input after the server restart.",
      }[capability.state] || "Terminal unavailable.";
      const query = offset === undefined ? "" : `&offset=${offset}`;
      const output = await request(`${path}/logs?stream=serial${query}`);
      if (output.reset) terminal.reset();
      const data = Uint8Array.from(atob(output.base64), (character) => character.charCodeAt(0));
      if (data.length) await new Promise((resolve) => terminal.write(data, resolve));
      offset = output.offset;
      if (!stopped) timer = setTimeout(poll, 500);
    } catch (error) { fail(error); }
  };
  poll();
  terminal.focus();
  return {
    dispose() {
      stopped = true;
      clearTimeout(timer);
      controller.abort();
      resize.disconnect();
      input.dispose();
      terminal.dispose();
    },
  };
}

export function disposeTerminal(handle) { handle.dispose(); }
