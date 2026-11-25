#!/usr/bin/env bash
# Run on an isolated X11 display with a window manager, xdotool, and Python 3.
set -euo pipefail

app=$(realpath "${1:-target/release/antios}")
data=$(mktemp -d)
pid=
cleanup() {
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
    wait "$pid" || true
  fi
  rm -rf "$data"
}
trap cleanup EXIT

"$app" --data-dir "$data" --quality low --size 960x600 >"$data/app.log" 2>&1 &
pid=$!
if ! window=$(timeout 25 xdotool search --sync --onlyvisible --pid "$pid" --name '^antiOS$' | head -n 1); then
  cat "$data/app.log"
  exit 1
fi
xdotool windowactivate --sync "$window"
sleep 1.5
xdotool key F11
timeout 10 bash -c 'until xprop -id "$1" _NET_WM_STATE | rg -q "_NET_WM_STATE_FULLSCREEN"; do sleep .1; done' check "$window"
xdotool key Escape
timeout 10 bash -c 'while xprop -id "$1" _NET_WM_STATE | rg -q "_NET_WM_STATE_FULLSCREEN"; do sleep .1; done' check "$window"
xdotool key d g minus
xdotool key Escape
xdotool mousemove --window "$window" 455 370
xdotool mousedown 1
xdotool mousemove --window "$window" 730 330
sleep .4
xdotool mouseup 1
xdotool key space s
timeout 30 bash -c 'until compgen -G "$1/captures/*.png" >/dev/null; do sleep .15; done' check "$data"
xdotool key shift+s
timeout 45 bash -c 'until [[ $(compgen -G "$1/captures/*.png" | wc -l) -eq 2 ]]; do sleep .15; done' check "$data"
xdotool key r Return
sleep 2
xdotool key q Escape
kill -0 "$pid"
xdotool key q Return
for _ in {1..200}; do
  if ! kill -0 "$pid" 2>/dev/null; then break; fi
  sleep .1
done
if kill -0 "$pid" 2>/dev/null; then
  cat "$data/app.log"
  exit 1
fi
wait "$pid"
pid=

python3 - "$data" <<'PY'
import json
from pathlib import Path
import struct
import sys

data = Path(sys.argv[1])
state = json.loads((data / "memory.json").read_bytes())
config = json.loads((data / "preferences.json").read_bytes())
assert state["version"] == 1
assert state["seconds"] > 1
assert len(state["sessions"]) == 2
assert state["sessions"][0]["seed"] != state["sessions"][1]["seed"]
assert len(state["sessions"][1]["survivors"]) == 7
assert config["reduced_motion"] is True
assert config["quality"] == "medium"
assert 0.37 < config["volume"] < 0.39
sizes = set()
for image in (data / "captures").glob("*.png"):
    with image.open("rb") as file:
        header = file.read(24)
    assert header.startswith(b"\x89PNG\r\n\x1a\n")
    sizes.add(struct.unpack(">II", header[16:24]))
assert sizes == {(960, 600), (3840, 2400)}, sizes
print("Native X11 integration passed")
PY

"$app" --data-dir "$data/installation" --quality low --exhibition --duration 2 >"$data/installation.log" 2>&1
python3 - "$data/installation/memory.json" <<'PY'
import json
from pathlib import Path
import sys
assert json.loads(Path(sys.argv[1]).read_bytes())["seconds"] >= 1
print("Timed exhibition exit passed")
PY
