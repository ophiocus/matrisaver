# Changelog

All notable changes to MatriSaver are documented here.

The format is loosely based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
For commit-level history use `git log`.

## [Unreleased]

(no entries yet)

## [0.3.7] — 2026-07-01

The **Reloaded 3D compositor** — first cut of a per-variant rendering
engine that translates specific Matrix Reloaded scenes into geometric
primitives. The variant retune from v0.3.6 laid the cascade values;
this release adds the pipeline that wraps that cascade onto 3D forms.

### Added

- **`Pipeline::CodeStrips3D`** — new render pipeline. The 2D code
  cascade renders offscreen exactly as before (full lifecycle, glyph
  swap, bloom, overlays, all of it) but the present pass wraps that
  scene texture onto animated 3D geometry instead of blitting flat.
  Selected automatically when the active variant demands it — a
  variant's `pipeline` field now overrides `settings.pipeline` for
  variants that require a specialised compositor.
- **Four-pose animation cycle** (24s loop) with smoothstep-eased
  ramps between poses:
  1. **FLAT** (4s hold, 2s ramp) — dense uniform matrix rain filling
     the frame. Imitates the 1999 opening titles.
  2. **CYLINDER** (4s hold, 2s ramp) — curved wall of code wrapping
     around an off-axis orbiting camera with 12° tilt. Matches
     Neo's dream-code cutscene (Trinity-falls premonition).
  3. **TUBE + CLOCK FACE** (4s hold, 2s ramp) — code-tunnel fly-
     through with a circular disc of code floating at the vanishing
     point. Directly reproduces the Reloaded inception moment where
     code assembles into a recognisable shape as the camera advances.
  4. **SHATTER** (4s hold, 2s ramp) — four flat panes of code at
     wild 3D angles with camera parallax drift. Matches the dream
     sequence's shattered-mirror-of-code fragments.
- **Reloaded variant wired to `CodeStrips3D`** — picking `reloaded`
  in settings routes to the new compositor automatically. Other
  variants remain on their existing pipelines.
- **Diagnostic `PRESENT_CHOICE` line** emitted to stderr on the
  first render call per presenter — surfaces variant name, resolved
  pipeline, `use_3d` flag, and surface size. Cheap, one-shot per
  presenter, useful for support.

### Changed

- **`reloaded` VariantConfig retuned** for the wet-luminous-chunks
  aesthetic seen in the actual Reloaded code cutscenes (Trinity-
  falls dream, Neo catches Trinity). Direct comparison against
  extracted film frames drove every value:
  - `color`: `(0, 255, 90)` → `(25, 245, 100)` — cooler, more saturated
  - `speed_range`: `(6, 14)` → `(3, 8)` — Reloaded's code churns, doesn't machinegun
  - `glow_color`: `(200, 255, 200)` → `(180, 255, 190)` — wetter white-green wash
  - `ghost_chance`: `0.15` → `0.28` — chunks morph more often
  - `ghost_swap_multiplier`: `10.0` → `18.0` — but hold longer between swaps
  - `trail_length_multiplier`: `1.5` → `3.5` — long luminous chunks
  - `volatile_chance`: `0.4` → `0.55` — more columns actively swapping
  - `gamma_range`: `(0.7, 1.3)` → `(0.6, 1.4)` — wider dynamic range
  - `bloom_range`: `(0.2, 0.9)` → `(0.35, 1.1)` — bloom heavily biased high
  - `head_bloom`: `2.2` → `3.2` — brighter heads
  - `vfx_glow_strength`: `1.2` → `1.9`
  - `vfx_glow_radius`: `1.8` → `2.6`
  - `vfx_glow_threshold`: `0.55` → `0.42`
- **Runtime pipeline resolution**: when a variant's `pipeline` is
  `CodeStrips3D`, that choice wins over `settings.pipeline` — a
  variant that demands a specialised compositor can't be broken
  by a stale user engine preference.

### Fixed

- **Fog attenuation** in the 3D pass now uses linearised view-space
  distance (`length(world_pos)`) instead of clip-space `z/w`. The
  old formulation compressed against the far plane and pegged
  everything to min-clamp; the new one gives a gentle radial
  attenuation from the near ring outward.
- **Far plane** moved from 8 → 20 world units so the tube's clock-
  face disc (at world Z=-3.2, camera-distance up to 9.2) no longer
  gets frustum-clipped at parts of the drift cycle.

### Dependencies

- Added `bytemuck = "1.21"` to `matrisaver-host-windows` for the
  vertex/uniform Pod derives in the new `present_3d` module.

---



## [0.3.6] — 2026-06-03

