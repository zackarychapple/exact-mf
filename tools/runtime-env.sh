# Source after changing to the application root.
if [ -z "${BUN:-}" ]; then
  if [ -x "$HOME/.bun-1.4.2/bin/bun" ]; then
    BUN="$HOME/.bun-1.4.2/bin/bun"
  else
    BUN="$(command -v bun)"
  fi
fi
if [ "$("$BUN" --version)" != "1.4.2" ]; then
  echo "This example requires Bun 1.4.2; set BUN to that executable." >&2
  exit 1
fi
EXACT2="$("$BUN" tools/configure-exact.mjs)"
export BUN EXACT2
