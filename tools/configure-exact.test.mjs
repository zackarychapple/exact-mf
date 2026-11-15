import { test, expect } from 'bun:test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readlinkSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { configureExact } from './configure-exact.mjs';

test('checkout selection handles spaces and refuses revision drift without replacing the link', () => {
  const root = mkdtempSync(join(tmpdir(), 'exact checkout '));
  const appRoot = join(root, 'app');
  const checkout = join(root, 'runtime');
  try {
    mkdirSync(appRoot);
    mkdirSync(join(checkout, 'scripts'), { recursive: true });
    writeFileSync(join(checkout, 'scripts/app-cli.mjs'), 'export const run = () => 0;');
    const git = (...args) => execFileSync('git', ['-C', checkout, ...args], { encoding: 'utf8' }).trim();
    git('init', '-q');
    git('add', '.');
    git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture');
    writeFileSync(join(appRoot, 'EXACT_REVISION'), git('rev-parse', 'HEAD') + '\n');
    expect(configureExact({ appRoot, checkout: '../runtime' })).toBe(realpathSync(checkout));
    expect(configureExact({ appRoot, checkout: '' })).toBe(realpathSync(checkout));
    const before = readlinkSync(join(appRoot, '.exact/runtime'));
    writeFileSync(join(appRoot, 'EXACT_REVISION'), '0'.repeat(40));
    expect(() => configureExact({ appRoot, checkout })).toThrow('revision mismatch');
    expect(readlinkSync(join(appRoot, '.exact/runtime'))).toBe(before);
    expect(() => configureExact({ appRoot, checkout: '../missing' })).toThrow('No Exact checkout');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
