# Issue 1: native federation implementation plan

Prepared 2026-11-14 for [issue #1](https://github.com/zackarychapple/exact-mf/issues/1). This is a source review and proposed implementation sequence, not new runtime, deployment, or platform qualification evidence.

## Review inputs and reproducibility

Two independent reviews accompany this plan:

- [Native runtime, sharing, lifetime, and Tauri](reviews/issue-1-native-review.md).
- [Zephyr publication, metadata, selectors, and CI](reviews/issue-1-zephyr-review.md).

All reference checkouts are under the root-ignored `reference-repos/`. These are research inputs, not replacement Cargo dependencies or vendored application code.

| Checkout | Repository | Reviewed revision |
| --- | --- | --- |
| Application | `zackarychapple/exact-mf`, main | `bf7c7817e2ab17a7af257d9501eef3d5491f7d8f` |
| `module-federation-core` | `https://github.com/module-federation/core.git` | `d4a31960d20a717626af43be59aab0881c7b6cc0` |
| `module-federation-tauri` | `https://github.com/module-federation/tauri.git` | `30e083a24c88ece9d09c84d0b19ff7847c89564f` |
| `zephyr-packages` | `https://github.com/ZephyrCloudIO/zephyr-packages.git` | `5f3ec208a2b0c5f4ca3cc883011ccc1b364632bb` |
| `exact-runtime` | `https://github.com/zackarychapple/exact-mf.git`, exact-runtime | `cbb3ae319bd761d4e3d339b464e7a420b0c6383a` |

Core and Zephyr match the issue's audit revisions. Tauri is the fetched default-branch revision, which the issue did not pin. Exact matches README and `EXACT_REVISION`; its `docs/start-here.md` was read. The README's `/Users/zackarychapple/code/t3/exact` checkout is absent on this machine. An additional reference clone made review possible without changing generated native dependencies.

## Recommended boundaries

1. **Keep Exact responsible for trust and activation.** Signatures, artifact hashes, grants, app identity, compiler/engine and plan compatibility, admission history, ownership and native commit remain mandatory. MF semver and Zephyr deployment selection never substitute for these checks.
2. **Retain upstream MF lifecycle and resolution.** Generate a native container adapter that forwards initialization arguments and invokes real `init`, `get`, factories and share resolution. Preserve the existing generation-handle composition profile. Do not implement host branches for release labels or depend on an undocumented removal API.
3. **Start with generation-owned execution.** Keep fresh MF execution identities. First support sharing within an explicitly owned execution realm; copies in independent realms are not a cross-generation singleton. Host-owned services require an explicit revocable capability contract. Cross-generation JS singletons remain unsupported until their ownership design is qualified.
4. **Use schema-validated values and named asynchronous operations.** Define structured values, error envelopes, cancellation, ownership and revocation first. Defer unrestricted callable proxies and raw engine/framework/native objects. A delayed operation must still be supported; do not reduce the producer to static JSON metadata.
5. **Treat selection as a separate trust policy.** Immutable pins and mutable environment following are distinct. Preserve intentional signed rollback under the current environment policy. Fresh promotion authorization, if required, needs a separate signed protocol; existing release signatures do not provide it.
6. **Reuse Zephyr contracts that already exist.** Qualify the pinned source's structured deploy result and metadata sidecar before designing new publication APIs. Source support is not evidence that an installed release or production service supports the complete Exact contract. `tap-app` is TAP-specific; agree a supported Exact target rather than relabeling Exact artifacts.
7. **Borrow Tauri's recovery ideas selectively.** Timeout/backoff and explicit trial/last-good state are useful references. Tauri executes in a WebView and recovers through page reload; Exact must preserve the predecessor and isolate per-remote effects without adopting that execution or commit boundary.

## Implementation sequence and release gates

Each phase should be a reviewable change with its own evidence. Runtime/plugin changes belong in the maintained Exact checkout/branch. The example consumes a deliberately updated Exact revision after those changes pass; generated dependencies must continue to point to that checkout.

### 0. Make validation reproducible and protect durable history

**Areas:** example Cargo manifests and build scripts; Exact `update/src/store.rs`, Store tests and CI.

- Establish one configurable Exact checkout path for root, tools, validation and generated native manifests. Verify its revision against `EXACT_REVISION`; `EXACT2` alone currently changes only script paths.
- Keep Bun and MF package pins and Exact's upstream package hash checks. Establish the attested Hermes toolchain before claiming a clean build.
- Reproduce admission-history growth beyond the 1 MiB reader limit with distinct valid signed generations. Cover both durable record and boot-mark writes.
- First prevent unreadable state from replacing a readable record. Then implement a scalable durable history format or an explicit bounded admission policy that preserves sequence-conflict security. Do not silently prune accepted sequence identities to make the size test pass.
- Distinguish oversized/corrupt state from an ordinary first boot. Prove recovery from retained signed entries; do not claim a demonstrated floor bypass from the static size mismatch alone.

**Gate:** clean checkout focused validation and simulator HOST build; history-limit reopen test checks selected/last-good state, embedded and maximum floors, prior sequence conflicts, rollback and bounded startup work. No simulator launch is needed for this build gate.

**Scheduling:** begin immediately; run in parallel with contract design. The history work is a production release blocker independent of federation feature work.

### 1. Specify the producer, identity and ownership contracts

**Areas:** Exact native federation builder, remote coordinator, export schema; example remote declaration.

- Specify stable remote ID, immutable artifact identity, execution ID, cache identity and generation token separately.
- Define exposes, structured payload schema, named async operation signatures, errors, cancellation and revoked-handle behavior. Model side effects and migration ownership explicitly.
- Define the signed artifact roster and compatibility envelope for plans, HBC, export schema and every executable dependency. No unsigned manifest or nested asset may become an execution authority.
- Publish the sharing support table: `singleton`, `requiredVersion`, `strictVersion`, `eager`, scopes and both strategies. Initially reject any unsupported option at build/preparation time.
- Set non-singleton incompatible-range behavior explicitly; recommended initial policy is rejection unless a declared, signed local fallback is available. Distinguish it from singleton strict mismatch.
- Define native target metadata and immutable/mutable selection policy with Zephyr. Specify cycle rejection initially and a future-compatible migration contract.

**Gate:** schema and refusal fixtures cover unknown fields/options, invalid values, unauthorized operations, incompatible ABI/grants and dependency cycles. Every identity has one owner and a documented lifetime.

### 2. Implement a reusable native producer and adapter

**Areas:** Exact `js/bake/src/native_federation.mjs`, `js/src/remote.rs`; example `tools/build-remotes.mjs`, `tools/host.mjs`, native bridge.

- Replace the handwritten one-expose producer with generated containers and export metadata, retaining the existing composition-handle profile.
- Forward share scope, initialization-cycle and extended initialization arguments through the adapter. Preserve upstream MF lifecycle code and its source hash qualification.
- Implement schema-checked structured results and delayed operations with cancellation/revocation checks on both dispatch and completion.
- Prepare the complete candidate before native activation; an export/init/factory/preparation refusal leaves the predecessor usable.

**Gate:** independently build one remote with two exposes, load both through real MF in Hermes, return structured data and complete a delayed operation. Reject unknown exposes, malformed payloads, unauthorized calls and revoked handles. Remote executable/plan bytes remain outside the initial host bundle.

### 3. Connect actual shared providers and qualify semver

**Areas:** generated providers/consumers, upstream MF adapter, coordinator ownership and Hermes acceptance fixtures.

- Register actual providers and consume through upstream MF within the declared realm. Report chosen provider identity, version, scope, initialization count and fallback policy.
- Reject unsupported cross-realm sharing rather than claiming independent copies form one singleton. Keep sharing lifetime separate from remote release/version labels.
- Make asynchronous preload/preparation explicit; `eager` must never imply synchronous network loading.

**Gate:** all eight issue scenarios run in Hermes: highest compatible version-first selection; incompatible non-singleton rejection/fallback; strict loaded-singleton rejection; loaded-first reuse; duplicate equal-version singleton identity/init policy; major-version candidate refusal preserving predecessor; unsupported-option rejection; rollback with a fresh execution identity and the same dependency policy. Stock Bun probes do not satisfy this gate.

### 4. Connect Zephyr metadata and authenticated resolution

**Areas:** Exact `plugins/zephyr/{index,prepare,snapshot,deploy-output}.mjs`; upstream CLI metadata/result and resolver contracts.

- Qualify an explicitly pinned CLI/API version supporting JSON receipts, real package semver and federation metadata. Independently verify the returned UID, target and immutable build rather than trusting requested slugs or echoed input.
- Publish truthful host/remote relationships, exposes and shared declarations. Dashboard metadata must not fabricate a browser container.
- Capture requested selector and independently established immutable resolution identity separately; validate exact/range/tag/environment/workspace results and reject incompatible fallback.
- The inspected JSON receipt lacks target, build ID and snapshot ID; retrieve those independently and fail on skipped/null/ambiguous results. The resolver can overwrite `version` with the requested selector and broadly retry workspace failures with `*`; retain the original request and constrain fallback by error class.
- Resolve dependencies before signing, or define a separately authenticated immutable resolution artifact. Preserve signed bytes even when HTML is rewritten; do not strip injected content to make hashes pass.
- Replace host-only dependency rejection only once the delivery contract and native checks are in place.

**Gate:** real dashboard/API relationships and shared versions; exact/range/tag/environment/workspace resolution, incompatible fallback refusal, immutable pins versus mutable following, and a dependency override all resolve to the same admitted signed identity. Local fixture tests are necessary but do not establish production service guarantees.

### 5. Qualify publication, promotion and CI end to end

**Areas:** maintained `deploy.mjs --zephyr-config` path, control hooks, deployment receipts and isolated Zephyr application.

- Run build → signing → export → upload → immutable readback → stable selection → native activation through the normal plugin command, not a direct CLI upload substitute.
- Verify every snapshot file, unique upload marker, UID, target and immutable build. Preserve failed/ambiguous upload evidence and check history before retrying.
- Inspect promotion rules before upload, because automatic rules can expose a build before post-upload verification. Test protected/locked environments and stable selected-content readback.
- Qualify one supported unattended CI actor/auth path without secrets in logs, artifacts or snapshots.
- Test stale requests, superseded candidates, embedded minimums and same-sequence conflicts while retaining intentional rollback. Report concurrent promotion races honestly unless the service supplies genuine atomic expected-version semantics.

**Gate:** reproducible isolated-service evidence for the whole path, rollback to retained versions and CI identity. Failed build/refused stream causes no upload. Publish the service contract findings for immutable retention, missing-blob status/MIME, redirects/cache behavior and dependency rewriting; backend gaps remain explicit blockers for the guarantees they affect.

### 6. Qualify multiple remotes, recovery and platforms

**Areas:** coordinator, Store, native ownership/activation, fault harness and platform CI.

- Exercise two independent remotes, update one without disturbing the other, and load a nested dependency. Reject cycles deterministically until supported.
- Introduce a registry of per-remote owners above the single-remote coordinator and replace the example's global retain-only-new-execution cleanup. Preparation, commit, abandonment and routing eviction must be scoped to the affected remote.
- Run at least 100 commit/abandon/retry cycles. Measure live engines, modules, registrations, asset caches, handles and memory under an explicit retention bound; bounded maps alone do not prove bounded process resources.
- Cover duplicate loads, cached factories, late async completion, timers, subscriptions, requests, native registrations and persistent effects. Retry with a fresh execution identity and bound attempts/backoff.
- Inject network, entry/init/get/factory, native preparation and commit failures; exercise offline restart, interrupted download, corrupt/missing blobs, disk refusal and every durable transition. Test migration/refusal with actual state.
- State the disk/render crash boundary explicitly. Do not equate a durable callback with power-loss atomicity.
- Qualify HBC/compiler/engine, plan/schema, seam ABI, app identity and grants. Maintain separate platform results and require a new host binary for native module/entitlement/permission/ABI changes.

**Gate:** reproducible failure/recovery/resource evidence and a support matrix. Build the simulator HOST without launching during routine implementation. Runtime simulator/device acceptance is a separate requested qualification activity; physical-device support cannot be claimed from simulator evidence.

### 7. Complete tooling and optional performance features

Thread minimal correlation fields through phases 1–6 so qualification evidence is usable. Finish reusable reports and generated types here: publication UID/target/build, selector/resolution, generation digest, execution ID, MF phase, provider decision and native outcome. Validate values at runtime even when generated types compile.

Keep native prefetch/code splitting explicitly unsupported initially. If added, prefetch only verified bytes without executing; sign and compatibility-check each split executable before evaluation. DOM/CSS loading, browser devtools, React bridges, hydration and SSR remain outside the initial native scope.

## Validation commands and evidence policy

After the checkout/toolchain prerequisites in phase 0 are fixed:

```sh
# Example: focused bridge validation, after generating signed fixtures.
CARGO_TARGET_DIR="$PWD/target" cargo test --manifest-path validation/Cargo.toml

# Pinned Exact checkout: explicitly include the otherwise ignored package/Hermes test.
cargo test --manifest-path "$EXACT2/js/bake/Cargo.toml" --test packages native_federation_runs_upstream_init_get_and_factory -- --ignored

# Exact plugin's existing local lifecycle suite.
bun test "$EXACT2/plugins/zephyr/lifecycle.test.mjs"

# Example: build the simulator HOST; the script uses --no-install.
tools/build-host.sh
```

These existing commands are baseline gates, not substitutes for the new acceptance fixtures above. Keep evidence per phase: source revisions, toolchain identities, command/result, nonsecret receipts and correlation records, platform, and limitations. No application code or bridge was changed during this review, so no native build, runtime test or live deployment was performed. Ignore coverage and whitespace are checked for this documentation-only change.

Issue closure requires every required acceptance item to have reproducible evidence or an explicitly agreed exclusion. Unsupported sharing, missing hosts and unknown service guarantees must remain visible rather than being counted as completed parity.
