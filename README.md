# Exact native remote example

This example loads signed Exact plans through Module Federation 2.9.3. Zephyr serves the static artifacts.

The host starts with a neutral loading plan. After its first draw, it polls the configured HTTPS origin every two seconds.
`RemoteCoordinator` admits the signed generation. Upstream MF calls `init`, `get`, and the factory through the native bridge.
The bridge checks remote release metadata against signed assets and the admitted plan digest.
`ExactComposition.applyGeneration` mounts the downloaded plan and commits the matching generation token.
The host does not choose a design from a version number. Neither remote plan is bundled in the host.

The blue editorial panel is `remotes/v1.contract`. The orange welcome layout is `remotes/v2.contract`.
The diagnostic area shows the remote release, verified sequence, entry digest, plan digest, and host session identity.
The session identity remains constant for the controller lifetime. Mount records also contain the process ID.

## Dependencies

Use Bun 1.4.2 and Exact commit `c48c1a39b55862520458a23e020d5bf84dbac12c` at `/Users/zackarychapple/code/t3/exact`.
Cargo manifests currently reference that checkout by path. The generated plans use format 5.
The old format 4 proof and its `/tmp` evidence remain unchanged.
The native MF artifact uses Exact's maintained `buildNativeFederation` API. The Bun lock pins all three upstream packages to 2.9.3.
Use the Hermes compiler from Exact's attested local bundle. `HERMESC` can override its path.

## Reproduce

1. Install dependencies with `bun install --frozen-lockfile`.
2. Run `bun tools/create-key.mjs` if the private demo key does not exist.
3. Set `DEMO_SIGNING_KEY` to that file. The local default is `$HOME/.config/exact-mf/demo-signing.key`.
4. Run `tools/sign-remotes.sh`.
5. Run `CARGO_TARGET_DIR="$PWD/target" cargo test --manifest-path validation/Cargo.toml`.
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
remote metadata, distinct plan and entry digests, unchanged-generation skips, commit, and rollback rejection.
It does not prove live HTTPS availability or device rendering.

## Publication and recording

`deployment.json` records the Zephyr site identifiers. Its hostname is provisional until root verifies it.
Root owns upload, stable promotion, device installation, and recording. Publish v1 before the first launch of a fresh app sandbox.
Promote v2 while the app remains open. Sequence rollback is intentionally refused.
A reused sandbox that admitted sequence 2 cannot return to sequence 1.
The runtime does not require any `/tmp` file.
