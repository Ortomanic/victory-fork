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

- **Ray tracing is on by default.** The structure builder and the programs that reach memory by address run
  unless `VICTORY_BVH_BUILDER=0` and `VICTORY_RAYS=0` are set. The code is the base's (avatarmanz's and
  DoubleGhosty's work); here only the default changed, because on this PC all 53 builder programs get host
  pipelines and it runs without a fault at the same frame time as without it. Whether traced light is what
  lights the shade has not been shown yet: the ambient stand-in was on in every run.
- **Hair strands are drawn by default** (`VICTORY_HAIR=0` leaves them out, as cool_fox `f4164f8` did). Without
  them the player's head is bare here; with them the hair is right.
- **One back edge per loop in emitted DXIL** (above).

## Settings and results here

Series S profile, a PC FidelityFX upscaler DLL, `VKD3D_CONFIG=no_upload_hvv` and:

```
VICTORY_AMBIENT_AIR=1.5 VICTORY_AMBIENT=0.8 VICTORY_TEXTURE_NO_WAIT=1 VICTORY_RELAY=2 VICTORY_WIDE_BACK=1
VICTORY_CARD_REHASH_RUNS=1 VICTORY_JOIN_COPIES=1 VICTORY_JOIN_PARTS=8 VICTORY_INDIRECT_WINDOW=1
VICTORY_INDIRECT_BATCH=2 VICTORY_CARD_STEPS=2 VICTORY_CARD_AWAKE=1 VICTORY_CARD_BUFFERS_MIN_KB=64
VICTORY_MAP_THREADS=1 VICTORY_SHOW_AFTER=1 VICTORY_THIN=1 VICTORY_UPLOAD_AHEAD=1 VICTORY_PORT_CBV=1
VICTORY_PORT_CONSTANTS=1 VICTORY_VRAM_IDLE_MS=6000,1500,700 VICTORY_OVERLAY=0 VICTORY_PIPELINE_BOOK=1
VICTORY_RELAY_COPY=0
```

`VICTORY_LEVELS_ON_DEMAND` is left unset, as the willy package found. `VICTORY_RELAY_COPY=0` differs from
`Play-VIctory.cmd`, which sets 1: ad3lre's fork links 1 to device removal on NVIDIA, and the device faults seen
here stopped once it was 0 (a handful of runs, not a proof). The `.cmd` launchers are the other forks' and have
not been run by us; nothing here has been tested on Windows.

Measured from a save in the open world, 5,000 to 7,000 draws a frame:

| | Frame | Whole card in use, at most |
|---|---|---|
| Rays off, card otherwise idle (one run) | 68 ms, about 15 fps | 11.2 GB |
| Rays off, another game holding about 3.2 GB (four runs) | 78 to 82 ms | 9.5 to 9.8 GB |
| Rays on, the same other game running (three runs) | 69 to 85 ms once settled | 11.1 to 12.1 GB |

No refused texture uploads and no device fault in any of them; picture lit and textured on foot and driving.
The card figures are for the whole card, desktop and the other game included. No run with rays on has had the
card to itself, and none is longer than six minutes.

**On a 12 GB card** the texture budget follows the card, so it should fit, but rays cost roughly 2 GB more
here. If textures are refused or the frame collapses, try `VICTORY_BVH_BUILDER=0 VICTORY_RAYS=0` first, then
`VICTORY_TEXTURE_MAX_DIM=1024` (ad3lre's cap).

**Warm the caches first.** `victory.exe shaders <game folder>\shaders.rpf` translates every shader (about two minutes here);
after one played session, `victory.exe warm --used` makes the pipelines that session drew (3,187 in
three and a half minutes here). Without them passes are missing until their pipelines are made: on a first
start with rays on the water was not drawn at all.

On Wine the largest single gain is outside this tree: Wine's server walks a list of the process's views on
every map and unmap (Wine bug 60868); with a tree lookup the frame went from about 105 to about 80 ms here
(fxceless/victory-fork#118).

## Known problems

- With rays on, every start spends its first one to two minutes in the world at about 3 fps while some 400
  pipelines are made, also after `warm --used`; then it settles. Under vkd3d-proton the host pipeline library is
  refused ("no host pipeline library: pipelines are not kept"), which may be why; not looked into yet.
- Three compute programs never get a host pipeline with rays on and their dispatches are dropped
  (`02384a60fda6f2d0`, `5dc3b25986622949`, `6f225b641c5b0708`); nothing is visibly missing.
- 27 console programs written for 64-lane waves have no 32-lane form here and are left out.
- About 2.5 GB of the card is taken by the driver outside any allocation while vkd3d-proton uses
  `VK_EXT_descriptor_heap` (measured here; not solved).
