# Overlay Subsystem

Related docs:

- Docs index: [`../README.md`](../README.md)
- Architecture: [`ARCHITECTURE.md`](ARCHITECTURE.md)
- Module structure: [`MODULE_STRUCTURE.md`](MODULE_STRUCTURE.md)
- User guide: [`../../assets/overlays/README.md`](../../assets/overlays/README.md)
- Research: [`../research/OVERLAY_DENSITY_AND_FILTER_HANDOFF.md`](../research/OVERLAY_DENSITY_AND_FILTER_HANDOFF.md)

## What this is

The "overlay" is the periodic interruption where the rain pauses for a
few seconds and a recognizable shape — Neo's face, a logo, glyphs
arranged into a phrase — emerges from the falling characters. Image in,
ASCII portrait out, then back to rain. This document is the source-of-
truth on how the subsystem is wired: what loads images, how those
images become "this glyph at this row in this column for this many
frames," and how the state mutates the rain so the rendering layer
just sees it as more glyphs to draw.

All file references are crate-relative under
`rust/crates/matrisaver-core/src/`.

## Source layout

| Concern | File | Role |
|---|---|---|
| Trigger / state machine / lock management | `runtime/overlay/state.rs` | When to start, when to stop, what to clean up |
| Image and tuning I/O | `runtime/overlay/io.rs` | Directory resolution, tuning JSON, image enumeration, glyph-atlas lookup |
| Per-cell sampling and glyph mapping | `runtime/overlay/image.rs` | 2×2 super-sample, opt-in auto-levels remap, density-ramp glyph lookup. V2 dropped the 7-stage filter pipeline. |
| Image-to-grid orchestration | `runtime/overlay/inject.rs` | Per-injection pipeline: load image → fit to grid → sample → glyph-map → build header & intro target sets → optional ASCII-alongside snapshot |
| Per-frame instance emission | `runtime/overlay/emit.rs` | Render-phase output: turns header / intro state into `GlyphInstance`s |
| Runtime fields and lifecycle hook | `lib.rs` (CoreRuntime) | `overlay_*` fields, `set_overlay_reference_rect` / `clear_overlay_reference_rect`, intro-mode default, `overlay_dir_writable` probe cache |
| Caller (per-frame) | `runtime/lifecycle/frame.rs` | Calls `update_overlay_state` early; calls the two `emit_overlay_*_instances` at the end of `build_stream_instances` |
| Frozen-cell behavior in rain lifecycle | `runtime/lifecycle/cells.rs` | `cell.frozen` flag is honored by `write_head_row`, `erase_row`, and `update_volatile_cells` so locked overlay cells survive the rain advancing past them |
| Constants and types | `runtime/types.rs` | `OVERLAY_*` timing/format constants, `OverlayHeader`, `OverlayIntroGlyph`, `OverlayTargetCell`, `OverlayIntroMode`, `OverlayTuning`, `OverlayTuningConfig` |
| User-facing settings | `lib.rs` (`config::Settings`) | `overlay_enabled`, `overlay_directories: Vec<OverlaySource>`, `overlay_auto_levels` |

These files are `include!`'d into `lib.rs` rather than declared as sub-
modules, so all impls extend `CoreRuntime` in the lib's root namespace.
That's a workspace-wide quirk worth knowing before you go hunting for
`pub mod overlay;` somewhere — there isn't one.

## How images are ingested

Three axes of input get resolved separately and combined inside
`inject_overlay_from_image` (`runtime/overlay/inject.rs`).

### 1. Where to look — directory resolution

V2 resolution is driven by `Settings.overlay_directories`, an ordered
list of `OverlaySource { path, enabled, write_ascii_alongside }`.
`overlay_image_paths()` walks enabled sources in declaration order,
collecting any file whose extension matches `OVERLAY_IMAGE_EXTENSIONS`
(`png, jpg, jpeg, bmp, gif, tga, tiff, webp`). Earlier sources win
on filename collisions (dedupe via `HashSet<OsString>`).

When `Settings.overlay_directories` is empty (e.g. fresh install
before the user adds anything via the dialog), resolution falls back
to the legacy chain in `resolve_overlay_directory()`:

