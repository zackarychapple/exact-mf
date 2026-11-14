// Optional native artifact build. This module never loads package code in the build process.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, join, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { build } from 'rolldown';
import { parseSync, transformSync } from 'rolldown/utils';
import { packageRefusals } from './typescript.mjs';

const inputs = {
  "error-codes/dist/browser.mjs": "fa8e3dd0080559576759e3f2484b57559be26abf7e4a96b1add1fa9fe9005044",
  "error-codes/dist/desc.mjs": "a81578d5b6e3f8528d3dac5b0a10ab2c2bdbd6b49cab7367b17d127cbfb2c038",
  "error-codes/dist/error-codes.mjs": "9b30320861e1d45b65e52a1a79f72f6d2f64638896d304b056f824d514b98801",
  "error-codes/dist/getShortErrorMsg.mjs": "ba61b21da251d790c653f72b58eaeef20d05896c9b475926e0ac289fa8f47443",
  "error-codes/dist/index.mjs": "d4057743e7fc58fc2daef7445a64399d3e4bc5cc317e38f7fb20f82a1e6a3cdb",
  "runtime-core/dist/_virtual/_rolldown/runtime.js": "7e6526a114855257a154435d2fd11f4520dafc98dd87251e81001a94442b3079",
  "runtime-core/dist/constant.js": "fab4384d293f56d367b6450135bf47850b8fa23878942b49d7468475ca3bc6fa",
  "runtime-core/dist/core.js": "921738cb850506fe5555b5edb1986a10189304e6f9cdecea5466aa4bad2c119d",
  "runtime-core/dist/global.js": "85ab404786ebea29385bc0db981a074550801e517decbe1c6e22cd871d3f3c8d",
  "runtime-core/dist/helpers.js": "c8d9b55ca792b2bacfd78f88bfbe4285b1637f1137fb857f03a9c3c7cae4d2c2",
  "runtime-core/dist/index.js": "835a1ff7ada96dee00f55b93ab2f6175b581769059c87abe5b15140be1a8583f",
  "runtime-core/dist/module/index.js": "267648ad631de5a449ab9d524511088c52ac6c5d77732c7cb890164ae3ed8c96",
  "runtime-core/dist/plugins/generate-preload-assets.js": "0bc1fe94c85331f7621a117a328c5f8cbf7c9f3b7f1a1930cf49b371995baa24",
  "runtime-core/dist/plugins/snapshot/SnapshotHandler.js": "75b26f07988459b52bcd089580aa8abd302878ff27e08d62f47b4292cf6cb22c",
  "runtime-core/dist/plugins/snapshot/disabled.js": "aa72b6da89c86fd83e0528234a537f519d146fe1bced7c510c89530904228ce7",
  "runtime-core/dist/plugins/snapshot/index.js": "a5ecddf536c4ed13572fa6ecd1040f259f0d49c7b89b2d8d3133c57b04e36923",
  "runtime-core/dist/remote/disabled.js": "60d9c0377e06a237f4b1fc25f3ef23dd6dea5eb1de542e5a38b70067a301fc68",
  "runtime-core/dist/remote/index.js": "9eab37772eb0cfad4d1ce39893b6abf44837e1ae2b0598e9e7a68efd7665527d",
  "runtime-core/dist/shared/disabled.js": "462c60eb52c2128ab97495571fcd4cce2b40ba9cac5e5bc10f83fdb72ce2d7ad",
  "runtime-core/dist/shared/index.js": "cc481df356a0c6a53fc4104ef31a3674b6b938ed3f3bc037fcd0809490494a82",
  "runtime-core/dist/type/index.js": "ea42cf29e3555ccd95d94646d0bcb4124c71da7e30184dc36fccb372bbe880eb",
  "runtime-core/dist/utils/context.js": "775b240555a4ecfc70c095daa3b68706e64db5094a4e6e162e83addd8c167a15",
  "runtime-core/dist/utils/env.js": "2b8f47aa9dac378cb0e29a9b98ec8f774cf246e4181f6e51e53acd6c955638e3",
  "runtime-core/dist/utils/hooks/asyncHook.js": "57f20ad7cb50025664c8224356696a7e4f54f02383d73111af794d98a8a004e9",
  "runtime-core/dist/utils/hooks/asyncWaterfallHooks.js": "13b015eee8e08209765d593db2e84e0a31a6e9fa85b53d37e562007b945b8dc2",
  "runtime-core/dist/utils/hooks/index.js": "adf785e9e770a2e4e20478fd589b9c8c27a5b35cdd74a875f14062bda052731e",
  "runtime-core/dist/utils/hooks/pluginSystem.js": "e2bbd43aa4735fee463c9fb63f7bd522b8364c677df6043821b60c8134b317d4",
  "runtime-core/dist/utils/hooks/syncHook.js": "c5de6221afecd720346b740d2555c3198465bf394aaf695af109ef21edd16bc2",
  "runtime-core/dist/utils/hooks/syncWaterfallHook.js": "039725a07f5aade6fdbd733e5a3dff223adc050a2ee5064794eb0d62048b6c51",
  "runtime-core/dist/utils/index.js": "9d3512e169f516f16ffaade608a362d0cfafeec1a32a14ab474f782c8b2d9ed6",
  "runtime-core/dist/utils/load.js": "5a7989b6288784f8ec6318f240e5d75335757b0929f2582016adbb575003a464",
  "runtime-core/dist/utils/logger.js": "9559be12b2081900724a57a3fb63a2a92e36cc10cc6931cf7a56b49c2444ce15",
  "runtime-core/dist/utils/manifest.js": "3ae3702c0a93840e76cdec478f00a636ede6a6ef34b3cf2e9d6e585cf3b3af89",
  "runtime-core/dist/utils/plugin.js": "992849e4b0f313c1ee7225b9baf461738f24a3b9b4a66b1fec8e6fbfd9446019",
  "runtime-core/dist/utils/preload.js": "195d4ee89d93f554c680617853a16502c422ff16f77685cfde12c76e6123b55d",
  "runtime-core/dist/utils/semver/compare.js": "6d6f81e76845703d059b43d161a44fadea412595777949103a15235f74e2a467",
  "runtime-core/dist/utils/semver/constants.js": "908346c453ceb511d836435361d747617af9a7f1cf635caa4800153c803bb171",
  "runtime-core/dist/utils/semver/index.js": "1578363703f79ffc5d0ad8a64b509ad72dc9971fbc7d751e2915359168a4544d",
  "runtime-core/dist/utils/semver/parser.js": "0424623e46f09df3553d118a3e478c540947f481c2171770d1b5bd2dc30e9fd3",
  "runtime-core/dist/utils/semver/utils.js": "04e800507849885b725c00c70b2eafcf9a21ba2d2bf934ccf9c1b289499225de",
  "runtime-core/dist/utils/share.js": "48607db0a0d255ddd127003dd4b3ce5ca92df2d9240731b3d9b7c973596f5689",
  "runtime-core/dist/utils/tool.js": "e2cad915afdbbe5137d4a14449084ae3375be9b6d385bc32cf5ec0141caa2daa",
  "sdk/dist/_virtual/_rolldown/runtime.js": "7e6526a114855257a154435d2fd11f4520dafc98dd87251e81001a94442b3079",
  "sdk/dist/constant.js": "283d1f7f05a5faea6d053b682c83ad3e0a045e222f2959ef10a2e4e2c9ec2663",
  "sdk/dist/createModuleFederationConfig.js": "b5a81ff2601193f2404a4af5bcf36244a17849462c2b3b39ad6d492cb9ec7507",
  "sdk/dist/dom.js": "8c4c0bc909d3c326c900da95ae4f227c44b459a634f775a9ed8cd84be6fbd39e",
  "sdk/dist/env.js": "66f39336b0034990ad86df834eabb966ad04b28545ff17cdf573906e0762543b",
  "sdk/dist/generateSnapshotFromManifest.js": "0da996cae191a02e8726ed9e97886db0590258a1db266a2ed426fa6c2722ed14",
  "sdk/dist/index.js": "a04e7a5ab9bd39b6a7132d1a5a5fc12a2b84a9cf7b6a93923dbccef9880f77b1",
  "sdk/dist/logger.js": "85da997c6e8f4fb467631a306f3fa8561c1b8c71e28e9a5f970be5e895f3cb2b",
  "sdk/dist/node.js": "7ee31c7b27bf07020f334a261886dd289686cc8f1f49a87d022ddad8de8e3ac3",
  "sdk/dist/normalizeOptions.js": "a44a5950b99a3648952b36ac7ac747acd4f95056acc0d8b6d22c952c16101b09",
  "sdk/dist/types/plugins/ConsumeSharedPlugin.js": "bae3fd93a85533bff03508edfbbf3f7a4749449a6d8a70091a5e24aaa00a0584",
  "sdk/dist/types/plugins/ContainerPlugin.js": "5a4d562813f0e7c3eadc83a7efde5d4165764c9b522b05e4b0af2a3c0bfe8838",
  "sdk/dist/types/plugins/ContainerReferencePlugin.js": "6351f79050ef5d3ef6e3a95156cda5b37072e2ef52221e844cd03c85204f9d45",
  "sdk/dist/types/plugins/ModuleFederationPlugin.js": "4730ea16c6bbcc2eba0c088df6a6f965f36f30ecfedaba8112ad55137717af8e",
  "sdk/dist/types/plugins/ProvideSharedPlugin.js": "a0051de552686242084d49e42451d6d45c91db88a52682775a3bd629f9a5e12d",
  "sdk/dist/types/plugins/SharePlugin.js": "ee91fdd1e455bbd09c340c536d308ba82033139206eb5e68427f0d1dbfd17258",
  "sdk/dist/utils.js": "aa4103b72fe626c295999cae26e859c1b834d569f5e16ef9d5b9d3ee133fd3bd"
};

