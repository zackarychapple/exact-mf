import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
import { randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { sha256 } from '../../scripts/origin.mjs';
import { collectSnapshot } from './snapshot.mjs';

// The control-plane hooks use the caller's normal authenticated Zephyr session.
// Promotion is separate because Zephyr does not implement origin conditional puts.
export async function publishSnapshot({ origin, app, site, control, dryRun = false, promote = false,
  upload = cliUpload, fetchFile = async (url) => {
    const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(30000), headers: { 'User-Agent': 'Exact-Zephyr-verification' } });
    if (!response.ok) throw new Error(`verification HTTP ${response.status}: ${url}`);
    return Buffer.from(await response.arrayBuffer());
  } }) {
  const files = await collectSnapshot(origin, app);
  if (dryRun) return { dryRun: true, files: files.size };
  if (process.env.ZE_ENV || process.env.ZE_SECRET_TOKEN) throw new Error('Unset ZE_ENV and ZE_SECRET_TOKEN before snapshot upload');
  for (const key of ['org', 'project', 'app']) if (!/^[a-z0-9][a-z0-9-]*$/.test(site[key] ?? '')) throw new Error(`invalid site ${key}`);
  const before = await control.inspect(site);
  if (!before.identityMatches || !before.noAutomaticPromotion) throw new Error('Upload requires matching user identity and no automatic deployment rules');
  const uploadId = randomUUID();
  files.set('exact-zephyr-upload-id.txt', Buffer.from(uploadId));
  const result = await upload(files, site, uploadId);
  const build = await control.resolveBuild(site, result);
  if (!build || build.applicationUid !== result.applicationUid || result.versionUrl && build.versionUrl !== result.versionUrl || !build?.versionId) throw new Error('Uploaded build identity does not match build history');
  const base = new URL(build.versionUrl);
  if (base.protocol !== 'https:' || base.username || base.password) throw new Error('Expected an immutable HTTPS build URL');
  for (const [path, bytes] of files) {
    const received = await fetchFile(new URL(path, base.href.replace(/\/?$/, '/')).href);
    if (sha256(received) !== sha256(bytes)) throw new Error(`Uploaded snapshot verification failed: ${path}`);
  }
  if (promote) {
    const now = await control.inspect(site);
    if (!now.identityMatches || !now.noAutomaticPromotion || now.versionId !== before.versionId) throw new Error('Environment changed during upload');
    await control.promote(site, { versionId: build.versionId, expectedVersionId: before.versionId });
  }
  return { ...build, verified: true, promotionRequested: promote };
}

export async function cliUpload(files, site, uploadId) {
  if (process.env.ZE_ENV || process.env.ZE_SECRET_TOKEN) throw new Error('Use the normal Zephyr user session without ZE_ENV or ZE_SECRET_TOKEN');
  const stage = mkdtempSync(resolve(tmpdir(), 'exact-zephyr-'));
  try {
    mkdirSync(resolve(stage, 'dist'));
    for (const [path, bytes] of files) {
      const target = resolve(stage, 'dist', path);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, bytes, { flag: 'wx' });
    }
    writeFileSync(resolve(stage, 'package.json'), JSON.stringify({ name: site.app, version: '0.0.0', private: true }));
    writeFileSync(resolve(stage, 'zephyr.config.mjs'), `export default ${JSON.stringify({ org: site.org, project: site.project, appName: site.app })};\n`);
    const env = { ...process.env, ZE_FAIL_BUILD: 'true' };
    delete env.ZE_ENV;
    const result = spawnSync('pnpm', ['--package=zephyr-cli@1.4.1', 'dlx', 'ze-cli', 'deploy', './dist', '--target', 'web'], { cwd: stage, env, encoding: 'utf8', maxBuffer: 16 << 20 });
    if (result.status !== 0) throw new Error('Zephyr upload failed. Inspect build history before retrying.');
    // CLI 1.4.1 has no structured deploy receipt. Resolve history by the
    // unique public marker, then verify every file at that immutable URL.
    return { applicationUid: `${site.app}.${site.project}.${site.org}`, uploadId };

  } finally { rmSync(stage, { recursive: true, force: true }); }
}
