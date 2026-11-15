#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
. ./tools/runtime-env.sh
export CARGO_TARGET_DIR="$PWD/target"
cargo test --manifest-path validation/Cargo.toml "$@"
