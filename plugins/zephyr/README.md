# Zephyr transport for Exact

Exact signs updates. Zephyr stores complete immutable public snapshots. No runtime imports this plugin.

Build and stage an update with the normal command:

```sh
bun scripts/deploy.mjs <app> --origin /absolute/retained-origin --export /absolute/new-snapshot --yes
```

Use the same retained origin for subsequent generations. Export requires a web root and authenticated native envelopes.
The export includes all retained native envelopes and their separate referenced MF, Hermes, plan and asset blobs.
It materializes the current web files at the root and retains previous web release directories.
It excludes origin locks and unreferenced blobs. Keep the signing key outside the origin.
Without `--yes`, deployment does not export or upload.

`publishSnapshot` in `index.mjs` accepts the retained `DirectoryOrigin`, resolved Exact app, site configuration and control-plane hooks.
It validates the complete graph before invoking `cliUpload`. The default transport uses the supported `zephyr-cli@1.4.1` uploader.
Install pnpm and authenticate its normal Zephyr user first. Do not use MCP credentials or operator tokens.

The caller implements these hooks through its supported authenticated Zephyr control plane:

- `inspect(site)` returns `identityMatches`, `noAutomaticPromotion` and the observed environment `versionId` (null when absent).
- `resolveBuild(site, receipt)` finds the exact uploaded build in history through its unique `exact-zephyr-upload-id.txt` marker. It returns `applicationUid`, `versionUrl` and `versionId`.
- `promote(site, {versionId, expectedVersionId})` requests explicit promotion.

`inspect` must check deployment rules and the CLI user before upload. Do not return constant approvals.
Unset `ZE_ENV`. Disable automatic environment or tag rules for this upload app.
The uploader can activate those rules before returning. The CLI 1.4.1 transport cannot undo automatic activation. The control-plane policy check must exclude it before upload.
`promote: true` verifies every immutable file before requesting promotion. The stale-read check is not atomic compare-and-swap.
The caller must verify stable content after promotion. `dryRun: true` performs no transport or control-plane writes.

Root's isolated live fixture uses org `zackarychapple`, project `exact-plugin-lifecycle`, app `signed-updates`, environment `exact-proof`.
Its site ID is `2dee0599-4d8d-4e9a-b15d-27e1ff509850`.
Use `site: {org: 'zackarychapple', project: 'exact-plugin-lifecycle', app: 'signed-updates', environment: 'exact-proof'}`.
The CLI upload marker and application identity must match this site's build history before promotion. Existing proof sites need no changes.

To connect the normal deploy command, provide a local module through `--transport /absolute/zephyr-transport.mjs`.
Its default function receives `{origin, app, snapshot}` after successful publication and export.
This `origin` reads the frozen export directory, so a concurrent retained-origin publish cannot change the upload.
The module calls `publishSnapshot({origin, app, site, control, promote: true})` with the control hooks above.
Exact does not load the hook during a dry run or after a failed build or refused stream.
Keep this module and its account configuration outside the exported directory.

Published CLI 1.4.1 has no structured deploy receipt. `resolveBuild` must inspect candidate immutable builds for `receipt.uploadId`.
Do not choose the newest build without checking this marker. Verification checks the marker again with every other file.

Web publication now retains `.exact/root/web/inventories/<id>.json` for each generation.
The publisher writes this inventory after it verifies every payload. Export includes these completed generations and skips partial directories.
Use a fresh retained origin for this lifecycle.
Existing proof artifacts remain unchanged.
