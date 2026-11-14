# Zephyr transport for Exact

Exact signs updates. This plugin uploads complete public snapshots through normal Zephyr CLI authentication.
No runtime imports Zephyr or this plugin.

Create a private site configuration outside the public output:

```json
{"org":"zackarychapple","project":"exact-plugin-lifecycle","app":"signed-updates"}
```

Install pnpm and sign in through the CLI's normal user login. Then run:

```sh
bun scripts/deploy.mjs <app> \
  --origin /absolute/retained-origin \
  --export /absolute/new-snapshot \
  --zephyr-config /absolute/zephyr-site.json --yes --json
```

No JavaScript hook or control callback is required. Use the same retained origin for later generations.
Use a new export directory for each deployment. Keep signing keys and site configuration outside the origin and export.
Without `--yes`, Exact does not export or invoke the uploader.
A failed build, refused stream or incomplete signed graph stops the workflow before upload.

The snapshot materializes the current web root and retains public native heads, release records and referenced blobs.
Separate MF and Hermes blobs remain separate files. Exact verifies signatures, hashes and byte lengths before upload.
The exporter omits locks, unreferenced blobs and private file types.

The included transport runs:

```sh
ZE_FAIL_BUILD=true pnpm --package=zephyr-cli@1.4.1 dlx ze-cli deploy ./dist --target web
```

It stages only the selected public files. The package and Zephyr configuration remain outside `dist`.
The plugin uses the existing normal CLI user session. It does not read credentials or call private API endpoints.
`ZE_SECRET_TOKEN` is refused. Normal deployment rules and an explicit ambient `ZE_ENV` retain their CLI behavior.

Published CLI 1.4.1 has no structured deploy receipt. Its pinned agent prints the immutable URL after
“Deployed to Zephyr's edge in …ms.” The plugin requires one unambiguous completion URL.
It checks the exact unique `exact-zephyr-upload-id.txt` marker at that URL, then compares every uploaded file.
A different build with identical application bytes cannot pass the unique marker check.
A missing blob that returns fallback HTML fails the byte comparison.

The returned `transport` receipt includes `versionUrl`, `uploadId`, `verified` and `evidenceDirectory`.
CLI stdout, stderr and attempt metadata remain in that private directory. Failed or ambiguous attempts also retain the staged public files.
Verification success removes the temporary `dist` copy and writes `verified.json`. Original exports remain unchanged.
Inspect evidence and build history before retrying an ambiguous upload. The plugin never retries an upload automatically.
Evidence files can contain CLI authentication prompts. Keep the directory private and remove it when no longer needed.

Zephyr can apply configured tag and environment rules after uploading assets, before the CLI returns.
Local graph validation precedes upload. Immutable snapshot verification follows the CLI return.
Normal upload cannot promise verification before those automatic rules run. Use an isolated app when that ordering matters.
See [version URLs](https://docs.zephyr-cloud.io/features/versions) and [deployment rules](https://docs.zephyr-cloud.io/features/versioning-tags).

For explicit manual promotion, optional `publishSnapshot` control hooks remain available:

- `inspect(site)` returns `identityMatches` and the observed environment `versionId`.
- `resolveBuild(site, receipt)` locates the exact marked build and returns its `versionId`, `applicationUid` and `versionUrl`.
- `promote(site, {versionId, expectedVersionId})` requests promotion.

`promote: true` verifies the marker and every file before calling `control.promote`.
The observed-version check is not atomic compare-and-swap. Verify stable content after promotion.
`strictPolicy: true` additionally requires real inspection of `noAutomaticPromotion` and refuses ambient `ZE_ENV`.
Do not return constant policy approvals. This stronger mode requires caller-supplied inspection.

A custom hook can still use `--transport /absolute/module.mjs` instead of `--zephyr-config`.
Its default function receives `{origin, app, snapshot}`. The origin reads the frozen export directory.

Web publication now retains `.exact/root/web/inventories/<id>.json` for each generation.
The publisher writes this inventory after it verifies every payload. Export includes these completed generations and skips partial directories.
Use a fresh retained origin for this lifecycle.

An initial host binary can require separate distribution while its signed update bundle is safe to publish.
That binary notice does not block export. A refused stream or a failed publication still blocks transport.

With `--zephyr-config`, Exact reads the site identity once before classification and release generation.
It inserts the site's compact host import map into web HTML before computing release IDs and inventory hashes.
This preparation includes `.html` and `.htm` paths, without case sensitivity.
Exact refuses unrelated import maps, unsupported insertion points and HTML listed as a bundle asset.
Native plans and assets remain unchanged. Custom transports and unconfigured builds keep their existing transformation.

This preparation assumes the observed Zephyr worker behavior: remove import maps, then insert the host map before
the first exact `<script type="module"` substring, or before `</head>` when no such substring exists.
Only an exact existing host map is accepted. Preparation is idempotent.
The worker currently maps `env:vars:APP.PROJECT.ORG` to `/zephyr-manifest.json`.
If this service behavior changes, complete remote byte readback refuses the upload result.
Exact does not normalize verification bytes or modify frozen exports.

Before upload, Exact checks all retained web HTML for compatibility with the captured site identity.
Unprepared history or a changed site identity requires a new retained origin. Preserve the old origin separately.
Exact never rewrites completed releases or discards history automatically.

The root-relative `/zephyr-manifest.json` supplies Zephyr runtime configuration outside the Exact inventory.
Current Exact code does not consume its host specifier. The staged package declares no remotes.
Readback also fetches this runtime manifest and refuses dependencies. This preparation supports only the host map.
