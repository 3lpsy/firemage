const maxTextBytes = 1024 * 1024;

async function fileText(path) {
  const url = new URL(path, location.origin);
  if (url.origin !== location.origin || !/^\/v1\/vms\/[^/]+\/files\/download$/.test(url.pathname)) {
    throw new Error("Invalid file location");
  }
  const response = await fetch(url.href, { credentials: "same-origin", redirect: "error", signal: AbortSignal.timeout(30000) });
  if (response.status === 401) window.dispatchEvent(new Event("firemage-session-expired"));
  if (!response.ok) throw new Error(`Cannot read file (${response.status})`);
  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let text = "";
  let bytes = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      bytes += value.length;
      if (bytes > maxTextBytes) throw new Error("Copy supports text files up to 1 MiB");
      text += decoder.decode(value, { stream: true });
    }
    text += decoder.decode();
    if (text.includes("\0")) throw new Error("This file is not plain text");
    return text;
  } finally {
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

export async function copyContent(kind, value) {
  try {
    if (!navigator.clipboard) throw new Error("Clipboard access requires HTTPS");
    if (kind === "file") {
      // Begin the clipboard write during the click, before the file request resolves.
      if (globalThis.ClipboardItem && navigator.clipboard.write) {
        const content = fileText(value).then(text => new Blob([text], { type: "text/plain" }));
        content.catch(() => {});
        await navigator.clipboard.write([new ClipboardItem({ "text/plain": content })]);
      } else {
        await navigator.clipboard.writeText(await fileText(value));
      }
    } else {
      if (kind === "terminal") {
        const detail = { text: null };
        document.getElementById(value)?.dispatchEvent(new CustomEvent("firemage-copy-content", { detail }));
        if (detail.text === null) throw new Error("TTY stream is not ready");
        value = detail.text;
      } else if (kind !== "text") throw new Error("Unknown clipboard source");
      await navigator.clipboard.writeText(value);
    }
    return "";
  } catch (error) {
    return error.message || "Copy failed";
  }
}