/** Build a separate native MF artifact from installed, pinned upstream packages.
 * The caller owns loading it after first pixel and supplying the loadEntry plugin.
 * Browser builds must use the stock package. Cross-generation singletons are unsupported.
 * No package is installed or evaluated here. The app's dependency lock owns installation.
 */
export async function buildNativeFederation({ input, file }) {
  if (!input || !file) throw Error('native federation requires input and file');
  const result = await build({
    input, write: false, platform: 'browser', plugins: [nativeFederation()],
    transform: { define: { 'process.env.NODE_ENV': '"production"' } },
    output: { format: 'iife', codeSplitting: false }
  });
  if (result.output.length !== 1 || result.output[0].type !== 'chunk')
    throw Error('native federation requires one JavaScript artifact');
  const code = result.output[0].code;
  const errors = packageRefusals(code, at => `native federation${at}`, (s, o) => parseSync('native.js', s, o));
  if (errors.length) throw Error(errors.join('\n'));
  writeFileSync(file, code);
}

const packageName = /^@module-federation\/(runtime-core|sdk|error-codes)$/;
const packageImport = /^(@module-federation\/(?:runtime-core|sdk|error-codes))(?:\/|$)/;

function nearestPackage(id) {
  if (!isAbsolute(id)) return null;
  for (let root = dirname(id); ; root = dirname(root)) {
    const path = join(root, 'package.json');
    if (existsSync(path)) return { root, manifest: JSON.parse(readFileSync(path, 'utf8')) };
    if (dirname(root) === root) return null;
  }
}

