#!/usr/bin/env bash
# Data-hygiene guard (PLAN.md P0.12). The repository is public, so it must never contain
# disc images, anything extracted from them, emulator captures or other large binaries.
#
# usage: tools/check-data-hygiene.sh            check every tracked file (CI)
#        tools/check-data-hygiene.sh --staged   check files staged for commit (pre-commit hook)
#
# Files larger than 1 MB fail unless listed, one path per line, in
# .data-hygiene-allowlist at the repository root.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

if [ "${1:-}" = "--staged" ]; then
  files=$(git diff --cached --name-only --diff-filter=ACMR)
else
  files=$(git ls-files)
fi

# Disc images, game files, savestates, memory cards, input recordings, GS dumps, raw
# memory dumps and pine-probe traces.
banned='\.(iso|cso|chd|bin|img|mdf|nrg|wad|elf|irx|vag|vab|pss|p2s|p2m2|ps2|mcd|mcr|gs|trace)(\.(xz|zst|gz|zip))?$'
max_bytes=$((1024 * 1024))
allowlist=.data-hygiene-allowlist

status=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  lower=$(printf '%s' "$f" | tr '[:upper:]' '[:lower:]')
  if printf '%s' "$lower" | grep -Eq "$banned"; then
    echo "data-hygiene: banned file type: $f" >&2
    status=1
    continue
  fi
  if [ "${1:-}" = "--staged" ]; then
    size=$(git cat-file -s ":$f")
  else
    size=$(wc -c <"$f")
  fi
  if [ "$size" -gt "$max_bytes" ] && ! { [ -f "$allowlist" ] && grep -Fxq "$f" "$allowlist"; }; then
    echo "data-hygiene: $f is $size bytes (over 1 MB and not in $allowlist)" >&2
    status=1
  fi
done <<<"$files"

if [ "$status" -ne 0 ]; then
  echo "data-hygiene: see the legal rules in CLAUDE.md. Disc-derived data belongs in /cache." >&2
fi
exit "$status"
