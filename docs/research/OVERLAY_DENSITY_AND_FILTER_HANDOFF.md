# Overlay Density + Filter Research Handoff

Related files:

- `rust/crates/matrisaver-core/src/runtime/overlay.rs`
- `rust/crates/matrisaver-core/src/runtime/lifecycle.rs`
- `assets/overlays/README.md`

## Scope

This handoff covers two next-step goals:

1. At overlay start, increase character density in the overlay area by rendering half-size overlay glyphs over the overlay columns.
2. Improve robustness of image preparation for random-source images before ASCII mapping.

## Current Runtime Behavior (Baseline)

- Overlay injection converts image cells to row-memory targets in `inject_overlay_from_image(...)`.
- Header sweep (`advance_overlay_headers(...)`) writes final frozen row cells as it descends.
- Header rendering (`emit_overlay_header_instances(...)`) uses normal character size.
- Current overlay prep already includes percentile levels + gamma + contrast + brightness scaling, and is now tunable via JSON.

## Goal 1: Density Boost During Overlay Intro

### Desired Behavior

- Before (and while) headers are descending, overlay area appears denser by using smaller glyphs in overlay columns.
- As headers overwrite cells, density-boosted intro glyphs are removed in those overwritten regions.
- Once intro completes, rendering is normal row-memory glyph size only.

### Recommended Implementation

Implement as a **temporary pre-header overlay layer** (do not change core row-memory grid topology).

Why:

- Current row-memory is one cell per `(column,row)` at global `char_size`; changing that grid for intro would be invasive.
- A transient render layer can be introduced and retired cleanly as headers advance.

### Data Model Additions

Add a transient collection in core runtime, for example:

- `overlay_intro_glyphs: Vec<OverlayIntroGlyph>`

Where `OverlayIntroGlyph` includes:

- `column_slot: u32`
- `row_index: usize`
- `x_offset: f32` (for sub-column placement)
- `glyph_index: u32`
- `brightness: f32`

### Rendering Strategy

- Generate 2 sub-column glyphs per overlay cell (horizontal density x2 over overlay columns).
- Render intro glyph instances at half size (`char_size * 0.5`) during the header phase.
- Keep this in `build_stream_instances(...)` as an additional emission pass before/after header instances.

### Retirement Strategy

- In `advance_overlay_headers(...)`, when a target `(column,row)` is committed to row-memory, remove matching intro glyphs for that region.
- When all headers complete, clear `overlay_intro_glyphs`.

### Config Knobs (Add to overlay tuning file)

- `intro_density_multiplier_x` (default `2.0`)
- `intro_glyph_scale` (default `0.5`)
- `intro_layer_brightness_scale` (default `1.0`)

## Goal 2: Robust Image Prep for Random Inputs

### Research Summary

For mixed-origin images (photos, screenshots, graphics, AI art), the most stable ASCII prep pipeline is:

1. Edge-preserving denoise (light bilateral or median)
2. Local contrast normalization (CLAHE)
3. Controlled global shaping (gamma/contrast)
4. Optional edge emphasis (unsharp or edge blend)

Notes from OpenCV guidance:

- CLAHE is preferred over global equalization when local regions vary strongly in brightness.
- Bilateral preserves edges while reducing texture noise, at a performance cost.
- Median blur is cheap and good for impulse noise.

### Recommended Pipeline

Apply in this order before glyph mapping:

1. Grayscale/luminance extraction (already present)
2. Optional denoise:
   - `median` for speed (`ksize=3`), or
   - `bilateral` for quality (`d=5..9`, conservative sigma)
3. CLAHE on luminance (small tiles, conservative clip limit)
4. Existing percentile remap (`levels_low/high_percentile`)
5. Existing gamma + contrast shaping
6. Optional mild unsharp (small radius/amount)

### New Config Fields (Proposed)

- `denoise_mode`: `"none" | "median" | "bilateral"`
- `denoise_strength`: number (normalized)
- `clahe_enabled`: bool
- `clahe_clip_limit`: number
- `clahe_tile_grid`: `[u32, u32]`
- `unsharp_enabled`: bool
- `unsharp_amount`: number

## Important Default Reset

Keep defaults neutral for contrast shaping unless explicitly configured:

- `gamma = 1.0`
- `contrast = 1.0`

This is already aligned with current code defaults.

## Execution Plan (Next Engineer)

1. Add transient intro-layer structs/state to core runtime.
2. Build intro glyphs in overlay injection pass.
3. Emit intro glyph instances in `build_stream_instances(...)` with half-size scale.
4. Retire intro glyphs progressively from `advance_overlay_headers(...)`.
5. Add config keys and parsing for intro-layer knobs.
6. Add optional denoise/CLAHE/unsharp hooks (feature-flagged by config).
7. Add lifecycle trace counters for intro-layer size/count to verify progression.

## Acceptance Criteria

1. Overlay intro visibly shows denser half-size glyphs only in overlay columns.
2. As headers pass, intro glyphs disappear in overwritten regions.
3. After intro completion, no half-size intro glyphs remain.
4. With defaults, output matches current neutral contrast behavior.
5. With config toggles off, no measurable behavior change from current pipeline.
