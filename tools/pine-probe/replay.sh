#!/usr/bin/env bash
# Replays a PCSX2 input recording in a private, headless PCSX2 and records the hero's
# state every frame with pine-probe (PLAN.md RE.5). Linux only. Details: docs/re-setup.md.
#
# usage: tools/pine-probe/replay.sh <disc.iso> <recording.p2m2> <layout.toml> <out.trace>
#
# The recording's savestate must sit next to it as `<recording.p2m2>_SaveState.p2s`.
# Everything the run writes stays under cache/ (gitignored). The owner's own PCSX2
# settings, memory cards and savestates are only read, never modified.
#
# Environment overrides:
#   XVFB      Xvfb binary (default: Xvfb on PATH)
#   PCSX2     PCSX2 launcher (default: ~/.local/bin/pcsx2)
#   SPEED     emulation speed, 0.5 = half speed (default 0.5; slower is safer for capture)
#   DISPLAY_NUM, PINE_SLOT   virtual display and PINE slot (defaults 99 and 28012)
#   PCSX2_CONFIG   the owner's PCSX2 config folder (default ~/.config/PCSX2)
set -euo pipefail

if [ $# -ne 4 ]; then
  sed -n '5,6p' "$0" | sed 's/^# //'
  exit 2
fi
ISO=$(realpath "$1")
REC=$(realpath "$2")
LAYOUT=$(realpath "$3")
OUT=$(realpath -m "$4")
STATE="${REC}_SaveState.p2s"

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
HEADLESS="$ROOT/cache/pcsx2/headless"
XVFB=${XVFB:-Xvfb}
PCSX2=${PCSX2:-$HOME/.local/bin/pcsx2}
SPEED=${SPEED:-0.5}
DISPLAY_NUM=${DISPLAY_NUM:-99}
SLOT=${PINE_SLOT:-28012}
OWNER=${PCSX2_CONFIG:-$HOME/.config/PCSX2}

for tool in xdotool xdpyinfo "$XVFB" "$PCSX2"; do
  command -v "$tool" >/dev/null || { echo "missing: $tool (see docs/re-setup.md)" >&2; exit 1; }
done
[ -f "$STATE" ] || { echo "missing savestate: $STATE" >&2; exit 1; }

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

# A virtual display keeps PCSX2 off the owner's desktop and lets xdotool drive its menus
# without touching the real keyboard focus.
if ! xdpyinfo >/dev/null 2>&1; then
  mkdir -p "$HEADLESS"
  "$XVFB" "$DISPLAY" -screen 0 1280x960x24 -nolisten tcp >"$HEADLESS/xvfb.log" 2>&1 &
  XVFB_PID=$!
  for _ in $(seq 50); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done
fi

# Private PCSX2 data folder, selected through XDG_CONFIG_HOME.
mkdir -p "$HEADLESS/xdg/PCSX2/memcards"
"$PROBE" headless-config --from "$OWNER/inis/PCSX2.ini" \
  --out "$HEADLESS/xdg/PCSX2/inis/PCSX2.ini" --bios-dir "$OWNER/bios" \
  --slot "$SLOT" --speed "$SPEED"
cp "$OWNER/memcards/Mcd001.ps2" "$OWNER/memcards/Mcd002.ps2" "$HEADLESS/xdg/PCSX2/memcards/"
"$PROBE" memcard-sync --state "$STATE" \
  --card1 "$HEADLESS/xdg/PCSX2/memcards/Mcd001.ps2" \
  --card2 "$HEADLESS/xdg/PCSX2/memcards/Mcd002.ps2"

XDG_CONFIG_HOME="$HEADLESS/xdg" nice -n 10 "$PCSX2" -- "$ISO" >"$HEADLESS/pcsx2.log" 2>&1 &
PCSX2_PID=$!

"$PROBE" status --slot "$SLOT" --wait 60
sleep 5 # let the boot settle before opening menus

# Tools > Input Recording > Play, then type the path into Qt's file dialog. Playback
# loads the recording's savestate and leaves the emulator paused on frame 0.
MAIN=$(xdotool search --sync --onlyvisible --classname . | head -n 1)
xdotool windowfocus --sync "$MAIN"
for key in alt+t i p; do xdotool key "$key"; sleep 0.5; done
sleep 1
xdotool type --delay 5 "$REC"
xdotool key Return
sleep 3

"$PROBE" capture --layout "$LAYOUT" --out "$OUT" --slot "$SLOT" --until-idle 5 &
CAPTURE_PID=$!
sleep 1

# Unpause with PCSX2's default Space hotkey, sent to the render widget (the last
# visible window).
RENDER=$(xdotool search --onlyvisible --classname . | tail -n 1)
xdotool windowfocus --sync "$RENDER" key space

wait "$CAPTURE_PID"
FRAMES=$("$PROBE" recording "$REC" --header-only | sed -E 's/.*: ([0-9]+) frames.*/\1/')
echo "recording has $FRAMES frames; trace written to $OUT"