First cut of the shake-to-menu feature request
([`docs/features/shake-menu.md`](docs/features/shake-menu.md)).

### Added

- **Shake-to-menu**: cursor movement during full screensaver playback
  no longer instantly exits. A rolling 600 ms cursor-path buffer plus
  a shake detector (path ≥ 220 px AND ≥ 3 direction reversals) tells a
  deliberate shake from ambient jitter. On shake, the screensaver
  spawns `./exe /c 0` (the same entrypoint Personalization's
  "Settings…" uses) and destroys its own window, so the user lands in
  the settings dialog. Keyboard and mouse-click still exit
  immediately.
- **Hidden cursor during playback**: `ShowCursor(FALSE)` at window
  create, paired with `ShowCursor(TRUE)` in `WM_NCDESTROY`. Preview
  mode leaves the cursor alone.
- **Tabbed config dialog** (Options / About / Share) with a
  right-aligned Exit button that quits the whole process.
- **About pane**: name, version (`CARGO_PKG_VERSION`), first-release
  month (hardcoded `2026-05`; tags can be rewritten, a const can't),
  author, repo link, MIT licence, and a credits block (Rezmason
  glyphs, WB Matrix Code NFT CC-personal-use, Rogers-v-Grimaldi
  original silhouette art).
- **Share pane**: a QR code of the repo URL (via the `qrcode` crate,
  rasterised into an egui texture once and cached) plus the URL as
  monospace-selectable text.
- New dependency: `qrcode = "0.14"` — pure Rust, MIT/Apache, no image
  encoder pulled in.

### Deferred (tracked in `docs/features/shake-menu.md`)

- Menu drawn as a translucent overlay over the animating rain (needs
  an egui composition layer inside the existing wgpu render pass).
- Options-tab button spawning a fresh `/c 0` (currently no-op
  redundant with just being in the dialog).
- Escape / re-shake to return to screensaver — not applicable while
  the menu lives in a separate dialog process.

## [0.3.5] — 2026-06-03

Closes the user-provided-folder loop so dropped images integrate
without an external pre-bake step.

### Added

- **In-runtime silhouette synthesis** (`synthesise_silhouette` in
  `runtime/overlay/image.rs`). Port of `scripts/bane_mask.py`'s
  forward + shadow-recovery passes, applied per subsample inside
  `sample_overlay_cell` before the 4-supersample average. Gated by a
  new `OverlaySamplePlan.synthesize_silhouette` flag set to
  `!has_mask` in `inject.rs` — when a `<name>.mask.png` sibling
  exists it's still used directly (existing path, unchanged); when
  it doesn't, the runtime derives the high-contrast silhouette from
  the colour image's luma. Constants (`SIL_ALPHA_LOW=0.14`,
  `SIL_ALPHA_HIGH=0.34`, `SIL_RGB_BLACK=0.10`, `SIL_RGB_WHITE=0.85`,
  `SIL_INV_NEAR_BLACK=0.88`, `SIL_INV_ALPHA_WEIGHT=0.85`,
  `SIL_INV_GREY_WEIGHT=0.6`) lifted verbatim from `bane_mask.py` so
  the visual contract matches. The python pass's post-stage
  `GaussianBlur(radius=1.2)` is not ported — the 4-subsample cell
  average produces equivalent edge smoothing.

### Changed

- **User-folder workflow:** dropping a new image into a watched
  overlay folder now integrates on the next inject cycle without any
  external script invocation. `scripts/bane_mask.py` remains as the
  reference implementation and is still available for pre-baking
  masks (slightly cheaper at inject — saves ~10 math ops × 4
  subsamples per cell once), but it's no longer required.

## [0.3.4] — 2026-06-03

Overlay subsystem overhaul. Drove the lifecycle, scaling, and chromatic
policy to match the three-stage intent — *incoming rain → stay
→ vanish in rain* — and laid in the diagnostic infrastructure that
surfaced the regressions along the way.

### Added

- **Three-stage overlay lifecycle**, strictly enforced: `INJECT →
  INTRO` (fast painting heads, **3.0×** max rain speed) `→ STAY`
  (`overlay_persist_seconds` dwell) `→ OUTRO` (slow dissolve heads at
  **0.4×** rain speed, spawning at the silhouette top, ablating
  top-down). The intro and outro multipliers live as a tuned pair of
  named constants in `runtime/types.rs`.
- **Dissolve overlay into rain**: at the end of `STAY`,
  `dissolve_overlay_into_rain` yanks a fresh rain head to the topmost
  silhouette row of every affected column and installs an
  `outro_speed_override` slowdown that auto-clears once the head
  passes the silhouette's bottom edge. The figure ablates into rain
  top-down instead of thawing in place.