1. `MATRISAVER_OVERLAY_DIR` env var, if it points at a real directory.
2. Walking ancestors of `std::env::current_exe()` looking for
   `assets/overlays/`.
3. Walking ancestors of `std::env::current_dir()` looking for
   `assets/overlays/`.
4. `option_env!("CARGO_MANIFEST_DIR")` — compile-time dev fallback.

If none resolve, overlay enumeration returns empty and the trigger
short-circuits.

### 2. Which images to play

`overlay_image_paths()` returns `Vec<(PathBuf, bool)>` where the
second element is `write_ascii_alongside` for the source the file
came from. The sorted-per-source list is walked round-robin via
`overlay_image_cursor` so successive overlays cycle through every
image before repeating.

### 3. How to tune the conversion

`load_overlay_tuning()` resolves a JSON tuning file in this order:

1. `MATRISAVER_OVERLAY_TUNING_PATH` env var (must point at a real file).
2. `<overlay_dir>/overlay_tuning.json`.
3. `<overlay_dir>/overlay_config.json` (compatibility alias).

If found, it parses into `OverlayTuningConfig` (all fields `Option<T>`)
and applies via `OverlayTuning::with_overrides`, layering over
`OverlayTuning::default()`. Missing or malformed file silently falls
through to defaults — overlay never errors out for tuning issues.

### 4. Decode and per-cell sampling

`image::open(path).to_rgba8()` produces an RGBA bitmap. The bitmap is
fit into the rain grid using letterbox math:

- The rain grid has `cols × rows` cells. `inject_overlay_from_image`
  computes those from the *overlay reference rect* (see *Reference
  rect*) divided by `column_pitch` (= `char_size × COLUMN_PITCH_SCALE`,
  with `COLUMN_PITCH_SCALE = 0.5` — physical columns are half the
  glyph cell width because the rain renders at column pitch < cell
  width).
- The image is fit into `(fit_cols, fit_rows)` preserving aspect; the
  smaller axis gets padding via `col_offset` / `row_offset`.
- For each `(cell_col, cell_row)`, `sample_overlay_cell` does 2×2
  super-sampling (four pixel reads at quarter-cell offsets), averages
  alpha and weighted luminance (default `(0.18, 0.74, 0.08)` —
  perceptual Rec. 709 luma, with green dominant).

Two passes are run with different `(fit_cols, fit_rows)`:

- **Header pass** at the actual grid resolution. Output drives the
  falling header glyphs that "draw" the overlay onto the rain.
- **Dense intro pass** at `fit_cols × intro_density_multiplier_x`.
  Output drives the smaller, sub-column "intro" glyphs that flicker
  between rain columns to thicken the silhouette before the headers
  arrive. Default multiplier is 2.0.

### 5. Shaping (V2 — passthrough by default)

Pre-v0.2.0 there was a seven-stage pipeline (denoise → CLAHE →
unsharp → auto-levels → gamma → contrast → glyph). Research synthesis
(jp2a, libcaca, Paul Bourke) showed every canonical ASCII-conversion
tool defaults to passthrough; the heavy pipeline was matrisaver's
outlier behavior, flattening silhouettes via denoise and amplifying
grid noise via unsharp.

V2 keeps **only the bits that are demonstrably part of "the ASCII
engine itself"**, not filtering imposed on top:

1. **Alpha cutoff.** Pixels with `alpha < tuning.alpha_cutoff` are
   silhouette boundary — they render as spaces in the snapshot and
   contribute nothing to header/intro glyph emission. Not a filter,
   a topological op.
2. **Auto-levels (opt-in).** When `Settings.overlay_auto_levels` is
   true: `auto_levels` sorts alpha-masked luminance and pulls
   `(low, high)` at `levels_low_percentile` / `levels_high_percentile`
   (defaults `(0.05, 0.95)`); `remap_level` linearly stretches each
   value from `[low, high]` to `[0, 1]`. Otherwise raw luminance
   goes straight to glyph mapping. Defensible for low-contrast /
   clustered-histogram inputs; harmful for already-high-contrast
   silhouettes. Off by default.
