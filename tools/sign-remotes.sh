#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
: "${BUN:=$HOME/.bun-1.4.2/bin/bun}"
: "${HERMESC:=$HOME/.cargo/hermes-lean-sys/hermes-vanilla-d412d3bd8512-v5/bc71c7b398f1d41a5390761b75ce72e7be234aa48cb1ddec947224188ed6a4f2/bin/hermesc}"
: "${DEMO_SIGNING_KEY:=$HOME/.config/exact-mf/demo-signing.key}"
export HERMESC DEMO_SIGNING_KEY
export PATH="$(dirname "$BUN"):$PATH"
"$BUN" install --frozen-lockfile
"$BUN" tools/build-remotes.mjs
CARGO_TARGET_DIR="$PWD/target" cargo run --manifest-path tools/Cargo.toml -- "$PWD"
"$BUN" tools/inventory.mjs