function verifyPackage(root, name) {
  const manifest = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  if (manifest.name !== name || manifest.version !== '2.9.3')
    throw Error('unsupported native federation version: ' + manifest.name + '@' + manifest.version);
}

function within(root, id) {
  if (!isAbsolute(id)) return false;
  const path = relative(root, id).replaceAll('\\', '/');
  return path !== '..' && !path.startsWith('../') && !isAbsolute(path);
}

function ownerOf(roots, id) {
  return [...roots].filter(([root]) => within(root, id))
    .sort(([a], [b]) => b.length - a.length)[0];
}

/** Exact 2.9.3 specialization. Every reached MF file must match its reviewed bytes. */
export function nativeFederation() {
  let reached = false;
  const roots = new Map();
  return {
    name: 'exact-native-federation',
    buildStart() {
      reached = false;
      roots.clear();
    },
    async resolveId(source, importer) {
      const name = packageImport.exec(source)?.[1];
      const parent = importer && ownerOf(roots, importer);
      if (!name && !(parent && source.startsWith('.'))) return null;
      const resolved = await this.resolve(source, importer, { skipSelf: true });
      if (!name) {
        if (!resolved || resolved.external || !within(parent[0], resolved.id))
          throw Error('unsupported native federation path: ' + source);
        return resolved;
      }
      const owner = resolved && !resolved.external && nearestPackage(resolved.id);
      if (!owner) throw Error('native federation package has no manifest: ' + source);
      verifyPackage(owner.root, name);
      roots.set(owner.root, name);
      return resolved;
    },
    transform(code, id) {
      const nearest = nearestPackage(id);
      if (nearest && packageName.test(nearest.manifest.name))
        roots.set(nearest.root, nearest.manifest.name);
      // Keep the package identity even if a nested manifest changes its claimed name.
      const owner = ownerOf(roots, id);
      if (!owner) return null;
      const [root, name] = owner;
      verifyPackage(root, name);
      const key = name.slice('@module-federation/'.length) + '/' + relative(root, id).replaceAll('\\', '/');
      const digest = createHash('sha256').update(code).digest('hex');
      if (inputs[key] !== digest) throw Error('unsupported native federation input: ' + key);
      if (name === '@module-federation/runtime-core') reached = true;
      const result = transformSync(id, specialize(code, key), { define: { ENV_TARGET: '"native"' } });
      if (result.errors.length) throw Error('native federation transform failed: ' + key);
      return result.code;
    },
    buildEnd(error) {
      if (!error && !reached) throw Error('native federation input does not reach runtime-core');
    }
  };
}

