#!/usr/bin/env node
// Capture one check's combined output and exit status without changing its result.
import { spawn } from 'node:child_process';
import { mkdirSync, openSync, closeSync, writeSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { constants } from 'node:os';

const [phase, command, ...args] = process.argv.slice(2);
const directory = process.env.FIREMAGE_CI_RESULTS_DIR;
if (!directory || !/^[a-z][a-z0-9-]{0,63}$/.test(phase ?? '') || !command) {
  console.error('Usage: ci-record.mjs PHASE COMMAND [ARGS]; set FIREMAGE_CI_RESULTS_DIR');
  process.exit(2);
}
mkdirSync(directory, { recursive: true });
const started = Date.now();
const result = { phase, started_at: new Date(started).toISOString(), status: 'running' };
const save = () => writeFileSync(join(directory, `${phase}.json`), JSON.stringify(result, null, 2));
save();
const log = openSync(join(directory, `${phase}.log`), 'w', 0o600);
const child = spawn(command, args, { stdio: ['inherit', 'pipe', 'pipe'] });
for (const stream of [child.stdout, child.stderr]) {
  stream.on('data', chunk => { writeSync(log, chunk); process.stdout.write(chunk); });
}
child.on('error', error => {
  const message = `Cannot start ${command}: ${error.message}\n`;
  writeSync(log, message);
  process.stderr.write(message);
});
for (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => child.kill(signal));
child.on('close', (code, signal) => {
  result.exit_code = code >= 0 && code !== null ? code : (signal ? 128 + constants.signals[signal] : 127);
  result.status = result.exit_code === 0 ? 'passed' : 'failed';
  result.duration_ms = Date.now() - started;
  result.signal = signal;
  save();
  closeSync(log);
  process.exitCode = result.exit_code;
});
