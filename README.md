# Exact native remote example

This example loads signed Exact plans through Module Federation 2.9.3. Zephyr serves the static artifacts.

The host starts with a neutral loading plan. After its first draw, it polls the configured HTTPS origin every two seconds.
`RemoteCoordinator` admits the signed generation. Upstream MF calls `init`, `get`, and the factory through the native bridge.
The bridge checks remote release metadata against signed assets and the admitted plan digest.
`ExactComposition.applyGeneration` mounts the downloaded plan and commits the matching generation token.
The host explicitly calls `Store::follow_environment` with the configured HTTPS head URL.
This policy trusts that environment to select any valid signed release at or above the embedded minimum.
Signed bytes establish release authenticity. They do not provide fresh signed promotion authorization.
Mutable environment selection has no anti-replay protection.
The store retains its maximum sequence floor and durable admitted sequence identities.
A different signed digest for a recorded sequence is refused, including after restart.
Existing stores seed this history from the retained maximum sequence. Previously collected lower-sequence identities cannot be recovered.
The default Exact policy still refuses releases below its retained floor.
The host does not choose a design from a version number. Neither remote plan is bundled in the host.

The blue editorial panel is `remotes/v1.contract`. The orange welcome layout is `remotes/v2.contract`.
The diagnostic area shows the remote release, verified sequence, entry digest, plan digest, and host session identity.
The session identity remains constant for the controller lifetime. Mount records also contain the process ID.

## Source layout

This repository has two branches:

- [`main`](https://github.com/zackarychapple/exact-mf/tree/main) contains this example, the native bridge, and remote sources.
- [`exact-runtime`](https://github.com/zackarychapple/exact-mf/tree/exact-runtime) contains the pinned Exact checkout and its integration changes.

The [Zephyr plugin](https://github.com/zackarychapple/exact-mf/tree/exact-runtime/plugins/zephyr) includes source, lifecycle tests, and publication instructions.
The [native MF builder](https://github.com/zackarychapple/exact-mf/blob/exact-runtime/js/bake/src/native_federation.mjs) specializes upstream Module Federation for Hermes.
The [remote coordinator](https://github.com/zackarychapple/exact-mf/blob/exact-runtime/js/src/remote.rs) controls verified remote execution and activation.
The plugin remains inside Exact because it imports Exact's build and publication modules.

Clone the branches into separate directories:

```sh
git clone --branch main https://github.com/zackarychapple/exact-mf.git exact-mf
git clone --branch exact-runtime https://github.com/zackarychapple/exact-mf.git exact-runtime
git -C exact-runtime checkout "$(cat exact-mf/EXACT_REVISION)"
```

Select the checkout once from the example directory:

```sh
EXACT2=../exact-runtime bun tools/configure-exact.mjs
```

This validates `EXACT_REVISION` before creating the ignored `.exact/runtime` link.
All Cargo manifests and build scripts use that same checkout. `EXACT2` may name
another directory, including `reference-repos/exact-runtime`; relative paths are
resolved from this example. Subsequent commands reuse the link. A revision mismatch
fails before the link changes. Reconfigure after moving either checkout.

## Dependencies

Use Bun 1.4.2 and Exact commit `4d902586fd2a6a207ffa02d0b3a66076b65cf026`.
Cargo manifests reference the configured checkout by path. The generated plans use format 5.
The old format 4 proof and its `/tmp` evidence remain unchanged.
The native MF artifact uses Exact's maintained `buildNativeFederation` API. The Bun lock pins all three upstream packages to 2.9.3.
Use the Hermes compiler from Exact's attested local bundle. `HERMESC` can override its path.

Install Exact's Bun dependencies with `bun install --frozen-lockfile` in that checkout.
Provision its pinned Hermes host and simulator bundles before building:

```sh
# In the Exact checkout on an Apple Silicon Mac:
cargo run --manifest-path vendor/ibex/crates/hermes-lean-sys-installer/Cargo.toml -- --target aarch64-apple-darwin --target aarch64-apple-ios-sim
rustup target add aarch64-apple-ios-sim
```

The shell helpers use `BUN`, then `~/.bun-1.4.2/bin/bun`, then `bun` on PATH, and
require version 1.4.2. Direct Cargo commands require checkout configuration first.
The root workspace explicitly excludes Exact's independent vendored workspaces.
The pinned Apple host references iOS 27.1 APIs, including `UIArrangementViewController`.
Xcode's iOS 26.5 SDK cannot compile that host; select an Xcode installation with
the required SDK using `DEVELOPER_DIR` before the simulator build.

## Reproduce

1. Install dependencies with `bun install --frozen-lockfile`.
2. Run `bun tools/create-key.mjs` if the private demo key does not exist.
3. Set `DEMO_SIGNING_KEY` to that file. The local default is `$HOME/.config/exact-mf/demo-signing.key`.
4. Run `tools/sign-remotes.sh`.
5. Run `tools/validate.sh` (configures the pinned checkout and Bun for Cargo).
6. Set `EXACT_MF_ORIGIN` to the verified stable HTTPS origin.
7. Run `tools/build-host.sh`.
8. Read `.exact/host-app.txt` for the installable HOST app path.

`public-v1` and `public-v2` are independent publication snapshots. Each includes every referenced blob, the public key, and `inventory.json`.
Relative signed URLs allow origin changes without signing again. Changing the origin requires host repackaging and signing.
The HTML page is only a landing page. Native verification applies to the signed head and blobs, independent of HTML rewrites.
`tools/package-host.mjs` copies only the MF routing bytecode, public key, and origin into the app bundle. It signs and verifies the bundle.
The app writes update storage and mount evidence inside Application Support/ExactMF in its sandbox.
No private signing key enters the app or publication snapshots.

The focused test uses local bytes behind an HTTPS fixture URL. It exercises production signature admission, real MF execution,
remote metadata, distinct plan and entry digests, unchanged-generation skips, commit, and repeated v1/v2 transitions.
It does not prove live HTTPS availability or device rendering.

## Publication and recording

`deployment.json` records the verified Zephyr site and stable HTTPS origin.

1. Open `zackarychapple / exact-mf / visual-remote` in Zephyr.
2. Open Versions and select build 18845 for blue v1 or 18846 for orange v2.
3. Deploy that build to the `demo` environment.
4. Keep the simulator app open while Zephyr propagates the selection.

The app checks every two seconds. Edge propagation can delay the visible change.
Repeat these steps in either direction. Existing signed builds need no rebuild or new publication.
The opt-in policy permits repeated transitions in either direction, including a sandbox that already selected sequence 2.
The predecessor remains active until the native owner accepts the complete candidate and commits its generation token.
The runtime does not require any `/tmp` file.
