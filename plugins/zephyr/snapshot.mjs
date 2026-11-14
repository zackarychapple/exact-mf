import { mkdirSync, writeFileSync, renameSync, rmSync, lstatSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { DirectoryOrigin, sha256, safeRelative, parseWebRoot, webRootPath, webReleasePath } from '../../scripts/origin.mjs';
import { inspectHead } from '../../scripts/deploy.mjs';

// Select public protocol objects, never copy an origin directory wholesale.
export async function collectSnapshot(origin, app) {
  const files = new Map();
  function publicName(name) {
    if (name.split('/').some(p => p.startsWith('.')) || /(?:\.map|\.pem|\.key|\.lock|\.rs|\.ts|\.contract)$/.test(name)
      || /(?:^|\/)(?:node_modules|target|src|package.json|bun.lock|Cargo.lock|AGENTS.md)(?:\/|$)/i.test(name)) throw new Error(`private public path: ${name}`);
  }
  async function add(path, card) {
    safeRelative(path);
    const bytes = await origin.get(path);
    if (!bytes || card && (sha256(bytes) !== card.sha256 || bytes.length !== card.bytes)) throw new Error(`snapshot integrity failure: ${path}`);
    files.set(path, bytes);
    return bytes;
  }
  async function envelope(bytes, stream) {
    const result = inspectHead({ bytes }, app, stream);
    if (!result.usable || !result.authenticated) throw new Error(`snapshot envelope: ${result.problem ?? 'signature required'}`);
    for (const card of [result.head.plan, ...result.head.assets ?? []]) {
      if (card.url !== `../../blobs/${card.sha256}`) throw new Error('snapshot requires local content-addressed blobs');
      await add(`.exact/blobs/${card.sha256}`, card);
    }
  }
  const root = parseWebRoot(await add(webRootPath));
  for (const id of await origin.list('.exact/root/web/releases', { includeHidden: true }) ?? []) {
    if (!/^[a-f0-9]{64}$/.test(id)) throw new Error('unsafe web release');
    // Retain complete previous web generations, using their published metadata.
    const prefix = webReleasePath(id);
    async function walk(path) {
      for (const name of await origin.list(path, { includeHidden: true }) ?? []) {
        const child = `${path}/${name}`;
        const stat = lstatSync(origin.path(child));
        if (stat.isDirectory()) await walk(child);
        else if (stat.isFile() && !stat.isSymbolicLink()) { publicName(child.slice(prefix.length + 1)); await add(child); }
        else throw new Error(`unsafe public output: ${child}`);
      }
    }
    await walk(prefix);
  }
  for (const card of root.files) {
    const bytes = await add(`${webReleasePath(root.id)}/${card.name}`, card);
    publicName(card.name);
    files.set(card.name, bytes);
  }
  if (!files.has('index.html')) throw new Error('snapshot requires index.html');
  for (const channel of await origin.list('.exact', { includeHidden: true }) ?? []) {
    if (['root', 'blobs'].includes(channel)) continue;
    for (const compatibilityId of await origin.list(`.exact/${channel}`, { includeHidden: true }) ?? []) {
      const stream = { channel, compatibilityId };
      const base = `.exact/${channel}/${compatibilityId}`;
      await envelope(await add(`${base}/exact.json`), stream);
      for (const name of await origin.list(`${base}/releases`, { includeHidden: true }) ?? []) {
        if (!/^[A-Za-z0-9][A-Za-z0-9._-]*\.json$/.test(name)) throw new Error('unsafe release name');
        const record = JSON.parse(await add(`${base}/releases/${name}`));
        await envelope(Buffer.from(JSON.stringify(record.envelope)), stream);
      }
    }
  }
  return files;
}

export async function exportSnapshot(origin, app, destination) {
  if (!(origin instanceof DirectoryOrigin)) throw new Error('snapshot export requires a retained directory origin');
  const files = await collectSnapshot(origin, app);
  const output = resolve(destination), staging = `${output}.pending-${process.pid}`;
  try {
    mkdirSync(output); // Reserve a new destination. Never overwrite a previous snapshot.
    mkdirSync(staging);
    for (const [path, bytes] of files) {
      const target = resolve(staging, safeRelative(path));
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, bytes, { flag: 'wx' });
    }
    rmSync(output, { recursive: true });
    renameSync(staging, output);
  } catch (error) { rmSync(staging, { recursive: true, force: true }); throw error; }
  return { directory: output, files: [...files].map(([path, bytes]) => ({ path, bytes: bytes.length, sha256: sha256(bytes) })) };
}
