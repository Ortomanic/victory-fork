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

## Settings and results here

Series S profile, a PC FidelityFX upscaler DLL, `VKD3D_CONFIG=no_upload_hvv`, `VICTORY_LEVELS_ON_DEMAND` unset
(as the willy package found), `VICTORY_RELAY_COPY=0` (ad3lre's fork links it to device removal on NVIDIA),
renderer ports 1 and 5 on. From a save in the open world, card otherwise idle: 68 ms a frame (about 15 fps) at
5,000 to 7,000 draws, 11.2 GB of the card at most, no refused texture uploads, picture lit and textured on foot
and driving. One run; a handful of shorter ones agree. Not yet played for a long session.

On Wine the largest single gain is outside this tree: Wine's server walks a list of the process's views on
every map and unmap (Wine bug 60868); with a tree lookup the frame went from about 105 to about 80 ms here
(fxceless/victory-fork#118).

## Known problems

- 27 console programs written for 64-lane waves have no 32-lane form here and are left out.
- About 2.5 GB of the card is taken by the driver outside any allocation while vkd3d-proton uses
  `VK_EXT_descriptor_heap` (measured here; not solved).
- Ray-traced light does not run; a stand-in lights what the sun does not reach.
