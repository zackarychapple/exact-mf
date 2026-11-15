# Issue 1 review 2: Zephyr publication and dependency selection

Source review on 2026-11-14. Scope: [issue #1](https://github.com/zackarychapple/exact-mf/issues/1), Exact runtime/plugin `cbb3ae319bd761d4e3d339b464e7a420b0c6383a`, Zephyr packages `5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb`. Paths below are relative to the corresponding checkout in `reference-repos/`. This is an architecture and acceptance-gap review, not a review of an implementation diff. No deployment, native build, or live service qualification was performed. Bun is unavailable in this review environment, so the existing Bun lifecycle tests were inspected but not executed.

## Findings

### 1. Reuse the existing metadata and JSON receipt contracts, but qualify their limits

**Confirmed gap, high implementation priority.** Exact stages `version: '0.0.0'`, supplies no federation metadata, invokes CLI `1.4.1 --target web`, parses human log output, and constructs the receipt's application UID from the requested configuration (`plugins/zephyr/index.mjs:98–110`). The marker and complete-file readback do establish which uploaded bytes were returned; the synthesized UID does not independently establish the service's application identity.

The pinned Zephyr source already offers `--metadata` and `--format json`. Its CLI source package version is 1.8.0. `CliPublicationMetadata` supports snapshot `mfConfig`/`mfConfigs` and dashboard `federation`; `uploadAssets` forwards these to the engine. The structured result supplies engine application UID, immutable URL and target URLs. It still does **not** supply target, immutable build/version ID, or snapshot ID, and permits a null version URL or an explicit skipped result. An upgrade alone therefore cannot meet the full receipt acceptance criteria.

**Recommendation:** qualify a released CLI containing these contracts; consume the structured result, fail on skipped/null/ambiguous results, and independently retrieve the missing immutable build metadata. Derive real package semver, exposes, shares and remote graph from the native producer contract. Keep the random upload marker and byte-for-byte readback.

Sources: [Exact transport](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/plugins/zephyr/index.mjs#L88), [metadata schema](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-cli/src/lib/publication-metadata.ts#L11), [upload forwarding](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-cli/src/lib/upload.ts#L29), [deploy result](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-cli/src/commands/deploy.ts#L26).

### 2. Native target selection is a contract decision, not a string substitution

**Confirmed integration gap; backend support unproved.** Zephyr's target enum permits `web`, `ios`, `android`, and `tap-app`; it has no Exact-specific target. The `tap-app` path is useful prior art: it requires paired container/dashboard metadata and preserves all locked artifact paths with read failures treated as errors. Generic extraction applies web-oriented filtering and can warn on directory read failures. These differences matter to a complete signed snapshot.

**Recommendation:** agree which existing target legitimately represents Exact, or add a supported target end to end. Do not relabel Exact as TAP merely to gain artifact preservation. Qualify hidden `.exact` records, opaque blobs, retained releases, MIME, redirects, missing-path behavior and upload/readback inventory equality under the chosen contract. Preserve native compatibility validation independently of target selection.

Sources: [target enum](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-edge-contract/src/lib/build-target.ts#L1), [artifact extraction](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-cli/src/lib/extract-assets.ts#L37), [TAP metadata contract](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-cli/src/lib/publication-metadata.ts#L251).

### 3. Resolver results need a separate immutable identity and explicit fallback policy

**Confirmed source behavior, high implementation priority.** `resolve_remote_dependency.ts:151–164` replaces the returned `version` with the requested selector and `platform` with the requested platform, while retaining the response platform as `resolved_platform`. Its interface also describes optional entity type/name fields for older API compatibility. Treating `result.version` as the resolved immutable release would lose the distinction the issue requires.

The workspace path at lines 68–80 catches any initial request failure and retries with `*`; it does not first distinguish a missing workspace build from authentication, network or server failures. That fallback may also return a result whose `version` is `*`, losing the original workspace selector unless the caller records it separately. Source comments describe an intended missing-build fallback, but the actual catch is broader.

**Recommendation:** preserve original selector and build context before resolution; record fallback attempts, requested and resolved platforms, entity type, immutable build identity, and signed Exact digest separately. Define whether workspace fallback is allowed and which error classes permit it. Require verified native metadata/HBC/ABI/grants even when a resolver returns HTTP success or falls back to web. Test exact/range/tag/environment/workspace, incompatible ranges, missing workspace, transient failure, cross-platform fallback, and pinning versus mutable selection.

Source: [resolver implementation and result interface](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-agent/src/zephyr-engine/resolve_remote_dependency.ts#L14).

### 4. Dependency overrides cannot be enabled by removing the host-only rejection

**Confirmed safety boundary, high implementation priority.** Exact's preparation creates only the host environment import map. `assertHostOnlyManifest` rejects nonempty dependencies after full uploaded-file verification. The manifest is outside the frozen signed inventory. Merely accepting dependencies would leave no defined authenticated link between a service-selected remote and the signed native generation.

**Recommendation:** decide between resolving dependency configuration before signing and authenticating a separate immutable resolution artifact. Native consumption must bind every executable dependency to admitted identity. Keep signed-byte verification unchanged and retain the current rejection until this delivery contract exists. A mutable environment override must select an authenticated dependency graph, not mutate already signed payloads.

Sources: [manifest refusal and verification](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/plugins/zephyr/index.mjs#L20), [HTML preparation](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/plugins/zephyr/prepare.mjs#L4), [plugin contract](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/plugins/zephyr/README.md).

### 5. Normal deployment does not activate the policy hooks or verify stable selection

**Confirmed acceptance gap.** `deploymentOutput` calls `publishSnapshot` with origin/app/site only. Inspection, build resolution and explicit promotion exist as optional callbacks, but are absent from the normal command. `publishSnapshot` verifies immutable content, then calls promotion when requested; it does not subsequently read back the stable environment marker and signed identity. Automatic service promotion can occur during upload, before immutable verification. Comparing an observed version before calling a promotion callback is not atomic CAS.

**Recommendation:** expose real policy/control integration through the maintained command, qualify it in an isolated application, and add stable readback after promotion. Preserve ambiguous-upload evidence and require history inspection before retrying. Test locked/protected environments, automatic rules, concurrent writers and stable content convergence. Report an unresolved race unless the backend supplies an atomic expected-version operation.

Sources: [normal command boundary](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/plugins/zephyr/deploy-output.mjs#L7), [inspection and promotion](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/plugins/zephyr/index.mjs#L31).

### 6. CI support exists upstream; the Exact path still needs end-to-end qualification

**Confirmed qualification gap.** Exact explicitly refuses `ZE_SECRET_TOKEN`; that is not a blanket prohibition on upstream CI authentication. Pinned Zephyr `token.ts:42–64` supports CI-token exchange with inferred CI identity, and lines 185 onward implement the exchange/cache. Exact passes the process environment to CLI 1.4.1. This does not establish that its pinned released CLI has the inspected source behavior or that staging outside the project preserves correct identity attribution.

**Recommendation:** choose and pin a supported released CLI/auth combination; qualify one real noninteractive CI run with a disposable application and the intended actor. Preserve project/git/build context explicitly, verify returned application/build identity independently, redact evidence, and keep all signing credentials outside public snapshots. Source capability, released package capability and successful CI evidence must be separate checklist items.

Sources: [Exact CLI invocation](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/plugins/zephyr/index.mjs#L88), [upstream token selection](https://github.com/ZephyrCloudIO/zephyr-packages/blob/5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb/libs/zephyr-agent/src/lib/node-persist/token.ts#L42).

## Recommended sequence and release gates

1. **Contract spike:** qualify the existing metadata sidecar and JSON receipt; settle target/artifact preservation and immutable resolver identity. Obtain service evidence for fallback, byte preservation, retention, missing blobs and promotion concurrency. These are external questions, not conclusions from a local mock.
2. **Producer-to-publication mapping:** map the validated native producer contract to package semver, host/remote graph, exposes/shares and signed compatibility metadata. Verify dashboard/API readback and an immutable build lookup. No fabricated browser container.
3. **Authenticated resolution:** implement selector policy and bind selections to the signed generation/dependency graph. Preserve intentional signed rollback, embedded minimum and same-sequence conflict checks; release signatures alone do not establish fresh promotion authorization.
4. **Delivery and controls:** replace the host-only assumption only after the dependency artifact contract exists. Wire inspection, promotion and stable readback into the normal command. Fault tests must prove failed/refused builds never upload and modified bytes never verify.
5. **Live qualification:** run maintained build → sign → export → upload → immutable readback → stable selection → native activation, then rollback and offline restart. Run one supported CI publish. Record service actor, UID/target/build, selector/resolved identity, digest/execution identity and outcomes together.

Existing `plugins/zephyr/lifecycle.test.mjs` covers important local refusal, byte-integrity, marker, retained-history and fake-CLI behavior. It cannot establish dashboard relationships, service retention, CI attribution, platform compatibility or live activation. Keep those acceptance gates separate.
