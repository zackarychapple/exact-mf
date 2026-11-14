import { mkdirSync, mkdtempSync, writeFileSync, renameSync, rmSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { DirectoryOrigin, sha256, safeRelative, parseWebRoot, webRootPath, webReleasePath } from '../../scripts/origin.mjs';
import { inspectHead } from '../../scripts/deploy.mjs';
import { INSTALL_PUBLIC } from '../../scripts/install-page.mjs';

const publicInstallFiles = new Set(INSTALL_PUBLIC.map(path => path.slice(1)));

// Select public protocol objects, never copy an origin directory wholesale.
export async function collectSnapshot(origin, app) {
  const files = new Map();
  function publicName(name) {
    if (publicInstallFiles.has(name)) return;
    if (name.split('/').some(p => p.startsWith('.')) || /(?:\.map|\.pem|\.key|\.p12|\.pfx|\.p8|\.lock|\.rs|\.ts|\.contract)$/.test(name)
      || /(?:^|\/)(?:node_modules|target|src|package.json|bun.lock|Cargo.lock|AGENTS.md)(?:\/|$)/i.test(name)) throw new Error(`private public path: ${name}`);
  }
  async function add(path, card) {
    safeRelative(path);
    if (!/^[A-Za-z0-9._@/-]+$/.test(path)) throw new Error(`unsafe snapshot path: ${path}`);
    const bytes = await origin.get(path);
    if (!bytes || card && (sha256(bytes) !== card.sha256 || bytes.length !== card.bytes)) throw new Error(`snapshot integrity failure: ${path}`);
    if (/-----BEGIN (?:[A-Z]+ )*PRIVATE KEY-----/.test(bytes.toString('utf8'))) throw new Error(`private key in snapshot output: ${path}`);
    files.set(path, bytes);
    return bytes;
  }
  async function envelope(bytes, stream) {
    const result = inspectHead({ bytes }, app, stream);
    if (!result.usable || !result.authenticated) throw new Error(`snapshot envelope: ${result.problem ?? 'signature required'}`);
    for (const card of [result.head.plan, ...result.head.assets ?? []]) {
      if (card.name) publicName(card.name);
      if (card.url !== `../../blobs/${card.sha256}`) throw new Error('snapshot requires local content-addressed blobs');
      await add(`.exact/blobs/${card.sha256}`, card);
    }
  }
  const root = parseWebRoot(await add(webRootPath));
  // Inventories become visible only after every immutable payload is verified.
  // An interrupted publication can leave a release directory without an inventory.
  for (const name of await origin.list('.exact/root/web/inventories', { includeHidden: true }) ?? []) {
    if (!/^[a-f0-9]{64}\.json$/.test(name)) throw new Error('unsafe web inventory');
    const id = name.slice(0, -5);
    const inventoryPath = `.exact/root/web/inventories/${name}`;
    const inventory = parseWebRoot(await add(inventoryPath));
    if (inventory.id !== id) throw new Error('web inventory identity mismatch');
    for (const card of inventory.files) {
      publicName(card.name);
      await add(`${webReleasePath(id)}/${card.name}`, card);
    }
  }
  for (const card of root.files) {
    const bytes = await add(`${webReleasePath(root.id)}/${card.name}`, card);
    publicName(card.name);
    files.set(card.name, bytes);
  }
  if (!files.has('index.html')) throw new Error('snapshot requires index.html');
  for (const channel of await origin.list('.exact', { includeHidden: true }) ?? []) {
    if (['root', 'blobs'].includes(channel) || publicInstallFiles.has(`.exact/${channel}`)) continue;
    for (const compatibilityId of await origin.list(`.exact/${channel}`, { includeHidden: true }) ?? []) {
      const stream = { channel, compatibilityId };
      const base = `.exact/${channel}/${compatibilityId}`;
      if (publicInstallFiles.has(base)) continue;
      if (await origin.get(`${base}/exact.json`) === null) continue;
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
  const output = resolve(destination);
  mkdirSync(output); // Reserve a new destination. Never overwrite a previous snapshot.
  const staging = mkdtempSync(`${output}.pending-`);
  try {
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
