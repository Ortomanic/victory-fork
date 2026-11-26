# cool_fox integration for NVIDIA 8 GB

The integration starts from the complete [cool_fox tree at f4b48ad](https://github.com/DoubleGhosty/cool_fox/tree/f4b48ad766ddbec1a1867022cd42f7645b0dd55e).
The NVIDIA baseline is [150ba23](https://github.com/ad3lre/victory-nvidia-8gb/tree/150ba23379dddc934df31992eba7989dace0e8d2).
There were 186 NVIDIA-side and 395 cool_fox-side commits since their common
ancestor. A trial merge conflicted in 32 files, including submission,
descriptor, shader, resource, and memory-watch code.

| Work | What the cool_fox code changes |
| --- | --- |
| CPU submission | It defers descriptor-table work until draws need it, spreads refreshes and command-list translation across threads, and batches submission parts and indirect work. |
| Shared-memory emulation | It narrows write-watch and copy-back work, uses card-side hashing and parallel AVX2 comparisons, and copies shader-written textures back to title memory for CPU consumers. |
| Memory pressure | It reserves room for render targets, scopes bindless refreshes to the selected heap, bounds speculative staging, and backs off failed pool growth or uploads when system commit is low. |
| Compatibility | It includes the Series S two-plane present fix, NVIDIA Wave32 shader lowering, movie-plane upload fixes, and XMA audio fixes. |

The [RX 7900 XTX measurements](https://github.com/DoubleGhosty/cool_fox/blob/f4b48ad766ddbec1a1867022cd42f7645b0dd55e/docs/findings.md)
attribute the largest early CPU improvement to deferring draw-table
descriptor records (280 ms to about 124 ms). Later measurements dropped
from 103 to 75 ms after moving preparation work to a separate thread and
parallelizing refreshes. Upstream reports 13–20 FPS indoors on Series X
at 1440p. The [NVIDIA baseline](https://github.com/ad3lre/victory-nvidia-8gb/blob/150ba23379dddc934df31992eba7989dace0e8d2/README.md)
reports 8–10 FPS on an RTX 4060 Laptop at 1080p in Series X with two
mips dropped. The RTX-oriented pareto launcher defaults to Series S.
These are different GPUs, workloads, and configurations.

The integration retains thin LTO, the optional resolution cap and
borderless workaround, Agility SDK 620 exports, and the optional
`VICTORY_VRAM_IDLE_MS` override.

## NVIDIA features and experiments

| Feature or original switch | Integration status |
| --- | --- |
| F10 performance overlay | Restored from the Neptune-derived NVIDIA overlay. It starts hidden in pareto launches and visible in the legacy NVIDIA launcher. `VICTORY_OVERLAY=0` and `-NoOverlay` disable it. |
| `VICTORY_TEXTURE_MAX_DIM` | Restored as an opt-in cap on title-memory uploads for eligible tiled 2D textures. It requires `VICTORY_LEVELS_ON_DEMAND=1` and tiled resource support. GPU writes can still make levels resident. |
| `VICTORY_RAM_TRIM_MB` | Restored as an opt-in free-physical-memory threshold. It releases spare scratch and snapshots, trims reset lists, and lowers idle RAM retention. |
| `VICTORY_CLAMP_WAITS=0` | Restored for experiments on the single-clamp fallback. Leaving the waits enabled is the default. Validate indirect arguments and output on hardware before using it in a preset. |
| `VICTORY_LOOKUP_CACHE` and `VICTORY_BIND_CACHE` | The current renderer caches buffer lookups per thread and render-target binds per list. The old lookup switch is accepted as an alias when `VICTORY_QUICK_FIND` is unset. |
| `VICTORY_STEP_INDEX` | Current submission preparation indexes refreshes and shader-write ranges by copy and deduplicates ranges. The old experimental switch itself has no effect. |
| `VICTORY_CARD_SYNC_BATCH` | The current renderer records card uploads, transitions, and rehashes in distinct phases. The older toggle has no equivalent off path. |
| `VICTORY_HEAP_PASS_MARKED` and `VICTORY_LIST_REUSE` | Separate experiments remain open. The current renderer already scopes bindless texture refreshes and can run heap passes beside recording. Those paths do not establish parity with the old marked-slot pass or keep and root-view reuse. |
| `VICTORY_FILLS_PLAIN` and `VICTORY_COPY_PLAIN` | The earlier guarded direct-copy implementation needs a port to the merged mapped-copy path. Current fills can run in parallel. |
| `VICTORY_CARD_REHASH_GAP` | Touching ranges already coalesce. A wider gap changes which pieces get hashed, so it needs an ordering and read-back check before porting. |
| `VICTORY_TARGET_VARIANTS` and the alias refresh change | The fork's bound-target pipeline variants and shader-written alias change need separate rendering regression tests. |
| `VICTORY_EXACT_WAVES` and `VICTORY_EXACT_FIRSTLANE` | Divergent cross-half exchange can hang when only one half reaches a group barrier. A safe port needs a way to verify both halves reach the exchange. |
| `VICTORY_TEXTURE_DROP_MIPS` | Forced mip dropping needs a port and runtime tests for host subresource indices across uploads, copies, read-back, and tiled textures. The restored dimension cap gives us a separate low-VRAM experiment. |

For Windows A/B comparisons set `VICTORY_OVERLAY=0` on each build. On the
integrated build, compare the stock pareto profile against the same profile
with `VICTORY_TEXTURE_MAX_DIM=1024` separately from RAM trimming. Record
peak dedicated VRAM, streaming stalls, and any visible change to textures.

## Windows comparison

Use the same title build, checkpoint, driver, and capture method for each
run. Warm the pipeline cache for each build separately. Start with Series X
and 1080p on both builds at equivalent texture settings (forced mip
dropping disabled on the baseline). Then compare the old tuned baseline
(two mips dropped) against the new Series X and pareto Series S profiles
as separate runs. Record median and 95th-percentile frame time, peak VRAM
and private commit, visible texture and lighting differences, and time
spent streaming. Repeat the first boat mission and the Series S transition
from movies to gameplay to check the read-back and two-plane fixes.

## Memory census and low-memory switches

These cover tasks 1, 3–7 and 13 of the memory plan (PC 1). Most are off by
default. The one stock-run change is the byte cap on idle single-view copies:
it now keeps at most 64 MiB by default, where before only the 256-copy count
was capped. Raise `VICTORY_VIEW_COPIES_MB` to keep more, or set it to 0 to
keep none. No stock-versus-`-LowMemory` frame-time comparison is included
here; capture one before relying on this being neutral for your title.

| Setting | Effect |
| --- | --- |
| `VICTORY_CENSUS_FRAMES=120` (`-CensusFrames 120`) | Logs one `memory census` line every N frames, at warn level so it shows with `VICTORY_LOG=warn`. Each key is `now/peak` in MiB: private commit and working set, host copies of title memory, copies' and windows' card sides, CPU-mapped windows, the snapshot pool, texture staging, pool blocks, host textures (split into title-filled and targets/written), list scratch and its spare part, list upload buffers not yet retired, single-view copies, the raw-address directory (and what only it keeps alive), and DXGI local/non-local use and budget beside the local bytes this crate knows of. Each allocation is counted once, by its owner. Host lists' command allocators (about 9 GB in `docs/findings.md`) are driver memory with no byte counter, so the line gives `host_lists` (a count) and they show in the private commit. |
| `VICTORY_STAGING_POOL_MB` | Most empty texture staging blocks kept, in 64 MiB blocks (default 8 blocks, 512 MiB). |
| `VICTORY_TEXTURE_POOL_SPARE_MB` | Most empty texture heap blocks kept, in 64 MiB blocks (default up to 4 GiB while the card has room). |
| `VICTORY_SCRATCH_SPARE_MB` | Most bytes of spare list scratch kept (default 128 MiB, 1 GiB with plenty of RAM). |
| `VICTORY_SNAPSHOT_POOL_MB` | Bytes of snapshot buffers kept beyond the largest one (default 512). |
| `VICTORY_VIEW_COPIES_MB` | Bytes of idle single-view copies kept (default 64). Before, only their number (256) was limited. Held copies are never evicted. |
| `VICTORY_RAM_TRIM_MB` (`-RamTrimMB`) | Now has hysteresis: trimming starts below the threshold and stops once free memory is back above it by an eighth (512 MiB at least). While trimming, pool blocks are kept as under memory pressure and idle single-view copies are released too. |
| `-LowMemory` | Sets `VICTORY_WINDOW_KB=512`, `VICTORY_WINDOW_TAILS=1`, staging pool 64, scratch spare 64, snapshot pool 128, texture pool spare 256 and view copies 32, unless already set. |

Tasks 3 and 4 needed no code: `VICTORY_CARD_WINDOWS_MB` defaults to 0 (no
card-side windows) and `VICTORY_CARD_BUFFERS_MB` already defaults to 512
on cards with up to 12 GiB, both below the plan's targets, and the window
grid and tails were already switches. Compare runs with and without
`-LowMemory`, using the census for bytes and the overlay for frame times.