3. **Glyph mapping.** `overlay_glyph_index_for_luminance` maps the
   shaped value through `OVERLAY_DENSITY_GLYPHS = ".:-=+*<>¦｜/\\"` —
   13 glyphs from sparse to dense — and looks up the corresponding
   atlas index. Falls back to `*`, then `+`, then a proportional
   fallback if the desired glyph isn't in the live atlas.

The result for each grid cell is `(glyph_index, brightness)` where
`brightness = clamp(brightness_floor + shaped × alpha × brightness_scale, brightness_floor, 1.0)`.

The dialog's "Auto-level overlay luminance" toggle is the source of
truth; `overlay_tuning.json::auto_levels_enabled` is honored only
when the Settings field is also true (the dialog field overrides on
each `inject_overlay_from_image`).

### 6. ASCII-alongside snapshot (opt-in per source)

Each `OverlaySource` has a `write_ascii_alongside: bool`. When true
and the directory passes the idempotent writability probe
(`probe_overlay_dir_writable` — writes/removes a zero-byte file once
per session, caches the result on `CoreRuntime.overlay_dir_writable`),
`write_overlay_ascii_alongside` drops a plain-text rendering of the
glyph grid next to each source image:

```
neo.png   →   neo.png.ascii.txt
logo.jpg  →   logo.jpg.ascii.txt
```

The text rendering uses `render_overlay_grid_text`, which mirrors the
same alpha-cutoff and auto-levels gates as the live render. Cells
below the cutoff render as spaces so the silhouette boundary is
visible.

Failures (read-only directory, write error mid-stream, etc.) are
silent — the entry in `overlay_dir_writable` flips to `false` and
all subsequent injections from that directory skip the write. No
error surfacing per the V2 contract.

## How it latches onto the runtime

### The trigger state machine

`update_overlay_state` (in `runtime/overlay/state.rs`) runs once per
frame from `build_stream_instances` and owns the entire lifecycle:

```
                 settings.overlay_enabled = false
                            │
                            ▼
                     ┌─────────────┐
   ┌───────────────► │    IDLE     │ ◄────────────┐
   │                 └──────┬──────┘              │
   │ overlay_active_until    │ now ≥ overlay_next_trigger
   │ has elapsed             ▼
   │                 ┌──────────────────┐
   │                 │ inject_overlay_  │
   │                 │   from_image()   │
   │                 └──────┬───────────┘
   │                        │
   │                        ▼ (success: headers populated,
   │                  ┌─────────────────┐  rain cells locked)
   └─────────── ◄─── │  HEADERS ACTIVE │
                     │  (falling)      │
                     └─────┬───────────┘
                           │ all headers reached their last target
                           ▼
                     ┌──────────────────┐
                     │  HOLD            │  overlay_active_until = now + 8s
                     │  (intro glyphs   │  Headers stop emitting; locked
                     │   visible, rain  │  cells keep their glyph; rain
                     │   keeps running) │  flows around them
                     └─────┬────────────┘
                           │ now ≥ overlay_active_until
                           ▼
                     IDLE (locks cleared, intro glyphs cleared,
                           next trigger scheduled)
```

Time constants live in `runtime/types.rs`:

| Constant | Default | Meaning |
|---|---|---|
| `OVERLAY_INITIAL_TRIGGER_SECONDS` | 8.0 | Delay before first overlay after runtime start |
| `OVERLAY_HOLD_SECONDS` | 8.0 | How long the locked image stays visible after headers finish |
| `OVERLAY_TRIGGER_MIN_SECONDS` | 15.0 | Minimum gap between successive overlays |
| `OVERLAY_TRIGGER_RANGE_SECONDS` | 15.0 | Random jitter on top of the minimum (uniform via `hash01`) |

So gaps are 15-30s, with the very first overlay arriving 8s after
launch.

### Reference rect — where the overlay is allowed to live

By default the overlay covers the entire surface. But the Windows host
in spanning-virtual-screen mode (multiple monitors) calls
`set_overlay_reference_rect(x, y, width, height)` to pin the overlay
to the primary monitor's slice of the global window. Without this, an
overlay rendered on a 5760×1080 spanned surface would sample the
image at 5×1 aspect and stretch the silhouette across all three
monitors.

The setter clamps the rect into the surface; the getter is internal.
`clear_overlay_reference_rect()` reverts to whole-surface behavior.

