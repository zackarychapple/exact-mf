#!/bin/sh
':' //; if [ -n "$BUN" ]; then exec "$BUN" "$0" "$@"; elif [ -x "$HOME/.bun-1.4.2/bin/bun" ]; then exec "$HOME/.bun-1.4.2/bin/bun" "$0" "$@"; else exec bun "$0" "$@"; fi
// All native and script dependencies use the checkout selected by tools/configure-exact.mjs.
import { configureExact } from './tools/configure-exact.mjs';
import { resolve } from 'node:path';
const EXACT2 = configureExact();
const cli = resolve(EXACT2, 'scripts/app-cli.mjs');
const { ensureBun } = await import(resolve(EXACT2, 'scripts/bun.mjs'));
ensureBun("1.4.2");
const { run } = await import(cli);
process.exit(await run({ dir: import.meta.dir, name: "exact-mf-cloud-sol-app", exact2: EXACT2, args: process.argv.slice(2) }));