function specialize(code, key) {
  if (key === 'sdk/dist/node.js')
    return 'export function createScriptNode(){throw Error("Native script loading unavailable");} export function loadScriptNode(){throw Error("Native script loading unavailable");}';
  if (key === 'runtime-core/dist/remote/index.js') {
    const line = 'if (isBrowserEnvValue && typeof window !== "undefined" && !remote.entry.startsWith("http")) remote.entry = new URL(remote.entry, window.location.origin).href;';
    if (!code.includes(line)) throw Error('URL seam changed');
    code = code.replace(line, '');
  }
  if (key === 'runtime-core/dist/global.js') {
    const current = 'const CurrentGlobal = typeof globalThis === "object" ? globalThis : window;';
    if (!code.includes(current)) throw Error('global seam changed');
    code = code.replace(current, 'const CurrentGlobal = globalThis;');
    const start = code.indexOf('const nativeGlobal =');
    const end = code.indexOf('const Global =', start);
    if (start < 0 || end < 0) throw Error('global seam changed');
    code = code.slice(0, start) + 'const nativeGlobal = globalThis;\n' + code.slice(end);
  }
  if (key === 'runtime-core/dist/plugins/generate-preload-assets.js') {
    const start = code.indexOf('const isExisted =');
    const end = code.indexOf('};', start);
    if (start < 0 || end < 0) throw Error('preload seam changed');
    code = code.slice(0, start) + 'const isExisted = () => {throw Error("Native preload unavailable");};' + code.slice(end + 2);
  }
  const names = {
    'runtime-core/dist/utils/load.js': ['loadEsmEntry', 'loadSystemJsEntry', 'loadEntryNode', 'loadEntryDom'],
    'sdk/dist/utils.js': ['getResourceUrl'],
    'sdk/dist/dom.js': ['createScript', 'createLink', 'loadScript'],
    'runtime-core/dist/utils/preload.js': ['waitForLinkPreload', 'waitForScriptPreload'],
    'sdk/dist/env.js': ['isBrowserDebug']
  }[key];
  if (!names) return code;
  const ast = parseSync(key, code).program;
  const edits = ast.body.filter(n => n.type === 'FunctionDeclaration' && names.includes(n.id.name));
  if (edits.length !== names.length) throw Error('transform shape changed: ' + key);
  for (const n of edits.reverse()) {
    const body = n.id.name === 'isBrowserDebug' ? 'return false;' : `throw Error('Unsupported native fallback: ${n.id.name}');`;
    code = code.slice(0, n.start) + `${n.async ? 'async ' : ''}function ${n.id.name}(){ ${body} }` + code.slice(n.end);
  }
  return code;
}