### What "latching" means concretely

Inside `inject_overlay_from_image`:

1. Build a `slot_to_column` map (column slot → index in
   `self.rain_columns`). Slots are stable; column indices may not be
   if the rain pool shifts.
2. Walk every grid cell that survived alpha-cutoff. For each
   `(grid_col, grid_row)` translate to the column_slot via
   `physical_start = col_start + (logical_col / COLUMN_PITCH_SCALE)`,
   spanning `column_span = round(1 / COLUMN_PITCH_SCALE) = 2` columns
   per "logical" overlay column.
3. Pick the brightest glyph candidate per `(column_index, row_index)`
   pair via `entry().and_modify().or_insert()`.
4. Emit a `OverlayIntroGlyph` per `(column_slot, row_index, intro_index)`
   for the dense pass — these float at sub-column offsets between
   real rain columns.
5. Build one `OverlayHeader` per affected column, ordered by column
   index. Headers start above the screen at `top_y - char_size` and
   fall at `header_speed = max(speed_range.1 × 3, 1)`.

When a header reaches a row in its `targets` list, `state.rs`'s
`advance_overlay_headers` writes to that row's `RowCell`:

```rust
cell.glyph_index = Some(target.glyph_index);
cell.brightness = target.brightness;
cell.frozen = true;             // ← THE LATCH
column.push((column_index, target.row_index)) into overlay_locked_cells;
```

The `frozen` flag is what couples the overlay to the rain lifecycle.
`runtime/lifecycle/cells.rs::write_head_row` and `erase_row` both
short-circuit on `cell.frozen`, and `update_volatile_cells` skips
volatility processing for frozen cells. The rain head can flow past
a frozen cell, but it cannot overwrite or erase it. That's how the
silhouette persists even though the rain animation continues
underneath.

When the hold expires, `clear_overlay_locks` walks
`overlay_locked_cells` and sets each `cell.frozen = false`,
returning the cells to the rain's normal lifecycle. The next rain
head that hits them overwrites the glyph; from the user's
perspective the silhouette dissolves as the rain reclaims it.

## How it couples with the rendering

There's no separate overlay render pass. Overlays produce
`renderer::GlyphInstance`s into the same vec the rain produces into,
and the GPU pipeline draws them all in a single instanced call.

`runtime/lifecycle/frame.rs::build_stream_instances` ends with:

```rust
self.emit_overlay_intro_instances(&mut instances, glyphs, height as f32, char_size);
self.emit_overlay_header_instances(&mut instances, glyphs, height as f32, char_size);
instances
```

Two emission functions, each in `runtime/overlay/emit.rs`:

- **`emit_overlay_intro_instances`** runs every frame the headers are
  *not* in HOLD state (i.e. while headers are still falling, after
  state.rs has populated `overlay_intro_glyphs`). It emits one
  `GlyphInstance` per intro glyph, sized at
  `char_size × intro_glyph_scale` (default 0.5 — half-cell), positioned
  at `column_slot × column_pitch + x_offset` so they sit between real
  rain columns. `params[1] = 0.45` — the head-boost channel — is
  intentionally lower than the rain's head boost so intros read as
  "ghost" glyphs flickering in the gaps.
- **`emit_overlay_header_instances`** runs every frame the headers are
  *not* in HOLD (`overlay_active_until` is None during the falling
  phase). It emits one instance per header, sized at full `char_size`,
  with `brightness × header_brightness_scale` (default 2.0 — headers
  read as bright leading glyphs, like rain heads). Once a header
  reaches its last target it stops emitting; the underlying rain
  cell is now `frozen` so the rain itself takes over rendering.

That distinction is critical: during HOLD, **neither** emission
function runs, but the silhouette is still on screen — because every
frozen cell is part of the normal rain instance emission in
`emit_original_instances`. The overlay is rendered "for free" by
the rain renderer once the headers have written their glyphs into
the cells. There is no separate state to manage during HOLD.

`overlay_intro_glyphs` is also pruned as headers retire each row
(`retired_intro_cells`), so the dense flicker fades from any column
slot whose frozen target has landed.

## The state owned by `CoreRuntime`

In `lib.rs` around line 612-622:

