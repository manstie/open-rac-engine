#!/usr/bin/env bash
# Imports a PCSX2 savestate's EE memory into a headless Ghidra project and analyses it
# (PLAN.md RE.1, RE.3). Details: docs/re-setup.md, "Static analysis".
#
# usage: tools/ghidra/import-savestate.sh <state.p2s> <program name> [symbols.toml]
#
# Writes cache/ghidra/ee/<name>.ee.bin and the project cache/ghidra/proj/<name>. Both are
# derived from the disc, so they stay in cache/ (gitignored). Takes about 3 minutes.
#
# Environment overrides:
#   GHIDRA_HEADLESS   analyzeHeadless launcher (default ~/.local/bin/ghidra-headless)
set -euo pipefail

if [ $# -lt 2 ] || [ $# -gt 3 ]; then
  sed -n '5,5p' "$0" | sed 's/^# //'
  exit 2
fi
STATE=$(realpath "$1")
NAME=$2
SYMBOLS=${3:+$(realpath "$3")}

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
SCRIPTS="$ROOT/tools/ghidra"
GHIDRA=${GHIDRA_HEADLESS:-$HOME/.local/bin/ghidra-headless}
EE="$ROOT/cache/ghidra/ee/$NAME.ee.bin"
PROJECTS="$ROOT/cache/ghidra/proj"

cargo build --release -q -p pine-probe --manifest-path "$ROOT/Cargo.toml"
PROBE="$ROOT/target/release/pine-probe"

mkdir -p "$(dirname "$EE")" "$PROJECTS"
"$PROBE" savestate-extract "$STATE" eeMemory.bin -o "$EE"
GP=$("$PROBE" savestate-regs "$STATE" | awk '$1 == "gp" { sub(/^0x/, "", $2); print $2 }')
echo "gp = 0x$GP"

POST=()
if [ -n "$SYMBOLS" ]; then
  POST=(-postScript ApplySymbols.java "$SYMBOLS")
fi
"$GHIDRA" "$PROJECTS" "$NAME" -import "$EE" \
  -processor r5900:LE:32:default -loader BinaryLoader -loader-baseAddr 0 \
  -scriptPath "$SCRIPTS" -preScript SeedFunctions.java "$GP" "${POST[@]}" \
  -analysisTimeoutPerFile 3600 >"$PROJECTS/$NAME-import.log" 2>&1 \
  || { tail -n 20 "$PROJECTS/$NAME-import.log" >&2; exit 1; }
LOG="$PROJECTS/$NAME-import.log"
grep -E "SeedFunctions.java>|ApplySymbols.java>|REPORT: (Analysis|Import)" "$LOG"
# Script failures don't stop analyzeHeadless, so check for them here.
if grep -q "SCRIPT ERROR" "$LOG"; then
  grep -A3 "SCRIPT ERROR" "$LOG" >&2
  exit 1
fi
