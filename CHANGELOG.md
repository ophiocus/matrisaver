# Changelog

All notable changes to MatriSaver are documented here.

The format is loosely based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
For commit-level history use `git log`.

## [Unreleased]

(no entries yet)

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