```rust
overlay_active_until: Option<f32>,            // None = idle/falling, Some = HOLD until t
overlay_next_trigger: f32,                    // animation_seconds at which next inject can fire
overlay_locked_cells: Vec<(usize, usize)>,    // (column_index, row_index) frozen by the current overlay
overlay_image_cursor: usize,                  // round-robin index into overlay_image_paths()
overlay_injected_count: u32,                  // diagnostic; reflects # of locked cells written so far
overlay_image_name: String,                   // sanitized filename of current overlay (for trace lines)
overlay_reference_rect: Option<(u32,u32,u32,u32)>,  // primary-monitor sub-rect, or None for full surface
overlay_headers: Vec<OverlayHeader>,          // active falling headers (one per affected column)
overlay_intro_glyphs: Vec<OverlayIntroGlyph>, // sub-column flickering intro glyphs
overlay_intro_mode: OverlayIntroMode,         // AllAtOnce | WaveLeftToRight; affects header start_y
overlay_tuning: OverlayTuning,                // current effective tuning (loaded from JSON or defaults)
```

`overlay_intro_mode` controls the header animation feel:

- **AllAtOnce** (default): every header starts at the same y above the
  screen and falls in lockstep. Reads as "the image arrives".
- **WaveLeftToRight**: header start_y is offset by
  `order × char_size × 0.75` so columns enter sequentially. Reads as
  "the image is being typed across".

Currently no public setter exists (`#[allow(dead_code)]` is on the
enum). Switching modes requires a code edit; intentional — overlay
intro mode isn't in the user-facing settings yet.

## Tuning JSON reference

Full schema in `OverlayTuningConfig`. See the user-facing version of
this in [`assets/overlays/README.md`](../../assets/overlays/README.md);
this section gives the engineering view.

| Field | Default | Range | Effect |
|---|---|---|---|
| `alpha_cutoff` | 0.03 | 0–1 | Min alpha for a pixel to participate at all |
| `luma_weights` | `[0.18, 0.74, 0.08]` | per-channel | RGB → luminance weighting (Rec. 709 luma) |
| `auto_levels_enabled` | false | bool | Opt-in percentile-based contrast stretch. Settings dialog field `overlay_auto_levels` overrides this on each inject. |
| `levels_low_percentile` | 0.05 | 0–1 | Lower auto-levels percentile (only used when auto-levels is on) |
| `levels_high_percentile` | 0.95 | 0–1 | Upper auto-levels percentile |
| `brightness_floor` | 0.10 | 0–1 | Minimum brightness for a contributing cell (keeps silhouette visible against bright rain) |
| `brightness_scale` | 0.95 | 0+ | Multiplier on `(shaped × alpha)` before clamping |
| `header_brightness_scale` | 2.0 | 0+ | Multiplier on per-header brightness (heads are bright) |
| `intro_density_multiplier_x` | 2.0 | round to ≥1 | Sub-column intro density (2 = one extra glyph per column) |
| `intro_glyph_scale` | 0.5 | 0+ | Intro glyph size relative to char_size |
| `intro_layer_brightness_scale` | 1.0 | 0+ | Intro brightness multiplier |

V2 dropped these fields entirely (`OverlayDenoiseMode` enum +
`denoise_mode`, `denoise_strength`, `clahe_enabled`, `clahe_clip_limit`,
`clahe_tile_grid`, `unsharp_enabled`, `unsharp_amount`, `gamma`,
`contrast`). Legacy JSON files carrying them parse cleanly — serde
ignores unknown fields by default — but the values are discarded.

## Open coupling concerns

These are real seams worth knowing about, not bugs.

### 1. Tuning resolution layer is untested

`OverlayTuning::with_overrides` and `OverlayTuningConfig` parsing have
unit tests in `lib.rs`. The file-resolution layer in
`resolve_overlay_tuning_path` — env var beats `overlay_tuning.json`
beats `overlay_config.json` — is not exercised by any test. A
regression that swaps the precedence wouldn't be caught.

### 2. Round-robin cursor wraps but isn't randomized

`overlay_image_cursor` walks paths in sorted order and wraps. With a
small image set this produces a perfectly predictable cycle. Switching
to a per-injection random pick (or a Knuth-shuffled deck) is a
trivial change but would feel less mechanical.

