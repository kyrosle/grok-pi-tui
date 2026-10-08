// One explicit local owner per store; all scheduling remains in the Harness.
import net from 'node:net';
import { createInterface } from 'node:readline';
import { createHash } from 'node:crypto';
import { mkdir, lstat, chmod, unlink } from 'node:fs/promises';
import { join } from 'node:path';

export async function socketPath(directory) {
  if (process.platform === 'win32') throw new Error('Background Durable owner currently requires Unix');
  // Short private directory avoids the macOS Unix socket path limit.
  const root = `/tmp/grok-pi-durable-${process.getuid()}`;
  await mkdir(root, { mode: 0o700 }).catch(error => { if (error.code !== 'EEXIST') throw error; });
  const info = await lstat(root);
  if (!info.isDirectory() || info.uid !== process.getuid() || (info.mode & 0o077)) throw new Error('Unsafe Durable socket directory');
  return join(root, `${createHash('sha256').update(directory).digest('hex').slice(0, 24)}.sock`);
}

export async function serveOwner(options, overrides = {}) {
  const { createController } = await import('./host.mjs');
  let client;
  const emit = value => new Promise(resolve => { if (!client || client.destroyed) resolve(); else client.write(`${JSON.stringify(value)}\n`, resolve); });
  const controller = await createController(options, overrides, emit);
  const path = await socketPath(controller.core.location.directory);
  // The exclusive store lock is already held; a stale socket cannot have a live owner.
  try { const old = await lstat(path); if (!old.isSocket() || old.uid !== process.getuid()) throw new Error('Unsafe stale Durable socket'); await unlink(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  let stopping = false, idle;
  const tasks = await controller.core.harness.taskGraph((await import('./core.mjs')).context);
  const maybeIdle = () => {
    clearTimeout(idle);
    if (!stopping && !client && !Object.keys(tasks.value.tasks).length) idle = setTimeout(() => stop(), 30000);
  };
  const unwatch = tasks.subscribe(maybeIdle);
  const stop = async () => {
    if (stopping) return; stopping = true; clearTimeout(idle); unwatch(); tasks.dispose();
    await controller.close(); client?.end(); server.close();
    const current = await lstat(path).catch(() => undefined);
    if (current?.ino === inode) await unlink(path);
  };
  const server = net.createServer(socket => {
    if (client) { socket.end(`${JSON.stringify({ kind: 'fatal', message: 'A UI is already attached to this Durable store.' })}\n`); return; }
    client = socket; clearTimeout(idle); socket.on('error', () => {});
    const send = value => new Promise(resolve => { if (socket.destroyed) resolve(); else socket.write(`${JSON.stringify(value)}\n`, resolve); });
    const input = createInterface({ input: socket, crlfDelay: Infinity });
    input.on('line', async line => {
      let request;
      try {
        if (line.length > 16 * 1024 * 1024) throw new Error('Durable request too large');
        request = JSON.parse(line);
        if (typeof request.id !== 'string' || typeof request.method !== 'string') throw new Error('Invalid Durable request');
        if (request.method === 'close') { await controller.request('detach'); await send({ kind: 'response', id: request.id, result: {} }); socket.end(() => socket.destroy()); return; }
        if (request.method === 'stop') { await controller.close(); await send({ kind: 'response', id: request.id, result: {} }); await stop(); return; }
        const result = await controller.request(request.method, request.params);
        await send({ kind: 'response', id: request.id, result: result ?? null });
      } catch (error) { await send({ kind: 'response', id: request?.id ?? '', error: String(error) }); }
    });
    socket.on('close', () => { if (client === socket) { client = undefined; if (!stopping) controller.request('detach').finally(maybeIdle); } });
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(path, resolve); });
  await chmod(path, 0o600); const inode = (await lstat(path)).ino;
  maybeIdle(); process.once('SIGTERM', () => stop()); process.once('SIGINT', () => stop());
  return { path, stop, controller };
}