- **Cover-scaled overlays anchored at main-screen char (0, 0)**:
  cover replaces aspect-FIT (was letterboxed + bottom-pinned). The
  painted grid stays the full target (`ascii_cols × rows`), and the
  source image is cover-scaled to fill it.
- **Bbox-cover scaling**: cover uses the silhouette's tight bounding
  box (auto-detected by alpha for transparency-carrying PNGs, by
  luma for greyscale masks) instead of the full image. Every overlay
  paints at consistent screen-cover regardless of how much padding
  the source has around the figure.
- **Per-glyph overlay colour sampling**: chromatic film variants
  (`original` / `reloaded` / `revolutions` / `resurrections`) paint
  overlay glyphs in hues sampled from the source image. Bane keeps
  its fixed crimson tint.
- **Mask + colour pairing**: an overlay image can have a
  `<name>.mask.png` sibling that drives the silhouette shape (the
  high-contrast "bane-look ASCII"); the original drives chromatic info.
- **Render-to-PNG overlay sidecar at full bloom**: custom overlay
  folders that set `write_ascii_alongside: true` get a
  `<image>.overlay.png` rendered next to the source after each cycle.
- **Per-variant overlay queues**: every variant leads with its own
  iconic-scene subdir under `assets/overlays/<variant>/`; user folders
  are interleaved (variant entry first at each index, then folder;
  dedup by filename).
- **`bane` variant**: Revolutions Bane-defeat performance — dim
  emerald rain field with a crimson silhouette overlay drawn at full
  char-pitch.
- **Coverage-ranked glyph mapping**: overlay glyph selection uses the
  embedded font's measured ink coverage instead of a hardcoded ASCII
  punctuation ramp.
- **`overlay_natural_color` settings toggle**: switch chromatic policy
  between source-image hues and screen-green. Fixed-tint variants
  (bane) ignore it.
- **`MATRISAVER_OVERLAY_FAST=1`** env var for showcase pacing —
  overlays trigger every 1.5-3s instead of 15-30s.
- **`MATRISAVER_OVERLAY_LOG=<path>`** env var for diagnostic tracing —
  emits per-event lifecycle lines (inject load with src dims, bbox,
  visible_rect, grid, flags; `INTRO`/`STAY`/`OUTRO` transitions with
  timestamps + counts; `DISSOLVE` row span and slow-speed range) to
  the named file. Zero cost when unset.

### Changed

- **VFX defaults retuned for punchy bloom**: `vfx_head_hdr_scale`
  1.5 → **3.0**, `vfx_bloom_threshold` 0.7 → **0.5**,
  `vfx_bloom_intensity` 0.85 → **1.3**. The three values move as a
  coordinated set — restoring head HDR alone without the bloom retune
  reintroduces the v0.3.0 thresholded-plate artefact.
- **Overlay reset-to-defaults** button in the admin panel pulls
  straight from `Settings::default()` so the values can't drift out
  of sync with the canonical defaults again.
- **Overlay colour toggle label** clarified to
  *"Overlay colour from image (off = screen green)"* with
  chromatic-policy help text.

### Removed

- **Active-hold pre-show overlay phase**: the up-to-8-second dim
  full-silhouette preview that ran between trigger and intro. The
  lifecycle now strictly matches the three-stage intent.
  `OVERLAY_HOLD_SECONDS` / `OVERLAY_FAST_HOLD_SECONDS` constants,
  `overlay_hold_seconds()` helper, `overlay_active_until` field, and
  the emit / trace gates that consulted them are gone.
- **Aspect-FIT overlay scaling** (was: horizontal centering inset +
  vertical bottom-pin). Replaced by bbox-cover.
- **Mega-run sidecar re-ingestion**: `collect_overlay_dir` now skips
  `*.overlay.png` and `*.mask.png` so generated outputs don't get
  re-fed as inputs on the next cycle.

### Fixed

- Stale `H:\matrisaver` paths in repo docs and scripts → `I:\matrisaver`.
- `bane` overlay was reading at half-char pitch (dense mat) instead
  of rain scale.

---

Earlier history is in `git log`; tags `v0.2.1` through **v0.3.3**
predate this changelog. Notable v0.3.x milestones:

- **v0.3.3** — admin-panel sliders for HDR head boost, bloom
  threshold, bloom intensity, and overlay dwell time; overlay-tuning
  schema V2.
- **v0.3.2** — post-reveal hold for overlays (silhouette dwells
  after painting completes; pre-v0.3.2 it cleared the same frame).
- **v0.3.1 / v0.3.0** — initial GPU pipeline + bloom + overlays.
