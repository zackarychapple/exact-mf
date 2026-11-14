import { readFileSync } from 'node:fs';

export const isHtml = name => /\.html?$/i.test(name);
export function hostImportMap(site) {
  for (const key of ['org', 'project', 'app']) {
    if (typeof site?.[key] !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(site[key])) throw new Error(`invalid site ${key}`);
  }
  return `<script type="importmap">${JSON.stringify({ imports: { [`env:vars:${site.app}.${site.project}.${site.org}`]: '/zephyr-manifest.json' } })}</script>`;
}

// Match the observed worker algorithm. Refuse authored maps instead of deleting them.
export function prepareHtml(bytes, site) {
  const map = hostImportMap(site);
  const html = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
  if (html.startsWith('\uFEFF') || html.includes('\0') || /charset\s*=\s*["']?(?!utf-8(?:["'\s;>]|$))[^"'\s;>]+/i.test(html)) throw new Error('Zephyr HTML requires UTF-8 encoding');
  const maps = html.match(/<script\b[^>]*\btype\s*=\s*["']?importmap["']?[^>]*>[\s\S]*?<\/script\s*>/gi) ?? [];
  const starts = html.match(/<script\b[^>]*\btype\s*=\s*["']?importmap(?:["'\s>])/gi) ?? [];
  if (starts.length !== maps.length || maps.some(value => value !== map) || maps.length > 1) throw new Error('Zephyr preparation refuses unrelated or duplicate import maps');
  const clean = html.replace(/<script[^>]*type=["']?importmap["']?[^>]*>[\s\S]*?<\/script>/gi, '');
  const index = clean.indexOf('<script type="module"');
  if (index < 0 && !clean.includes('</head>')) throw new Error('Zephyr HTML requires an exact module script insertion point or </head>');
  return Buffer.from(index < 0 ? clean.replace('</head>', `${map}</head>`) : clean.slice(0, index) + map + clean.slice(index));
}

export function prepareWebBytes(web, name, bytes, site) {
  if (!site || !isHtml(name)) return bytes;
  let envelope;
  try { envelope = JSON.parse(readFileSync(`${web}/exact.json`, 'utf8')); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  if (envelope?.assets?.some(card => card.name === name)) throw new Error(`Zephyr preparation refuses HTML bundle asset: ${name}`);
  return prepareHtml(bytes, site);
}

export function checkPreparedSnapshot(files, site) {
  hostImportMap(site);
  if (files.has('zephyr-manifest.json')) throw new Error('The app uses the reserved Zephyr runtime manifest path');
  for (const [name, bytes] of files) {
    // Opaque native blobs have digest paths, regardless of their original asset names.
    if (!isHtml(name) || name.startsWith('.exact/blobs/')) continue;
    let prepared;
    try { prepared = prepareHtml(bytes, site); } catch (error) {
      throw new Error(`Zephyr retained HTML is incompatible: ${name}. Use a new retained origin prepared for this site. Keep existing history. ${error.message}`);
    }
    if (!prepared.equals(bytes)) throw new Error(`Zephyr retained HTML is unprepared: ${name}. Use a new retained origin prepared for this site. Keep existing history.`);
  }
}