### 3. No abort-mid-headers path

If the user disables overlays via settings while headers are still
falling, `clear_overlay_locks` runs immediately but in-flight headers
keep emitting until the next `update_overlay_state` tick. Cosmetic
only — they unlock and unfreeze instantly, but the visible header
glyph still draws for one frame.

### 4. ASCII-alongside writer doesn't snapshot intro glyphs

`render_overlay_grid_text` walks the header pass only (`fit_cols ×
fit_rows`), not the dense intro pass. The `.ascii.txt` snapshot
represents what the header layer draws, not the full visual including
the sub-column ghost glyphs. Fine for "what will this image look
like" preview but not bit-exact to the live render.

### 5. Tuning JSON path resolution still walks "the overlay directory"

`resolve_overlay_tuning_path` looks for `overlay_tuning.json` /
`overlay_config.json` in the legacy single-directory location (via
`resolve_overlay_directory()`). It doesn't search each entry of
`Settings.overlay_directories`. If a user has multiple sources, only
the first one's tuning JSON gets picked up — and only if it matches
the legacy resolution. Tractable, but the dialog's auto-levels toggle
sidesteps this for the most-needed setting.

## Public API surface (host-facing)

Only these exit `matrisaver-core`:

- `CoreRuntime::set_overlay_reference_rect(x: u32, y: u32, w: u32, h: u32)`
  — host calls this when running a multi-monitor spanning surface to
  pin the overlay to the primary monitor's pixel rect.
- `CoreRuntime::clear_overlay_reference_rect()` — opt back out to
  full-surface overlays.
- `config::Settings.overlay_enabled` — kill switch;
  `update_overlay_state` short-circuits when false and clears
  everything on the first tick where it sees false.
- `config::Settings.overlay_directories: Vec<OverlaySource>` — V2
  directory list (path + enabled + write_ascii_alongside per entry).
- `config::Settings.overlay_auto_levels` — V2 top-level toggle for
  the percentile contrast stretch. Wins over `overlay_tuning.json`'s
  `auto_levels_enabled` field.

Everything else (state, tuning, lock list, headers, intro glyphs,
write probe cache) is private. The host treats overlays as an
internal feature of the rain renderer.

## Quick reference — the call graph (V2)

```
build_stream_instances (runtime/lifecycle/frame.rs)
  ├── update_overlay_state (runtime/overlay/state.rs)
  │     ├── clear_overlay_locks                         [if disabled or hold expired]
  │     ├── advance_overlay_headers                     [if headers active]
  │     │     └── (writes RowCell.frozen, populates overlay_locked_cells)
  │     └── inject_overlay_from_image (runtime/overlay/inject.rs)  [if trigger fired]
  │           ├── load_overlay_tuning (runtime/overlay/io.rs)
  │           │     └── resolve_overlay_tuning_path
  │           │     └── (Settings.overlay_auto_levels overrides tuning.auto_levels_enabled)
  │           ├── overlay_image_paths (runtime/overlay/io.rs)
  │           │     └── walks Settings.overlay_directories (or legacy fallback)
  │           ├── image::open / to_rgba8
  │           ├── sample_overlay_cell (runtime/overlay/image.rs)         [×2 passes]
  │           ├── (optional) auto_levels / remap_level                   [if auto_levels_enabled]
  │           ├── overlay_glyph_index_for_luminance (runtime/overlay/image.rs)
  │           ├── (builds overlay_headers + overlay_intro_glyphs)
  │           └── (optional) write_overlay_ascii_alongside               [if source opted in]
  │                 ├── probe_overlay_dir_writable                       [cached per session]
  │                 ├── render_overlay_grid_text
  │                 └── std::fs::write(<image>.<ext>.ascii.txt)
  ├── (per-column rain lifecycle — runtime/lifecycle/{column,cells,reset}.rs)
  │     └── frozen cells short-circuit write_head_row / erase_row
  └── emit instances:
        ├── (rain instances from emit_original_instances — frozen cells render here)
        ├── emit_overlay_intro_instances (runtime/overlay/emit.rs)
        └── emit_overlay_header_instances (runtime/overlay/emit.rs)
```
