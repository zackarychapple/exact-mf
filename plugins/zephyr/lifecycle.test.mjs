import { test, expect } from 'bun:test';
import { generateKeyPairSync, sign } from 'node:crypto';
import { canonicalBytes } from '../../scripts/deploy-signing.mjs';
import { sha256 } from '../../scripts/origin.mjs';
import { collectSnapshot } from './snapshot.mjs';
import { publishSnapshot } from './index.mjs';

function fixture() {
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  const app = { id: 'test', displayName: 'Test', manifest: { deploy: { signing: { keys: { test: publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64') } } } } };
  const files = new Map();
  const old = Buffer.from('previous Hermes blob'), current = Buffer.from('current MF blob');
  function head(blob, seq) {
    const digest = sha256(blob);
    files.set(`.exact/blobs/${digest}`, blob);
    const envelope = { exact: 1, app: { id: 'test', name: 'Test' }, stream: { app: 'test', channel: 'prod', compatibilityId: 'cohort', seq }, release: `v${seq}`, plan: { url: `../../blobs/${digest}`, sha256: digest, bytes: blob.length }, assets: [] };
    envelope.signature = { keyId: 'test', ed25519: sign(null, canonicalBytes(envelope), privateKey).toString('base64') };
    return envelope;
  }
  const previous = head(old, 1), next = head(current, 2);
  files.set('.exact/prod/cohort/exact.json', Buffer.from(JSON.stringify(next)));
  for (const envelope of [previous, next]) files.set(`.exact/prod/cohort/releases/${envelope.release}.json`, Buffer.from(JSON.stringify({ envelope })));
  const html = Buffer.from('<h1>Exact</h1>'), id = 'a'.repeat(64);
  files.set('.exact/root/web/exact.json', Buffer.from(JSON.stringify({ webRoot: 1, id, files: [{ name: 'index.html', sha256: sha256(html), sourceSha256: sha256(html), bytes: html.length }] })));
  files.set(`.exact/root/web/releases/${id}/index.html`, html);
  files.set(`.exact/root/web/inventories/${id}.json`, files.get('.exact/root/web/exact.json')); 
  const origin = { get: async path => files.get(path) ?? null, list: async path => path === '.exact' ? ['root', 'blobs', 'prod'] : path === '.exact/prod' ? ['cohort'] : path.endsWith('/releases') && path.includes('/cohort/') ? ['v1.json', 'v2.json'] : [] };
  return { app, origin, files, old, current };
}

test('complete graph retains hidden records and previous blobs; tampering blocks upload', async () => {
  const f = fixture();
  const snapshot = await collectSnapshot(f.origin, f.app);
  expect(snapshot.get(`.exact/blobs/${sha256(f.old)}`)).toEqual(f.old);
  expect(snapshot.get('index.html').toString()).toContain('Exact');
  let uploads = 0;
  f.files.delete(`.exact/blobs/${sha256(f.old)}`);
  await expect(publishSnapshot({ ...f, upload: () => { uploads++; } })).rejects.toThrow('integrity');
  expect(uploads).toBe(0);
  f.files.set(`.exact/blobs/${sha256(f.old)}`, Buffer.from('tampered'));
  await expect(collectSnapshot(f.origin, f.app)).rejects.toThrow('integrity');
});

test('dry run makes no calls; wrong identity and failed verification never promote', async () => {
  const f = fixture();
  let uploads = 0, promotions = 0;
  const site = { org: 'test', project: 'test', app: 'test' };
  const receipt = { applicationUid: 'test.test.test', versionUrl: 'https://immutable.example/' };
  const control = { inspect: async () => ({ identityMatches: true, noAutomaticPromotion: true, versionId: null }), resolveBuild: async () => ({ ...receipt, versionId: 'v1' }), promote: async () => { promotions++; } };
  const args = { ...f, site, control, promote: true, upload: async () => { uploads++; return receipt; } };
  await publishSnapshot({ ...args, dryRun: true, control: null });
  expect(uploads).toBe(0);
  await expect(publishSnapshot({ ...args, fetchFile: async () => Buffer.from('HTML fallback') })).rejects.toThrow('verification');
  control.resolveBuild = async () => ({ ...receipt, applicationUid: 'wrong', versionId: 'v2' });
  await expect(publishSnapshot(args)).rejects.toThrow('identity');
  expect(promotions).toBe(0);
});

test('directory export materializes the browser root and excludes locks and unreferenced private blobs', async () => {
  const { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve, dirname } = await import('node:path');
  const { DirectoryOrigin } = await import('../../scripts/origin.mjs');
  const { exportSnapshot } = await import('./snapshot.mjs');
  const f = fixture(), dir = mkdtempSync(resolve(tmpdir(), 'exact-zephyr-test-'));
  try {
    const source = resolve(dir, 'origin');
    for (const [path, bytes] of f.files) {
      const dest = resolve(source, path);
      mkdirSync(dirname(dest), { recursive: true });
      writeFileSync(dest, bytes);
    }
    writeFileSync(resolve(source, '.exact/prod/cohort/.lock'), 'private lock');
    writeFileSync(resolve(source, '.exact/blobs/unreferenced'), 'private input');
    const output = resolve(dir, 'snapshot');
    await exportSnapshot(new DirectoryOrigin(source), f.app, output);
    expect(readFileSync(resolve(output, 'index.html'), 'utf8')).toBe('<h1>Exact</h1>');
    expect(existsSync(resolve(output, `.exact/blobs/${sha256(f.old)}`))).toBe(true);
    expect(existsSync(resolve(output, '.exact/prod/cohort/.lock'))).toBe(false);
    expect(existsSync(resolve(output, '.exact/blobs/unreferenced'))).toBe(false);
    await expect(exportSnapshot(new DirectoryOrigin(source), f.app, output)).rejects.toThrow();
    expect(readFileSync(resolve(output, 'index.html'), 'utf8')).toBe('<h1>Exact</h1>');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('verified immutable snapshot permits explicit promotion', async () => {
  const f = fixture();
  let uploaded, promoted = false;
  const receipt = { applicationUid: 'app.project.org', versionUrl: 'https://immutable.example/' };
  const control = {
    inspect: async () => ({ identityMatches: true, noAutomaticPromotion: true, versionId: 'old' }),
    resolveBuild: async () => ({ ...receipt, versionId: 'new' }),
    promote: async (_site, input) => { expect(input).toEqual({ versionId: 'new', expectedVersionId: 'old' }); promoted = true; },
  };
  await publishSnapshot({ ...f, site: { org: 'org', project: 'project', app: 'app' }, control, promote: true,
    upload: async files => { uploaded = files; return receipt; },
    fetchFile: async url => uploaded.get(new URL(url).pathname.slice(1)),
  });
  expect(promoted).toBe(true);
});
