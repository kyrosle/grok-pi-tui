// Keep published SDK package layouts intact (public asset/metadata paths).
import { cp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
for (const pkg of ['pi-durable', 'pi-coding-agent', 'pi-ai', 'chord']) {
  const info = JSON.parse(await readFile(`node_modules/@earendil-works/${pkg}/package.json`, 'utf8'));
  if (info.version !== '1.1.0') throw new Error(`Unexpected ${pkg} version: ${info.version}`);
}
// This is generated packaging output, never a user store. Do not ship stale
// modules left over from the previous SDK dependency tree.
await rm('dist', { recursive: true, force: true });
await mkdir('dist', { recursive: true });
for (const name of ['core.mjs', 'host.mjs', 'owner.mjs', 'subagent.mjs', 'capabilities.json', 'package.json', 'package-lock.json', 'licenses', 'node_modules']) await cp(name, `dist/${name}`, { recursive: true, dereference: false, verbatimSymlinks: true });
await writeFile('dist/NOTICE.txt', 'grok-pi Durable host uses published Pi SDKs 1.1.0. Pi: Copyright (c) 2025 Mario Zechner, MIT. Dependency licenses and metadata are preserved in node_modules; Pi license is in licenses/pi-MIT.txt.\n');
console.log('Durable host assembled: dist/host.mjs (requires system Node >=22.19.0)');
