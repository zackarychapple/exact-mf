#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
. ./tools/runtime-env.sh
: "${EXACT_MF_ORIGIN:=$("$BUN" -e 'console.log(require("./deployment.json").origin)')}"
printf '%s' "$EXACT_MF_ORIGIN" > resources/origin.txt
export BUN
export PATH="$(dirname "$BUN"):$PATH"
export EXACT_APP_DIR="$PWD"
export CARGO_TARGET_DIR="$PWD/target"
"$BUN" "$EXACT2/host/apple/build.mjs" exact-mf-apple-async-app-apple --ios --host --no-install
"$BUN" tools/package-host.mjs
