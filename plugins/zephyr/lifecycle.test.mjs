import { test, expect } from 'bun:test';
import { generateKeyPairSync, sign } from 'node:crypto';
import { canonicalBytes } from '../../scripts/deploy-signing.mjs';
import { sha256 } from '../../scripts/origin.mjs';
import { collectSnapshot } from './snapshot.mjs';
import { publishSnapshot, parseCliVersionUrl } from './index.mjs';

test('the published CLI logger prefixes both the completion message and version URL', () => {
  const output = "\u001b[36m ZEPHYR \u001b[0m  Deployed to Zephyr's edge in 1851ms.\n ZEPHYR   https://immutable.example/\n";
  expect(parseCliVersionUrl(output)).toBe('https://immutable.example/');
  expect(() => parseCliVersionUrl(output + output.replace('immutable.example', 'another.example'))).toThrow('unambiguous');
});
import { publishRoot } from '../../scripts/deploy.mjs';

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

async function withFakeCli(run) {
  const fs = await import('node:fs');
  const { resolve, dirname } = await import('node:path');
  const { tmpdir } = await import('node:os');
  const dir = fs.mkdtempSync(resolve(tmpdir(), 'exact-cli-test-')), bin = resolve(dir, 'bin');
  fs.mkdirSync(bin);
  const capture = resolve(dir, 'invocation.json');
  fs.writeFileSync(resolve(bin, 'pnpm'), `#!${process.execPath}
import {writeFileSync,readFileSync} from 'node:fs';
writeFileSync(process.env.EXACT_ZEPHYR_TEST_CAPTURE, JSON.stringify({cwd:process.cwd(),args:process.argv.slice(2),config:readFileSync('zephyr.config.mjs','utf8'),failBuild:process.env.ZE_FAIL_BUILD}));
if(process.env.EXACT_ZEPHYR_TEST_MODE==='failure'){console.error('test uploader failed after accepting assets');process.exit(2);}
console.log("Deployed to \\x1b[36mZephyr\\x1b[0m's edge in 12ms.\\n\\nhttps://immutable.example/");
if(process.env.EXACT_ZEPHYR_TEST_MODE==='ambiguous')console.log("Deployed to Zephyr's edge in 13ms.\\n\\nhttps://other.example/");
console.log('Updated 1 targets in 4ms\\n- https://stable.example/');
`, { mode: 0o700 });
  const before = { PATH: process.env.PATH, capture: process.env.EXACT_ZEPHYR_TEST_CAPTURE, mode: process.env.EXACT_ZEPHYR_TEST_MODE };
  const oldFetch = globalThis.fetch;
  process.env.PATH = `${dirname(process.execPath)}:${bin}:${before.PATH}`;
  process.env.EXACT_ZEPHYR_TEST_CAPTURE = capture;
  delete process.env.EXACT_ZEPHYR_TEST_MODE;
  globalThis.fetch = async url => {
    const invoked = JSON.parse(fs.readFileSync(capture, 'utf8'));
    const path = new URL(url).pathname.slice(1);
    if (process.env.EXACT_ZEPHYR_TEST_MODE === 'wrong-marker' && path === 'exact-zephyr-upload-id.txt') return new Response('another upload');
    if (process.env.EXACT_ZEPHYR_TEST_MODE === 'tampered' && path === 'index.html') return new Response('HTML fallback');
    return new Response(fs.readFileSync(resolve(invoked.cwd, 'dist', path)));
  };
  try { await run({ fs, resolve, dir, capture }); }
  finally {
    globalThis.fetch = oldFetch;
    process.env.PATH = before.PATH;
    for (const [key, value] of [['EXACT_ZEPHYR_TEST_CAPTURE', before.capture], ['EXACT_ZEPHYR_TEST_MODE', before.mode]]) {
      if (value === undefined) delete process.env[key]; else process.env[key] = value;
    }
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test('configured deploy output invokes pinned CLI and verifies bytes without control hooks', async () => {
  await withFakeCli(async ({ fs, resolve, dir, capture }) => {
    const { DirectoryOrigin } = await import('../../scripts/origin.mjs');
    const { deploymentOutput } = await import('./deploy-output.mjs');
    const f = fixture(), source = resolve(dir, 'origin'), config = resolve(dir, 'site.json');
    const { INSTALL_PUBLIC } = await import('../../scripts/install-page.mjs');
    const rootPath = '.exact/root/web/exact.json', root = JSON.parse(f.files.get(rootPath));
    for (const path of INSTALL_PUBLIC) {
      const name = path.slice(1), body = Buffer.from(name.endsWith('.json') ? '{}' : '<h1>Install</h1>');
      root.files.push({ name, bytes: body.length, sha256: sha256(body), sourceSha256: sha256(body) });
      f.files.set(`.exact/root/web/releases/${root.id}/${name}`, body);
    }
    f.files.set(rootPath, Buffer.from(JSON.stringify(root)));
    f.files.set(`.exact/root/web/inventories/${root.id}.json`, f.files.get(rootPath));
    for (const [name, bytes] of f.files) {
      const path = resolve(source, name);
      fs.mkdirSync((await import('node:path')).dirname(path), { recursive: true });
      fs.writeFileSync(path, bytes);
    }
    fs.writeFileSync(config, JSON.stringify({ org: 'org', project: 'project', app: 'app' }));
    const result = await deploymentOutput({ opts: { yes: true, export: resolve(dir, 'export'), zephyrConfig: config }, origin: new DirectoryOrigin(source), app: f.app,
      refused: [{ kind: 'binary', reason: 'Initial host binary must be distributed separately' }] });
    expect(result.transport.verified).toBe(true);
    expect(result.transport.versionUrl).toBe('https://immutable.example/');
    expect(result.transport.promotionRequested).toBe(false);
    for (const path of INSTALL_PUBLIC) expect(result.files.some(file => file.path === path.slice(1))).toBe(true);
    const invoked = JSON.parse(fs.readFileSync(capture));
    expect(invoked.args).toEqual(['--package=zephyr-cli@1.4.1', 'dlx', 'ze-cli', 'deploy', './dist', '--target', 'web']);
    expect(invoked.config).toContain('"appName":"app"');
    expect(invoked.failBuild).toBe('true');
    const evidence = result.transport.evidenceDirectory;
    expect(fs.existsSync(resolve(evidence, 'verified.json'))).toBe(true);
    expect(fs.existsSync(resolve(evidence, 'dist'))).toBe(false);
    fs.rmSync(evidence, { recursive: true, force: true });
  });
});

test('CLI failure, ambiguous receipt, wrong marker and corrupt file preserve evidence', async () => {
  await withFakeCli(async ({ fs, resolve, capture }) => {
    for (const mode of ['failure', 'ambiguous', 'wrong-marker', 'tampered']) {
      process.env.EXACT_ZEPHYR_TEST_MODE = mode;
      let failure;
      try { await publishSnapshot({ ...fixture(), site: { org: 'org', project: 'project', app: 'app' } }); }
      catch (error) { failure = error; }
      expect(failure).toBeDefined();
      expect(failure.message).toContain('upload evidence:');
      const { cwd } = JSON.parse(fs.readFileSync(capture));
      expect(fs.existsSync(resolve(cwd, 'dist/.exact/prod/cohort/exact.json'))).toBe(true);
      expect(fs.existsSync(resolve(cwd, 'attempt.json'))).toBe(true);
      if (mode === 'failure') expect(fs.readFileSync(resolve(cwd, 'stderr.log'), 'utf8')).toContain('test uploader failed');
      expect(fs.existsSync(resolve(cwd, 'verified.json'))).toBe(false);
      fs.rmSync(cwd, { recursive: true, force: true });
    }
  });
});

test('failed deployment and dry run never invoke transport or create export output', async () => {
  await withFakeCli(async ({ fs, resolve, dir, capture }) => {
    const { deploymentOutput } = await import('./deploy-output.mjs');
    const opts = { yes: true, export: resolve(dir, 'export'), zephyrConfig: resolve(dir, 'missing.json') };
    // Failed builds throw before this boundary. Failed stream publication also stops here.
    expect(await deploymentOutput({ opts, failed: ['build failed'] })).toBeUndefined();
    expect(await deploymentOutput({ opts, refused: [{ kind: 'stream', reason: 'Incompatible cohort' }] })).toBeUndefined();
    expect(await deploymentOutput({ opts: { ...opts, yes: false } })).toBeUndefined();
    expect(fs.existsSync(capture)).toBe(false);
    expect(fs.existsSync(opts.export)).toBe(false);
  });
});

test('strict policy remains optional and requires an actual inspector', async () => {
  await expect(publishSnapshot({ ...fixture(), site: { org: 'org', project: 'project', app: 'app' }, strictPolicy: true })).rejects.toThrow('real control hooks');
});

test('an application cannot overwrite the transport marker', async () => {
  const f = fixture(), path = '.exact/root/web/exact.json';
  const root = JSON.parse(f.files.get(path)), bytes = Buffer.from('application content');
  const name = 'exact-zephyr-upload-id.txt';
  root.files.push({ name, bytes: bytes.length, sha256: sha256(bytes), sourceSha256: sha256(bytes) });
  f.files.set(path, Buffer.from(JSON.stringify(root)));
  f.files.set(`.exact/root/web/releases/${root.id}/${name}`, bytes);
  let calls = 0;
  await expect(publishSnapshot({ ...f, upload: () => { calls++; } })).rejects.toThrow('reserved');
  expect(calls).toBe(0);
  expect(f.files.get(`.exact/root/web/releases/${root.id}/${name}`)).toEqual(bytes);
});

test('interrupted publication does not prevent export or discard completed generations', async () => {
  const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { resolve } = await import('node:path');
  const { DirectoryOrigin } = await import('../../scripts/origin.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-zephyr-interrupted-'));
  try {
    const origin = new DirectoryOrigin(resolve(dir, 'origin')), web = resolve(dir, 'web');
    mkdirSync(web);
    writeFileSync(resolve(web, 'index.html'), '<h1>First</h1>');
    const first = await publishRoot({ origin, row: {}, web });
    writeFileSync(resolve(web, 'index.html'), '<h1>Incomplete</h1>');
    writeFileSync(resolve(web, 'z.js'), 'console.log("incomplete")');
    const put = origin.put.bind(origin);
    origin.put = async (path, ...args) => {
      if (path.endsWith('/z.js')) throw new Error('injected payload write refusal');
      return put(path, ...args);
    };
    await expect(publishRoot({ origin, row: {}, web })).rejects.toThrow('injected');
    const before = await collectSnapshot(origin, {});
    expect(before.get('index.html').toString()).toContain('First');
    expect([...before.keys()].some(path => path.endsWith('/z.js'))).toBe(false);
    origin.put = put;
    writeFileSync(resolve(web, 'index.html'), '<h1>Third</h1>');
    await publishRoot({ origin, row: {}, web });
    // A new native publisher can create its lock before it publishes a head.
    mkdirSync(resolve(origin.dir, '.exact/prod/pending'), { recursive: true });
    writeFileSync(resolve(origin.dir, '.exact/prod/pending/.lock'), '');
    const after = await collectSnapshot(origin, {});
    expect(after.get('index.html').toString()).toContain('Third');
    expect(after.get(`.exact/root/web/releases/${first.root}/index.html`)).toEqual(before.get('index.html'));
    expect([...after.keys()].some(path => path.includes('/pending/'))).toBe(false);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('normal deploy stops before CLI invocation when its compiler build fails', async () => {
  await withFakeCli(async ({ fs, resolve, dir, capture }) => {
    const { spawnSync } = await import('node:child_process');
    const app = resolve(dir, 'app'), bin = resolve(dir, 'bin');
    fs.mkdirSync(app);
    fs.writeFileSync(resolve(app, 'app.contract'), 'component App\n  view\n    text "Test"\n');
    fs.writeFileSync(resolve(app, 'app.json'), JSON.stringify({ name: 'Test', app: { id: 'test.failed-build', name: 'Test' }, host: { web: {} } }));
    for (const args of [['init', '-q'], ['add', '.']]) {
      expect(spawnSync('git', args, { cwd: app }).status).toBe(0);
    }
    // The publisher must stop on a toolchain failure, regardless of CLI availability.
    const realCargo = Bun.which('cargo');
    fs.writeFileSync(resolve(bin, 'cargo'), `#!${process.execPath}\nimport {spawnSync} from 'node:child_process';
if(process.argv[2]==='build'){console.error('test compiler build failed');process.exit(19);}
const result=spawnSync(${JSON.stringify(realCargo)},process.argv.slice(2),{stdio:'inherit'});process.exit(result.status??1);
`, { mode: 0o700 });
    const config = resolve(dir, 'site.json');
    fs.writeFileSync(config, JSON.stringify({ org: 'org', project: 'project', app: 'app' }));
    const result = spawnSync(process.execPath, [resolve(import.meta.dir, '../../scripts/deploy.mjs'), app,
      '--dirty', '--yes', '--origin', resolve(dir, 'origin'), '--export', resolve(dir, 'export'), '--zephyr-config', config], {
      cwd: resolve(import.meta.dir, '../..'), env: { ...process.env, EXACT_APP_DIR: app }, encoding: 'utf8',
    });
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain('test compiler build failed');
    expect(fs.existsSync(capture)).toBe(false);
    expect(fs.existsSync(resolve(dir, 'export'))).toBe(false);
  });
});
