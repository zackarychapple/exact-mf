import { resolve, relative, isAbsolute } from 'node:path';
import { pathToFileURL } from 'node:url';
import { openOrigin } from '../../scripts/origin.mjs';
import { exportSnapshot } from './snapshot.mjs';
import { publishSnapshot, readSiteConfig } from './index.mjs';

// This boundary runs only after the build and all selected streams succeeded.
export async function deploymentOutput({ opts, origin, app, failed = [], refused = [] }) {
  if (!opts.yes || failed.length || refused.some(row => row.kind !== 'binary') || !opts.export) return undefined;
  let site;
  if (opts.zephyrConfig) {
    const rel = relative(resolve(opts.export), resolve(opts.zephyrConfig));
    if (!rel || !isAbsolute(rel) && rel !== '..' && !rel.startsWith('../')) throw new Error('Keep Zephyr configuration outside the public snapshot');
    site = readSiteConfig(opts.zephyrConfig);
  }
  const exported = await exportSnapshot(origin, app, opts.export);
  const frozen = openOrigin(exported.directory);
  if (site) exported.transport = await publishSnapshot({ origin: frozen, app, site });
  else if (opts.transport) {
    const transport = await import(pathToFileURL(opts.transport).href);
    if (typeof transport.default !== 'function') throw new Error('--transport module must export a default function');
    exported.transport = await transport.default({ origin: frozen, app, snapshot: exported });
  }
  return exported;
}
