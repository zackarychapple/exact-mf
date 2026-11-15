# Issue 1 — native federation review

Source review of [issue 1](https://github.com/zackarychapple/exact-mf/issues/1), performed 2026-11-14. Example `bf7c7817e2ab17a7af257d9501eef3d5491f7d8f`; Exact `cbb3ae319bd761d4e3d339b464e7a420b0c6383a`; MF core `d4a31960d20a717626af43be59aab0881c7b6cc0`; Tauri `30e083a24c88ece9d09c84d0b19ff7847c89564f`. References below are immutable source links. This review ran no builds, runtime probes, simulator sessions, or device tests. Existing proof claims belong to the issue/README, not to this review.

## Findings

### 1. Export generation and the asynchronous value boundary must come before general sharing

**Confirmed gap.** `tools/build-remotes.mjs:9` emits a remote that returns fixed release metadata regardless of operation. `tools/host.mjs:6` returns a handwritten container and discards every argument to `init`; its factory returns a native handle. `native/apple/src/bridge.rs:108` permits only `./value`, and `:139` explicitly refuses a non-immediate MF answer. These are useful lifecycle fixtures, but do not implement the issue's two exposes, delayed operations, validated structured values, or cancellation.

Sources: [producer](https://github.com/zackarychapple/exact-mf/blob/bf7c7817e2ab17a7af257d9501eef3d5491f7d8f/tools/build-remotes.mjs#L9), [container](https://github.com/zackarychapple/exact-mf/blob/bf7c7817e2ab17a7af257d9501eef3d5491f7d8f/tools/host.mjs#L6), [bridge](https://github.com/zackarychapple/exact-mf/blob/bf7c7817e2ab17a7af257d9501eef3d5491f7d8f/native/apple/src/bridge.rs#L106).

**Recommendation.** Generate a signed export roster and adapter from one validated schema. Preserve the existing composition-handle profile. Initially support structured values and named asynchronous operations through revocable execution-scoped handles; exclude arbitrary engine objects, components, and raw function references. Define errors, request IDs, cancellation, payload limits, and late completion disposal before implementation. Acceptance must execute two exposes and a genuinely delayed operation through real MF/Hermes, including negative payload, authorization, unknown-expose, and revoked-handle cases.

### 2. Upstream semver exists, but Exact has no connected sharing boundary

**Confirmed gap.** Core constructs `shareScope`, `initScope`, and extended initialization options (`packages/runtime-core/src/module/index.ts:197`). Exact's container ignores them. Core's resolver distinguishes strict singleton mismatch from non-singleton range selection (`utils/share.ts:395` and `:416`); merely retaining that source does not register or consume providers. `PreparedRemote` explicitly isolates each remote realm and forbids engine/share-scope objects crossing its boundary (`js/src/remote.rs:7`). The native builder explicitly declares cross-generation singletons unsupported (`js/bake/src/native_federation.mjs:71`).

Sources: [core initialization](https://github.com/module-federation/core/blob/d4a31960d20a717626af43be59aab0881c7b6cc0/packages/runtime-core/src/module/index.ts#L197), [resolver](https://github.com/module-federation/core/blob/d4a31960d20a717626af43be59aab0881c7b6cc0/packages/runtime-core/src/utils/share.ts#L395), [Exact boundary](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/js/src/remote.rs#L7), [builder contract](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/js/bake/src/native_federation.mjs#L71).

**Recommendation.** Choose a precise sharing profile before wiring scopes. Generation-owned realms with same-realm provider sharing are a plausible first profile; host-owned service proxies require an explicit separate capability contract. Do not call separately bundled copies shared singletons. Qualify all eight issue scenarios in Hermes, record provider identity as well as version, and reject unsupported options before native commit. Preserve predecessor isolation when testing loaded-first and major-version changes; a fresh candidate's realm cannot silently inherit the old realm's loaded singleton.

### 3. Multi-remote replacement requires per-remote ownership, not a bigger registration map

**Confirmed scope limitation.** `RemoteCoordinator` owns one Store, one pending candidate, and one active candidate (`js/src/remote.rs:154`). The example's commit path retains only the new execution in both `REMOTES` and `HOST` (`native/apple/src/bridge.rs:292`). Adding a second remote to this path would remove the first remote's routing/handle state on commit. Existing fresh MF instances are legitimate; persistent-instance replacement is not necessary to solve this.

Sources: [coordinator ownership](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/js/src/remote.rs#L154), [example eviction](https://github.com/zackarychapple/exact-mf/blob/bf7c7817e2ab17a7af257d9501eef3d5491f7d8f/native/apple/src/bridge.rs#L272).

**Recommendation.** Introduce stable remote IDs above execution IDs and a host registry of per-remote owners. Define whether nested dependencies are independently replaceable or belong to a signed generation graph; reject cycles deterministically initially. Keep cache keys execution-specific. Acceptance needs two independent remotes, replacement of only one, nested dependency behavior, stale factory/async references, and at least 100 commit/abandon/retry cycles measuring engines, handles, caches and actual resource behavior. Existing map-size/proxy counters do not prove bounded process memory.

### 4. The admission-history persistence mismatch deserves an early release gate

**Confirmed source defect; security consequences need reproduction.** Every distinct admitted sequence is retained (`update/src/store.rs:886`). `read_record` caps reads at 1 MiB and treats failure as fresh state (`:372`), while `write_record` has no corresponding bound (`:1276`). Boot-mark writes also serialize the whole record without a size check (`:1289`). Startup recovers the maximum floor from retained signed entries (`:534`) but only seeds that maximum into admission history (`:545`). Therefore an oversized record can lose selected/last-good and historical conflict information during restart; this review does **not** demonstrate a floor bypass.

Sources: [bounded reader](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/update/src/store.rs#L372), [recovery](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/update/src/store.rs#L534), [history insertion](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/update/src/store.rs#L886), [writers](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/update/src/store.rs#L1275).

**Recommendation.** Reproduce with enough valid signed generations to cross the actual serialized limit before long-lived qualification. Add fail-safe writes as an initial safeguard, then choose a scalable durable history format or an explicitly justified retention policy. Cover both durable-record and boot-mark writers. Arbitrarily truncating history weakens same-sequence conflict detection. Reopen tests must verify selected/last-good, maximum and embedded floors, historical conflicts, and intentional rollback; measure startup work and bytes.

### 5. Tauri supplies useful failure-policy ideas, not a drop-in native Exact adapter

**Confirmed applicability boundary.** Tauri's runtime plugin rewrites URLs to `mf://`, uses `fetch`, marks an entry good at `onLoad`, and calls `location.reload()` on rollback (`packages/tauri/index.js:33`, `:48`, `:66`). Its documented implementation runs in a WebView. Rollback rejects all unconfirmed trials and reloads the page to reset shared state. Manifest remotes bypass its protocol cache; rollback requires content-hashed chunks.

Sources: [Tauri hooks](https://github.com/module-federation/tauri/blob/30e083a24c88ece9d09c84d0b19ff7847c89564f/packages/tauri/index.js#L33), [architecture](https://github.com/module-federation/tauri/blob/30e083a24c88ece9d09c84d0b19ff7847c89564f/docs/architecture.md), [limits](https://github.com/module-federation/tauri/blob/30e083a24c88ece9d09c84d0b19ff7847c89564f/docs/offline-and-rollback.md#limitations).

**Recommendation.** Reuse bounded retry, explicit trial/last-good terminology, failure telemetry, and immutable asset retention concepts. Do not copy global reload/rollback or treat `onLoad` as Exact native commit. Exact must preserve an unrelated remote and retain the predecessor until native acceptance and durable selection succeed. Exact's signed digest/roster cache remains authoritative; successful HTTP or a content hash alone is not admission.

### 6. Qualification must preserve specialization integrity and separate host claims

**Confirmed implementation boundary.** The builder verifies pinned package versions and hashes (`native_federation.mjs:103`, `:155`), emits one artifact without code splitting (`:76`), and disables DOM/Node/module loaders and preload paths (`:169`). Preserve these controls when adding native features. The explicit package/Hermes lifecycle test is ignored by default because it installs registry packages (`js/bake/tests/packages.rs:591`). Local Cargo paths still target the absent original checkout (`validation/Cargo.toml:9`).

Sources: [specialization](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/js/bake/src/native_federation.mjs#L103), [qualification test](https://github.com/zackarychapple/exact-mf/blob/cbb3ae319bd761d4e3d339b464e7a420b0c6383a/js/bake/tests/packages.rs#L591), [manifest paths](https://github.com/zackarychapple/exact-mf/blob/bf7c7817e2ab17a7af257d9501eef3d5491f7d8f/validation/Cargo.toml#L9).

**Recommendation.** Make checkout discovery reproducible before relying on CI; keep all generated dependencies on the pinned Exact revision. Run the focused validation after bridge changes and explicitly run this additional test from the Exact checkout:

```sh
cargo test -p exact-js-bake --test packages native_federation_runs_upstream_init_get_and_factory -- --ignored --exact
```

After implementation, build the simulator HOST app without launching it. Live simulator/device/platform qualification is separate evidence; no physical-device claim follows from that build. Fault gates must cover entry/init/get/factory/native preparation/commit, incompatible HBC/plan/ABI/grants, offline restart, corrupt storage, cancellation, and migration using actual state.

## Recommended order

1. Make pinned checkout/build paths reproducible; reproduce and safeguard history persistence in parallel with contract design.
2. Define export values, async ownership, compatibility identities, sharing boundary, and selection trust; explicitly retain intentional rollback.
3. Generate the signed producer/adapter and prove two exposes before implementing sharing.
4. Wire real provider registration/consumption and qualify the complete semver/support matrix in Hermes.
5. Add per-remote ownership, nested graph policy, recovery/migration and resource stress tests.
6. Qualify platform claims and the explicit ignored package test; add optional prefetch only through signed native asset paths.

The Zephyr review supplies the dependency publication/resolution steps that follow the signed producer contract.
