#!/usr/bin/env bash
# Runs the title under Wine with the settings this branch is tested with (see LINUX-MIX.md).
# A courtesy starting point: the tests behind LINUX-MIX.md were run through a sandboxed harness of ours that
# sets the same variables; this script itself has only been dry-run. Nothing here is specific to one PC.
#
#   VICTORY_GAME_DIR=/path/to/your/dump ./play-linux.sh            # run
#   VICTORY_GAME_DIR=/path/to/your/dump ./play-linux.sh --dry-run  # print what would run
#
# You need: Wine with vkd3d-proton's d3d12.dll and d3d12core.dll in the prefix (WINEPREFIX is respected), a
# built victory.exe (VICTORY_EXE, default: target/x86_64-pc-windows-msvc/release/victory.exe beside this
# script), and your own dump of the title. Optional: FSR_DLL=/path/to/amd_fidelityfx_upscaler_dx12.dll from a
# PC game you own. Anything you set yourself is kept; `VAR=value ./play-linux.sh` overrides one setting.
set -euo pipefail
here=$(dirname -- "$(realpath -- "$0")")
dry=0; [[ ${1:-} == --dry-run ]] && { dry=1; shift; }
: "${VICTORY_GAME_DIR:?set VICTORY_GAME_DIR to the folder with game_scarlett_master.exe}"
exe=${VICTORY_EXE:-$here/target/x86_64-pc-windows-msvc/release/victory.exe}
game=$VICTORY_GAME_DIR/game_scarlett_master.exe
[[ -f $game ]] || { echo "not found: $game" >&2; exit 2; }
[[ -f $exe ]] || { echo "not found: $exe (set VICTORY_EXE)" >&2; exit 2; }
command -v wine >/dev/null || { echo "wine is not on PATH" >&2; exit 2; }

# NVIDIA's shader cache in a folder of its own, large, and never trimmed. The driver gives every process that
# has the cache open its own file and reads only that one, and a file that has grown for weeks can stop taking
# entries; then the ray-tracing pipelines are compiled again at every start (minutes at 3 fps). With a folder
# of its own the first start is slow once and later ones are not. End a session by quitting, not by killing
# it: what a killed one compiled seems not to be kept. Other drivers ignore these.
cache=${XDG_CACHE_HOME:-$HOME/.cache}/victory-linux-mix
mkdir -p "$cache/nvidia" "$cache/victory"
export __GL_SHADER_DISK_CACHE_PATH="${__GL_SHADER_DISK_CACHE_PATH:-$cache/nvidia}"
export __GL_SHADER_DISK_CACHE_SIZE="${__GL_SHADER_DISK_CACHE_SIZE:-10737418240}"
export __GL_SHADER_DISK_CACHE_SKIP_CLEANUP="${__GL_SHADER_DISK_CACHE_SKIP_CLEANUP:-1}"
# Upload memory in system RAM: about a fifth faster here (104 against 125 ms a frame on another build).
export VKD3D_CONFIG="${VKD3D_CONFIG-no_upload_hvv}"

# The settings of LINUX-MIX.md. VICTORY_RELAY_COPY=1 is about 15% faster here, but a played session with it
# showed black artifacts in the sky and ended in a device fault on NVIDIA; see LINUX-MIX.md.
while read -r name value; do
  [[ -n ${!name+x} ]] || export "$name=$value"
done <<'SETTINGS'
VICTORY_AMBIENT_AIR 1.5
VICTORY_AMBIENT 0.8
VICTORY_TEXTURE_NO_WAIT 1
VICTORY_RELAY 2
VICTORY_WIDE_BACK 1
VICTORY_CARD_REHASH_RUNS 1
VICTORY_JOIN_COPIES 1
VICTORY_JOIN_PARTS 8
VICTORY_INDIRECT_WINDOW 1
VICTORY_INDIRECT_BATCH 2
VICTORY_CARD_STEPS 2
VICTORY_CARD_AWAKE 1
VICTORY_CARD_BUFFERS_MIN_KB 64
VICTORY_MAP_THREADS 1
VICTORY_SHOW_AFTER 1
VICTORY_THIN 1
VICTORY_UPLOAD_AHEAD 1
VICTORY_PORT_CBV 1
VICTORY_PORT_CONSTANTS 1
VICTORY_VRAM_IDLE_MS 6000,1500,700
VICTORY_OVERLAY 0
VICTORY_PIPELINE_BOOK 1
VICTORY_RELAY_COPY 0
VICTORY_LOG warn
SETTINGS
# Hair strands are on by default in this branch; ray tracing is off (it ran at 1.5 fps away from the start
# here): VICTORY_BVH_BUILDER=1 VICTORY_RAYS=1 ./play-linux.sh tries it. On a card with less room:
#   VICTORY_TEXTURE_MAX_DIM=1024 ./play-linux.sh

if (( dry )); then
  env | grep -E '^(VICTORY_|__GL_SHADER|VKD3D_CONFIG)' | sort
  echo "VICTORY_CACHE_DIR, VICTORY_FSR_DLL and the title's path are converted with winepath when run"
  echo "wine $exe run <$game> --console ${VICTORY_CONSOLE:-series-s} --execute $*"
  exit 0
fi
export VICTORY_CACHE_DIR="${VICTORY_CACHE_DIR:-$(winepath -w "$cache/victory")}"
[[ -z ${FSR_DLL:-} ]] || export VICTORY_FSR_DLL="$(winepath -w "$FSR_DLL")"
exec wine "$exe" run "$(winepath -w "$game")" --console "${VICTORY_CONSOLE:-series-s}" --execute "$@"
