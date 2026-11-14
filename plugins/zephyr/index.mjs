import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
import { randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { sha256 } from '../../scripts/origin.mjs';
import { collectSnapshot } from './snapshot.mjs';

export function readSiteConfig(path) {
  const site = JSON.parse(readFileSync(path, 'utf8'));
  if (!site || typeof site !== 'object' || Array.isArray(site)
    || Object.keys(site).some(key => !['org', 'project', 'app'].includes(key))) throw new Error('Zephyr config accepts only org, project and app');
  validateSite(site);
  return site;
}
function validateSite(site) {
  for (const key of ['org', 'project', 'app']) if (!/^[a-z0-9][a-z0-9-]*$/.test(site?.[key] ?? '')) throw new Error(`invalid site ${key}`);
}
async function fetchBytes(url) {
  const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(30000), headers: { 'User-Agent': 'Exact-Zephyr-verification' } });
  if (!response.ok) throw new Error(`verification HTTP ${response.status}: ${url}`);
  return Buffer.from(await response.arrayBuffer());
}

// Zephyr can apply configured rules during upload. Only strictPolicy requests
// pre-upload rule inspection. Explicit promotion always follows verification.
export async function publishSnapshot({ origin, app, site, control, dryRun = false, promote = false,
  strictPolicy = false, upload = cliUpload, fetchFile = fetchBytes }) {
  const files = await collectSnapshot(origin, app);
  if (files.has('exact-zephyr-upload-id.txt')) throw new Error('The app uses the reserved Zephyr upload marker path');
  if (dryRun) return { dryRun: true, files: files.size };
  if (process.env.ZE_SECRET_TOKEN) throw new Error('Unset ZE_SECRET_TOKEN and use the normal Zephyr user session');
  validateSite(site);
  let before;
  if (strictPolicy || promote) {
    if (typeof control?.inspect !== 'function' || promote && (typeof control.resolveBuild !== 'function' || typeof control.promote !== 'function')) throw new Error('Explicit promotion and strict policy require real control hooks');
    before = await control.inspect(site);
    if (!before.identityMatches || strictPolicy && (!before.noAutomaticPromotion || process.env.ZE_ENV)) throw new Error('Upload policy inspection failed');
  }
  const uploadId = randomUUID();
  files.set('exact-zephyr-upload-id.txt', Buffer.from(uploadId));
  let result;
  try {
    result = await upload(files, site, uploadId);
    const build = control?.resolveBuild ? await control.resolveBuild(site, result) : result;
    if (!build || build.applicationUid !== `${site.app}.${site.project}.${site.org}` || result.versionUrl && build.versionUrl !== result.versionUrl || promote && !build.versionId) throw new Error('Uploaded build identity does not match site configuration');
    const base = new URL(build.versionUrl);
    if (base.protocol !== 'https:' || base.username || base.password || base.pathname !== '/' || base.search || base.hash) throw new Error('Expected an immutable HTTPS build URL');
    // Check the unique marker first. Identical app bytes alone cannot identify a build.
    const marker = await fetchFile(new URL('exact-zephyr-upload-id.txt', base).href);
    if (!Buffer.from(marker).equals(Buffer.from(uploadId))) throw new Error('Uploaded snapshot marker verification failed');
    for (const [path, bytes] of files) {
      const received = await fetchFile(new URL(path, base).href);
      if (!received || sha256(received) !== sha256(bytes)) throw new Error(`Uploaded snapshot verification failed: ${path}`);
    }
    if (promote) {
      const now = await control.inspect(site);
      if (!now.identityMatches || strictPolicy && !now.noAutomaticPromotion || now.versionId !== before.versionId) throw new Error('Environment changed during upload');
      await control.promote(site, { versionId: build.versionId, expectedVersionId: before.versionId });
    }
    if (result.evidenceDirectory) {
      writeFileSync(resolve(result.evidenceDirectory, 'verified.json'), JSON.stringify({ ...build, uploadId, verified: true }) + '\n', { mode: 0o600 });
      rmSync(resolve(result.evidenceDirectory, 'dist'), { recursive: true, force: true });
    }
    return { ...build, uploadId, verified: true, promotionRequested: promote };
  } catch (error) {
    if (result?.evidenceDirectory) error.message += `; upload evidence: ${result.evidenceDirectory}. Inspect this build before retrying.`;
    throw error;
  }
}

export function parseCliVersionUrl(output) {
  const plain = output.replace(/\x1b\[[0-9;]*m/g, '');
  const urls = [...new Set([...plain.matchAll(/Deployed to Zephyr's edge in \d+ms\.\s*(https:\/\/[^\s]+)/g)].map(match => match[1]))];
  if (urls.length !== 1) throw new Error('CLI returned no unambiguous immutable version URL');
  return urls[0];
}

export async function cliUpload(files, site, uploadId) {
  if (process.env.ZE_SECRET_TOKEN) throw new Error('Use the normal Zephyr user session without ZE_SECRET_TOKEN');
  validateSite(site);
  const stage = mkdtempSync(resolve(tmpdir(), 'exact-zephyr-'));
  mkdirSync(resolve(stage, 'dist'));
  for (const [path, bytes] of files) {
    const target = resolve(stage, 'dist', path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, bytes, { flag: 'wx' });
  }
  writeFileSync(resolve(stage, 'package.json'), JSON.stringify({ name: site.app, version: '0.0.0', private: true }));
  writeFileSync(resolve(stage, 'zephyr.config.mjs'), `export default ${JSON.stringify({ org: site.org, project: site.project, appName: site.app })};\n`);
  const result = spawnSync('pnpm', ['--package=zephyr-cli@1.4.1', 'dlx', 'ze-cli', 'deploy', './dist', '--target', 'web'], {
    cwd: stage, env: { ...process.env, ZE_FAIL_BUILD: 'true' }, encoding: 'utf8', maxBuffer: 16 << 20,
  });
  writeFileSync(resolve(stage, 'stdout.log'), result.stdout ?? '', { mode: 0o600 });
  writeFileSync(resolve(stage, 'stderr.log'), result.stderr ?? '', { mode: 0o600 });
  writeFileSync(resolve(stage, 'attempt.json'), JSON.stringify({ site, uploadId, status: result.status, signal: result.signal, error: result.error?.message }) + '\n', { mode: 0o600 });
  try {
    if (result.status !== 0 || result.error) throw new Error('Zephyr CLI upload failed');
    const versionUrl = parseCliVersionUrl(`${result.stdout}\n${result.stderr}`);
    const receipt = { applicationUid: `${site.app}.${site.project}.${site.org}`, uploadId, versionUrl, evidenceDirectory: stage };
    writeFileSync(resolve(stage, 'receipt.json'), JSON.stringify(receipt) + '\n', { mode: 0o600 });
    return receipt;
  } catch (error) {
    throw new Error(`${error.message}; upload evidence: ${stage}. Inspect build history before retrying.`);
  }
}
