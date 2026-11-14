#!/usr/bin/env bash
# The UI parity check, one screen end to end (docs/OBSERVABILITY.md, "UI
# parity against the look book"):
#
#   tools/ui-parity/run.sh <board|chip|chip_lead|feed|lobby|menu> [out-dir]
#
# 1. mockup.mjs renders the look book's frame (reference/) at 1920x1080
#    and measures every data-ui element;
# 2. the Ultimate Gun Game, offscreen at 1920x1080, stages the same screen
#    with the same fixture (`ui_scene <scene> <fixture>`) over a flat ground, dumps its
#    UiParityId nodes (`ui_dump`) and takes a screenshot;
# 3. compare.mjs writes <out>/<screen>.txt (mismatches ranked by error),
#    <screen>.side.png and <screen>.overlay.png.
#
# out-dir defaults to target/ui-parity. The game runs as
# `$UI_PARITY_GAME` when set (a built binary, e.g.
# target/debug/ultimate_gun_game), else `cargo run --features dev --bin
# ultimate_gun_game --`. It reads the games' install paths from the local
# config like any run (CHIMERA_CONFIG). Exit status: compare's (1 when
# anything is past tolerance).
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../.." && pwd)
screen=${1:?usage: tools/ui-parity/run.sh <board|chip|chip_lead|feed|lobby|menu> [out-dir]}
out=${2:-$repo/target/ui-parity}
fixture=${UI_PARITY_FIXTURE:-$here/fixtures/hud.json}
mkdir -p "$out"
stage=$(node -e 'const s=require(process.argv[1])[process.argv[2]]; if(!s){process.exit(2)} console.log(s.stage)' "$here/screens.json" "$screen") \
  || { echo "ui-parity: no screen $screen in screens.json" >&2; exit 2; }

node "$here/mockup.mjs" "$screen" "$out/$screen.mockup" --fixture "$fixture"

if [ -n "${UI_PARITY_GAME:-}" ]; then
  # shellcheck disable=SC2206
  game=($UI_PARITY_GAME)
else
  game=(cargo run -q --features dev --bin ultimate_gun_game --)
fi
rm -f "$out/$screen.game.json" "$out/$screen.game.png"
(cd "$repo" && "${game[@]}" --offscreen 1920x1080 --frames 150 --screenshot "$out/$screen.game.png" \
  +ui_scene "$stage" "$fixture" +ui_dump "$out/$screen.game.json" 90) > "$out/$screen.game.log" 2>&1 \
  || { echo "ui-parity: the game failed (log: $out/$screen.game.log)" >&2; tail -20 "$out/$screen.game.log" >&2; exit 2; }
[ -s "$out/$screen.game.json" ] || { echo "ui-parity: the game wrote no dump (log: $out/$screen.game.log)" >&2; exit 2; }

node "$here/compare.mjs" "$out/$screen.mockup" "$out/$screen.game" "$out/$screen"
