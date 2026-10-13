#!/usr/bin/env bash
# Sends a running UYA NTSC-U (SCUS-97353) or Deadlocked NTSC-U (SCUS-97465) game to another
# level, the way the games' own level change does it: the next level's number, then the
# change request the main loop checks each frame (docs/re-setup.md, "Going straight to a
# level"). Only for the private research instances that tools/pine-probe/boot.sh starts.
#
# usage: tools/pine-probe/warp.sh <level> [--from-state <n>] [--slot <pine slot>]
#
# --from-state loads that savestate slot first. In UYA a warp only works from a level that
# wasn't itself reached by flying in (such as Veldin from a new game): from a planet the
# change goes through space mode and waits there for a destination that was never chosen.
#
# Returns once the level has loaded, the game is back in normal play (game mode 0) after
# any arrival cutscene, and the hero stands idle (or 20 s more have passed). Start skips
# most cutscenes. Level numbers are listed in docs/re-setup.md.
set -euo pipefail

usage() {
  sed -n '7,7p' "$0" | sed 's/^# //' >&2
  exit 2
}
[ $# -ge 1 ] || usage
LEVEL=$1
shift
STATE=
SLOT=28013
while [ $# -gt 0 ]; do
  case "$1" in
    --from-state) STATE=${2:?}; shift 2 ;;
    --slot) SLOT=${2:?}; shift 2 ;;
    *) usage ;;
  esac
done
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
PROBE="$ROOT/target/release/pine-probe"
[ -x "$PROBE" ] || cargo build --release -q -p pine-probe --manifest-path "$ROOT/Cargo.toml"

# re/<serial>/symbols.toml: level_next, level_change_request, level_current, game_mode;
# and the hero's state from re/<serial>/hero.toml.
STATUS=$("$PROBE" status --slot "$SLOT")
case "$STATUS" in
  *SCUS-97353*) NEXT=0x1DFBC4 REQUEST=0x1D6674 CURRENT=0x1D545C MODE=0x1D6694 HERO=0x1A71A4 ;;
  *SCUS-97465*) NEXT=0x222B80 REQUEST=0x21E80C CURRENT=0x21DE10 MODE=0x21DDB4 HERO=0x36B52C ;;
  *)
    echo "slot $SLOT runs neither SCUS-97353 nor SCUS-97465: $STATUS" >&2
    exit 1
    ;;
esac
peek() { "$PROBE" peek --slot "$SLOT" "$1"; }

if [ -n "$STATE" ]; then
  "$PROBE" load-state --slot "$SLOT" "$STATE"
  # PCSX2 loads it a moment later; then wait for normal play.
  sleep 3
  for _ in $(seq 30); do
    [ "$(peek "$MODE")" = 0 ] && break
    sleep 1
  done
fi

"$PROBE" poke --slot "$SLOT" --word "$NEXT" "$LEVEL"
"$PROBE" poke --slot "$SLOT" --word "$REQUEST" 1
for _ in $(seq 300); do
  sleep 1
  if [ "$(peek "$CURRENT")" = "$LEVEL" ] && [ "$(peek "$MODE")" = 0 ]; then
    # Getting out of the ship is still normal play; wait for him to stand.
    for _ in $(seq 20); do
      [ "$(peek "$HERO")" = 0 ] && break
      sleep 1
    done
    echo "level $LEVEL"
    exit 0
  fi
done
echo "level $LEVEL: not in normal play after 5 minutes (level $(peek "$CURRENT"), mode $(peek "$MODE"))" >&2
exit 1
