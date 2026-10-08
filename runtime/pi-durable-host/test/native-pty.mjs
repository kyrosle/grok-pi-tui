// Real native Pager, published Durable SDK and xterm screen. No paid model.
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { createInterface } from 'node:readline';
import assert from 'node:assert/strict';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const binary = realpathSync(process.env.GROK_PI_BINARY ?? join(root, 'target/debug/grok-pi'));
const require = createRequire(realpathSync(execFileSync('which', ['pi'], { encoding: 'utf8' }).trim()));
const { Terminal } = require('@xterm/headless');
const directory = mkdtempSync(join(tmpdir(), 'grok-pi-durable-pty-'));
const home = join(directory, 'grok'); mkdirSync(home);
writeFileSync(join(home, 'config.toml'), '[ui]\nlanguage="en"\npi_durable=true\n');
writeFileSync(join(directory, 'fixture.txt'), 'DURABLE_PTY_FILE');
const env = { ...process.env, GROK_HOME: home, GROK_PROJECT_DIR: '.grok-pi', PI_CODING_AGENT_DIR: join(directory, 'pi'), GROK_PI_DURABLE_HOST: process.env.GROK_PI_DURABLE_HOST ?? join(root, 'runtime/pi-durable-host/test/fixture.mjs'), GROK_PI_NO_AUTO_UPDATE: '1', GROK_CONTEXTUAL_HINTS: '0', PI_PROGRAM_STATUS: 'auto', TERM_PROGRAM: 'xterm', TERM: 'xterm-256color', COLORTERM: 'truecolor' };
delete env.TMUX; delete env.STY;
for (const key of Object.keys(env)) if (key.endsWith('_API_KEY') || key.endsWith('_TOKEN') || key.startsWith('AWS_')) delete env[key];

async function run(resume) {
  const argv = [binary, '--no-approve', '--no-context-files', '--no-skills'];
  if (resume) argv.push('--durable', '--continue');
  const bridge = spawn('python3', ['-u', join(root, 'crates/codegen/pi-grok-adapter/tests/fixtures/pty_bridge.py'), JSON.stringify({ argv, cwd: directory, rows: 40, cols: 120 })], { env });
  const terminal = new Terminal({ rows: 40, cols: 120, allowProposedApi: true });
  let writes = Promise.resolve(), exit, diagnostics = '', raw = '';
  const statuses = [];
  const send = data => bridge.stdin.write(`${JSON.stringify({ type: 'write', data: Buffer.from(data).toString('base64') })}\n`);
  terminal.onData(send); bridge.stdin.on('error', () => {});
  terminal.parser.registerOscHandler(7501, body => {
    if (body.startsWith('?')) send('\x1b]7501;?\x1b\\');
    else statuses.push(body);
    return true;
  });
  bridge.stderr.on('data', b => { diagnostics += b; });
  createInterface({ input: bridge.stdout }).on('line', line => {
    const event = JSON.parse(line);
    if (event.type === 'exit') exit = event.status;
    if (event.type === 'output') { const bytes = Buffer.from(event.data, 'base64'); raw += bytes.toString(); writes = writes.then(() => new Promise(done => terminal.write(bytes, done))); }
  });
  const screen = () => Array.from({ length: 40 }, (_, y) => terminal.buffer.active.getLine(terminal.buffer.active.viewportY + y)?.translateToString(true) ?? '').join('\n');
  const wait = async (predicate, label) => {
    const deadline = Date.now() + 30000;
    while (Date.now() < deadline) { await writes; const text = screen(); if (predicate(text)) return text; if (exit !== undefined) throw new Error(`exit ${exit}: ${label}\n${text}\n${diagnostics}`); await new Promise(done => setTimeout(done, 30)); }
    throw new Error(`timeout ${label}\n${screen()}\n${diagnostics}`);
  };
  try {
    await wait(t => t.includes('grok-pi') || t.includes('Durable fixture'), 'startup');
    if (!resume) { await new Promise(done => setTimeout(done, 1000)); send('/new\r'); await new Promise(done => setTimeout(done, 1000)); }
    if (resume) await wait(t => t.includes('DURABLE_REPLY still durable') && !t.includes('Loading session'), 'committed replay finished');
    const prompt = resume ? 'resume verification' : 'hello durable';
    send(`${prompt}\r`);
    await wait(t => t.includes(`DURABLE_REPLY ${prompt}`), 'fresh native streamed message');
    if (!resume) {
      send('read fixture\r'); await wait(t => t.includes('DURABLE_TOOL_DONE'), 'native tool card');
      send('/tasks\r'); await wait(t => t.includes('Durable tasks'), 'native task picker'); send('\x1b'); await wait(t => !t.includes('Durable tasks'), 'task picker close');
      await new Promise(done => setTimeout(done, 100));
      send('\x1bOQ'); await wait(t => t.includes('Settings'), 'F2'); send('/');
      await wait(t => t.includes('type to filter'), 'F2 search'); send('durable');
      await wait(t => t.includes('Durable mode') && !t.includes('No matches'), 'F2 mode row');
      writeFileSync(join(directory, 'f2.txt'), screen());
      send('\r'); await wait(t => t.includes('Space toggle') && t.includes('Current runtime: Pi Durable'), 'current vs saved mode');
      send(' '); await wait(() => readFileSync(join(home, 'config.toml'), 'utf8').includes('pi_durable = false'), 'F2 saved preference');
      send('\x1bOQ');
      send('still durable\r'); await wait(t => t.includes('DURABLE_REPLY still durable'), 'F2 does not change active backend');
    }
    writeFileSync(join(directory, resume ? 'resume.txt' : 'live.txt'), screen());
    send('/exit\r');
    const deadline = Date.now() + 10000;
    while (exit === undefined && Date.now() < deadline) await new Promise(done => setTimeout(done, 25));
    assert.equal(exit, 0); assert.ok(raw.includes('grok-pi --durable --session durable:'));
    await writes;
    assert.ok(statuses.some(value => value.includes('state=working')));
    assert.ok(statuses.some(value => value.includes('state=done')));
    assert.ok(statuses.some(value => value.includes('state=clear')));
    assert.ok(statuses.every(value => !value.includes('msg=') && !value.includes(prompt)));
    writeFileSync(join(directory, resume ? 'resume-status.json' : 'live-status.json'), JSON.stringify(statuses));
  } finally {
    if (exit === undefined) bridge.stdin.write(`${JSON.stringify({ type: 'close' })}\n`);
    bridge.stdin.end(); terminal.dispose(); writeFileSync(join(directory, resume ? 'resume.raw' : 'live.raw'), raw);
  }
}
try { await run(false); await run(true); console.log(JSON.stringify({ passed: true, artifactDirectory: directory })); }
catch (error) { console.error(String(error)); console.error(`Evidence: ${directory}`); process.exitCode = 1; }
