#!/usr/bin/env bash
# Boots a disc in a private, headless PCSX2 with controller 1 bound to the keyboard, so
# xdotool can play it and take savestates (PLAN.md RE.8). Linux only. Runs until PCSX2 exits
# or this script is interrupted. Details: docs/re-setup.md, "NTSC savestates without a
# capture session".
#
# usage: tools/pine-probe/boot.sh <disc.iso> <name>
#
# Everything the run writes stays under cache/pcsx2/boot/<name>/ (gitignored): the private
# config, screenshots (F8) in xdg/PCSX2/snaps/ and savestates (F1) in xdg/PCSX2/sstates/.
# Memory cards are copies of the owner's; the owner's own files are only read.
#
# Environment overrides:
#   XVFB      Xvfb binary (default: Xvfb on PATH)
#   PCSX2     PCSX2 launcher (default: ~/.local/bin/pcsx2)
#   SPEED     emulation speed (default 1)
#   DISPLAY_NUM, PINE_SLOT   virtual display and PINE slot (defaults 98 and 28013)
#   AXIS_SCALE   scale both sticks (PCSX2's AxisScale), so keys give part tilt: 0.5 is half
#   PAD2=1     bind controller 2 to other keys too (Deadlocked's co-op; docs/re-setup.md)
#   PCSX2_CONFIG   the owner's PCSX2 config folder (default ~/.config/PCSX2)
set -euo pipefail

if [ $# -ne 2 ]; then
  sed -n '7,7p' "$0" | sed 's/^# //'
  exit 2
fi
ISO=$(realpath "$1")
NAME=$2

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WORK="$ROOT/cache/pcsx2/boot/$NAME"
XVFB=${XVFB:-Xvfb}
PCSX2=${PCSX2:-$HOME/.local/bin/pcsx2}
SPEED=${SPEED:-1}
DISPLAY_NUM=${DISPLAY_NUM:-98}
SLOT=${PINE_SLOT:-28013}
OWNER=${PCSX2_CONFIG:-$HOME/.config/PCSX2}

for tool in xdotool xdpyinfo "$XVFB" "$PCSX2"; do
  command -v "$tool" >/dev/null || { echo "missing: $tool (see docs/re-setup.md)" >&2; exit 1; }
done

cargo build --release -q -p pine-probe --manifest-path "$ROOT/Cargo.toml"
PROBE="$ROOT/target/release/pine-probe"
export DISPLAY=":$DISPLAY_NUM"

PCSX2_PID=
XVFB_PID=
cleanup() {
  # PCSX2 aborts if its display disappears while it shuts down, so let it exit first.
  if [ -n "$PCSX2_PID" ] && kill "$PCSX2_PID" 2>/dev/null; then
    for _ in $(seq 100); do kill -0 "$PCSX2_PID" 2>/dev/null || break; sleep 0.1; done
  fi
  if [ -n "$XVFB_PID" ]; then kill "$XVFB_PID" 2>/dev/null || true; fi
}
trap cleanup EXIT

mkdir -p "$WORK/xdg/PCSX2/memcards"
if ! xdpyinfo >/dev/null 2>&1; then
  "$XVFB" "$DISPLAY" -screen 0 1280x960x24 -nolisten tcp >"$WORK/xvfb.log" 2>&1 &
  XVFB_PID=$!
  for _ in $(seq 50); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done
fi

"$PROBE" headless-config --from "$OWNER/inis/PCSX2.ini" \
  --out "$WORK/xdg/PCSX2/inis/PCSX2.ini" --bios-dir "$OWNER/bios" \
  --slot "$SLOT" --speed "$SPEED" --keyboard-pad ${AXIS_SCALE:+--axis-scale "$AXIS_SCALE"} \
  ${PAD2:+--keyboard-pad2}
cp "$OWNER/memcards/Mcd001.ps2" "$OWNER/memcards/Mcd002.ps2" "$WORK/xdg/PCSX2/memcards/"

XDG_CONFIG_HOME="$WORK/xdg" nice -n 10 "$PCSX2" -- "$ISO" >"$WORK/pcsx2.log" 2>&1 &
PCSX2_PID=$!
"$PROBE" status --slot "$SLOT" --wait 60
echo "running on DISPLAY=$DISPLAY; stop with Ctrl-C or by killing PID $PCSX2_PID"
wait "$PCSX2_PID"
