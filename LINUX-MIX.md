# linux-mix: a Linux (Wine + vkd3d-proton, NVIDIA) working branch

This branch is a combination of other people's work, kept buildable and tested on one Linux PC
(CachyOS, RTX 5080 16 GB, Ryzen 7 9800X3D, Wine 11.22, vkd3d-proton 3.1, NVIDIA 620.73). It is not a
project of its own: what it does well comes from the trees below. No game files are here; see
`docs/asset-policy.md`.

## What it is made of

| Part | From | By |
|---|---|---|
| Base: threading, write relay, card fills, residency, renderer ports, Wine fixes | [DoubleGhosty/cool_fox](https://github.com/DoubleGhosty/cool_fox) at `754cafc`, the continuation of codec_d's cool_fox, on [avatarmanz/VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX), [fxceless/victory-fork](https://github.com/fxceless/victory-fork) and [Marvisak/VIctory](https://github.com/Marvisak/VIctory) | their authors, as the history shows |
| Texture dimension cap, idle-texture override, pool caps, memory census | [ad3lre/victory-nvidia-8gb](https://github.com/ad3lre/victory-nvidia-8gb) at `9e87ec8` (the state before its telemetry was added), with soldsawn's NVIDIA work it carries | ad3lre, soldsawn, Dabagh, BvtterFvcker and others |
| Face bake timing and 16-bit views (fxceless PR #927 by tippythetaidum), NaN guard, R11G11B10 stand-in, FidelityFX redirect, keyboard and mouse | the cool_fox "willy" source package, build `30e54b4` (its README credits Neptune-VE, proneonn/Vertex and others) | its authors; committed here as packaged |
| One back edge per loop in emitted DXIL, including a block that continues two loops | [Ortomanic/victory-fork](https://github.com/Ortomanic/victory-fork) (fxceless PR #257) | Ortomanic |

The merge commits name the exact commits taken. Where two trees had the same feature, the base's was kept.

## Why the loop fix replaces the base's

The base carries a port of Marvisak/VIctory#1885 for the same problem. Under vkd3d-proton it still asserted
at `cfg_structurizer.cpp:3486` on this title (one pipeline never finished, 350 ms a frame); with the pass from
PR #257 there are none.

## What this branch changes over its parts

- **Ray tracing runs here, and is off by default** (`VICTORY_BVH_BUILDER=1 VICTORY_RAYS=1` turn it on). The
  code is the base's (avatarmanz's and DoubleGhosty's work). On this PC all 53 builder programs get host
  pipelines and it runs without a fault; around the save's house it costs nothing measurable. Ridden away
  from the house on a bike, the card took 550 to 680 ms a frame (1.5 fps) in two runs at the same place, with
  card memory at 7.9 of 10 GB and no pipeline being made; with rays off the same ride holds its rate (the
  player's report, not a logged run). It was this branch's default for one day, on tests that never left the
  house. Whether traced light is what lights the shade has not been shown: the ambient stand-in was on.
- **Hair strands are drawn by default** (`VICTORY_HAIR=0` leaves them out, as cool_fox `f4164f8` did). Without
  them the player's head is bare here; with them the hair is right.
- **One back edge per loop in emitted DXIL** (above).

## Settings and results here

`./play-linux.sh` sets all of the below, the driver cache folder from the known problems included
(`VICTORY_GAME_DIR=/path/to/your/dump ./play-linux.sh`, `--dry-run` to see what it would do). It is a
starting point: our runs went through a sandboxed harness that sets the same variables, and the script itself
has only been dry-run.

Series S profile, a PC FidelityFX upscaler DLL, `VKD3D_CONFIG=no_upload_hvv` and:

```
VICTORY_AMBIENT_AIR=1.5 VICTORY_AMBIENT=0.8 VICTORY_TEXTURE_NO_WAIT=1 VICTORY_RELAY=2 VICTORY_WIDE_BACK=1
VICTORY_CARD_REHASH_RUNS=1 VICTORY_JOIN_COPIES=1 VICTORY_JOIN_PARTS=8 VICTORY_INDIRECT_WINDOW=1
VICTORY_INDIRECT_BATCH=2 VICTORY_CARD_STEPS=2 VICTORY_CARD_AWAKE=1 VICTORY_CARD_BUFFERS_MIN_KB=64
VICTORY_MAP_THREADS=1 VICTORY_SHOW_AFTER=1 VICTORY_THIN=1 VICTORY_UPLOAD_AHEAD=1 VICTORY_PORT_CBV=1
VICTORY_PORT_CONSTANTS=1 VICTORY_VRAM_IDLE_MS=6000,1500,700 VICTORY_OVERLAY=0 VICTORY_PIPELINE_BOOK=1
VICTORY_RELAY_COPY=0
```

`VICTORY_LEVELS_ON_DEMAND` is left unset, as the willy package found. **`VICTORY_RELAY_COPY=0`** differs from
`Play-VIctory.cmd`, which sets 1. With 1 the frame is about 15% shorter here (table below) and eight unattended
runs and two sessions of 14 and 35 minutes had no fault; then the first played session with it showed black
artifacts popping into the sky and ended in a device fault (Xid 13) after six minutes. ad3lre's fork links 1
to device removal on NVIDIA as well. It stays 0. The `.cmd` launchers are the other forks' and have not been
run by us; nothing here has been tested on Windows.

Measured on 2026-11-27 from a save in the open world, the card to itself, standing by the house, rays on (they
cost nothing there, last row), 1920x1080 output unless
said, each run 40 or more settled readings at 5,000 to 7,000 draws a frame:

| | Frame |
|---|---|
| `VICTORY_RELAY_COPY=0` (three runs) | 62 to 63 ms, 16 fps |
| `VICTORY_RELAY_COPY=1` (six runs) | 53 to 55 ms, 18 to 19 fps |
| the same, `VICTORY_OUTPUT=1664x936` | 51 ms, 19.6 fps |
| the same, `VICTORY_OUTPUT=1280x720` | 50 ms, 20.2 fps |
| rays off (`VICTORY_BVH_BUILDER=0 VICTORY_RAYS=0`), relay copy 0 | 72 against 73 ms in the same hour: no difference |

A 35 minute unattended session of scripted walking and jumping with `VICTORY_RELAY_COPY=1` ran its whole
length with no device fault (a played one did not, see above): 53 ms at 5,000 to 7,000 draws, 57 ms at 7,000 to 9,500, 62 ms at 9,500 to 13,000. The whole card
held 10.5 to 13 GB (desktop included), no texture upload was refused, and the picture stayed lit and textured.
In about 70 starts that night the device faulted once, at the menu (Xid 13, with relay copy 0).

Not gains here, each one run or a few: a `znver4` build, `VICTORY_SUBMIT_FRAMES` 0/2/3, `VICTORY_WIDE_SPIN`,
`VICTORY_THIN=0 VICTORY_JOIN_PARTS=0`, half render scale, foxhound's own build and settings (56 to 60 ms with
relay copy 1). Other programs on the card cost a great deal: with a desktop stream idling at 20% of the card
the same runs read 72 to 74 ms.

Settings reach the title through the environment; a sandbox that clears it must pass each `VICTORY_*` one on.

**On a 12 GB card** the texture budget follows the card, so it should fit. If textures are refused, try
`VICTORY_TEXTURE_MAX_DIM=1024` (ad3lre's cap).

**Translate the shaders first.** `victory.exe shaders <game folder>\shaders.rpf` translates every shader (about two minutes here);
`victory.exe warm --used` makes the pipelines earlier sessions drew, but with NVIDIA on Linux what it
compiles does not reach the game (see the known problems); there the first session fills the cache.

On Wine the largest single gain is outside this tree: Wine's server walks a list of the process's views on
every map and unmap (Wine bug 60868); with a tree lookup the frame went from about 105 to about 80 ms here
(fxceless/victory-fork#118).

## Known problems

- **Slow starts with rays on were the driver's shader cache, and are solved here by a fresh one.** NVIDIA's
  Linux driver gives each process that has the cache open a file of its own and reads only that one, so what
  the processes of `warm --used` compile never reaches the game (the same 150 pipelines: 49 s, then 2 s again
  in one process, then 52 s when ten processes shared the folder). The game's own file had also stopped
  growing at 7.4 GB, so its ray-tracing pipelines were compiled again at every start: two minutes at 3 fps.
  With `__GL_SHADER_DISK_CACHE_PATH` pointing at an empty folder the game fills the cache itself: the first
  start is that slow once, later ones mostly show the world at once. Not every one: 5 of 33 starts
  still made some 400 pipelines again, after sessions that were killed and after ones that ended by
  themselves, with the same translations logged. Why is not known.
- `VICTORY_PIPELINE_WAIT_MS=0` is no way around the first start: the frame rate holds, but for those three
  minutes the picture is glare and lights on black.
- Memory of our own grows while roaming: 6.6 to 10.8 GB private in 30 minutes (about 140 MB a minute). A
  session of some hours will want watching on a PC with less than 64 GB.
- DoubleGhosty/cool_fox after `92afbcf` is not merged yet. Tried at `69718c0`: `2a1965f` (a dispatch waits for
  its compute pipeline) ends the title at loading here (a write to 0, with rays on), and `76c4a28` (renderer
  port 1, in-place arguments clamped alone) takes the card's share of a frame from 25 to 82 ms unless
  `VICTORY_PORT_CONSTANTS_ORDER_ALONE=0`. With the first reverted and the second switched off it measures the
  same as this branch or a little slower (54 to 58 ms).
- Three compute programs never get a host pipeline with rays on and their dispatches are dropped
  (`02384a60fda6f2d0`, `5dc3b25986622949`, `6f225b641c5b0708`); nothing is visibly missing.
- 27 console programs written for 64-lane waves have no 32-lane form here and are left out.
- About 2.5 GB of the card is taken by the driver outside any allocation while vkd3d-proton uses
  `VK_EXT_descriptor_heap` (measured here; not solved).
