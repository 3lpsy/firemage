import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test, beforeEach } from "node:test";

const source = await readFile(new URL("../src/clipboard.js", import.meta.url), "utf8");
const { copyContent } = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
let copied;
beforeEach(() => {
  copied = undefined;
  globalThis.location = { origin: "https://example.test" };
  globalThis.window = { dispatchEvent() {} };
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: { clipboard: {
    writeText: async value => { copied = value; },
    write: async items => { copied = await items[0].content["text/plain"].then(blob => blob.text()); },
  } } });
  globalThis.ClipboardItem = class { constructor(content) { this.content = content; } };
});
const file = "/v1/vms/test/files/download?inode=31";

test("copies literal configuration without altering whitespace or markup", async () => {
  const text = 'userdata = "<script>\\n"\n  spaces  \n';
  assert.equal(await copyContent("text", text), "");
  assert.equal(copied, text);
});

test("copies UTF-8 split across response chunks as plain text", async () => {
  const bytes = new TextEncoder().encode("résumé\n");
  globalThis.fetch = async (url, options) => {
    assert.equal(url, `https://example.test${file}`);
    assert.equal(options.credentials, "same-origin");
    return new Response(new ReadableStream({ start(controller) {
      controller.enqueue(bytes.slice(0, 2)); controller.enqueue(bytes.slice(2)); controller.close();
    } }));
  };
  assert.equal(await copyContent("file", file), "");
  assert.equal(copied, "résumé\n");
});

test("rejects cross-origin and non-file locations", async () => {
  globalThis.fetch = async () => assert.fail("unexpected request");
  for (const path of ["https://other.test" + file, "/v1/secrets/key"]) {
    assert.equal(await copyContent("file", path), "Invalid file location");
    assert.equal(copied, undefined);
  }
});

test("HTTP errors and binary files do not overwrite clipboard", async () => {
  for (const response of [new Response("denied", { status: 403 }), new Response(new Uint8Array([255])), new Response("nul\0byte")]) {
    globalThis.fetch = async () => response;
    assert.notEqual(await copyContent("file", file), "");
    assert.equal(copied, undefined);
  }
});

test("oversized text cancels the response stream", async () => {
  let cancelled = false;
  globalThis.fetch = async () => new Response(new ReadableStream({
    start(controller) { controller.enqueue(new Uint8Array(1024 * 1024 + 1)); },
    cancel() { cancelled = true; },
  }));
  assert.equal(await copyContent("file", file), "Copy supports text files up to 1 MiB");
  assert.equal(cancelled, true);
  assert.equal(copied, undefined);
});

test("clipboard denial is reported and never claimed as success", async () => {
  navigator.clipboard.writeText = async () => { throw new Error("Permission denied"); };
  assert.equal(await copyContent("text", "value"), "Permission denied");
  assert.equal(copied, undefined);
});

test("starts file clipboard write before file loading resolves", async () => {
  let begin;
  let wrote = false;
  globalThis.fetch = () => new Promise(resolve => { begin = resolve; });
  const write = navigator.clipboard.write;
  navigator.clipboard.write = items => { wrote = true; return write(items); };
  const result = copyContent("file", file);
  assert.equal(wrote, true);
  begin(new Response("loaded"));
  assert.equal(await result, "");
  assert.equal(copied, "loaded");
});
