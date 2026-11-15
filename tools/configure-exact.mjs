import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, readFileSync, realpathSync, renameSync, symlinkSync, unlinkSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export function configureExact({ appRoot = root, checkout = process.env.EXACT2 } = {}) {
  const link = resolve(appRoot, '.exact/runtime');
  const selected = resolve(appRoot, checkout || (existsSync(link) ? link : '../exact-runtime'));
  if (!existsSync(resolve(selected, 'scripts/app-cli.mjs'))) {
    throw Error(`No Exact checkout at ${selected}. Clone the exact-runtime branch and set EXACT2 to its directory.`);
  }
  const expected = readFileSync(resolve(appRoot, 'EXACT_REVISION'), 'utf8').trim();
  if (!/^[a-f0-9]{40}$/.test(expected)) throw Error('EXACT_REVISION must contain one full Git commit.');
  const actual = execFileSync('git', ['-C', selected, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  if (actual !== expected) throw Error(`Exact revision mismatch: expected ${expected}, found ${actual} at ${selected}.`);
  const target = realpathSync(selected);
  const existing = lstatSync(link, { throwIfNoEntry: false });
  if (existing && !existing.isSymbolicLink()) throw Error(`${link} must be a symlink; refusing to replace it.`);
  if (!existing || !existsSync(link) || realpathSync(link) !== target) {
    mkdirSync(dirname(link), { recursive: true });
    const temporary = `${link}.${process.pid}`;
    try {
      symlinkSync(target, temporary, 'dir');
      renameSync(temporary, link);
    } finally {
      if (lstatSync(temporary, { throwIfNoEntry: false })) unlinkSync(temporary);
    }
  }
  return target;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  console.log(configureExact());
}
