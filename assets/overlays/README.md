# Overlay Assets

Optional source images for the ASCII overlay effect — silhouettes that
emerge briefly from the rain when overlay mode is on. Drop high-contrast
portrait-style images here (or in any directory you add via Settings →
Overlays) and the engine ASCII-fies them at injection time.

## How the engine sees images (V2)

- 2×2 super-sampled luminance per overlay grid cell, weighted by
  Rec. 709 RGB → Y.
- Optional auto-levels remap (5th/95th percentile stretch), off by
  default. Toggle in Settings → Overlays.
- Direct map of shaped luminance to a density-ramp glyph
  (`.:-=+*<>¦｜/\\`), darker pixels → sparser glyphs.
- No denoise / CLAHE / unsharp / gamma / contrast preprocessing — V2
  defaults to passthrough per canonical practice (jp2a, libcaca,
  Paul Bourke). The 7-stage pipeline that lived here through v0.1.x
  is gone.

## Asset guidance

- High-contrast subjects work best. Portraits, logos, single
  silhouettes against a flat background — anything where the eye
  reads a clear figure-ground split survives downsampling to a
  20–40 cell grid.
- Photographic mid-tones flatten out — turn on auto-levels in the
  dialog if the result looks washed out.
- Supported image types: `.png`, `.jpg`, `.jpeg`, `.bmp`, `.gif`,
  `.tga`, `.tiff`, `.webp`.
- This `assets/overlays/` directory is gitignored (except this
  README) so local images don't pollute the repo.

## Multi-directory supply

Settings → Overlays accepts an ordered list of folders. Each row:

```
[ ✓ ] C:\Users\Carlos\Pictures\matrisaver-overlays   [Remove]
       [ ✓ Write ASCII snapshot beside each image ]
[ ✓ ] %PROGRAMDATA%\matrisaver\overlays              [Remove]
       [   Write ASCII snapshot beside each image   ]
```

Earlier entries win on filename collisions. Empty list falls back to
the legacy resolution chain:

1. `MATRISAVER_OVERLAY_DIR` environment variable.
2. `assets/overlays/` walking ancestors of the running binary.
3. Same walk against the current working directory.

## ASCII-alongside snapshot (opt-in per directory)

When the "Write ASCII snapshot" checkbox is on for a directory and
that directory is writable, the engine drops a plain-text rendering
of each overlay next to the source on each injection:

```
neo.png   →   neo.png.ascii.txt
logo.jpg  →   logo.jpg.ascii.txt
```

The text file mirrors the exact glyph grid the runtime drew (alpha
boundary as spaces, density ramp inside). Useful for previewing how
an image will look without launching the screensaver.

Writability is probed once per session per directory. Read-only
directories are silently skipped — no error surfacing, no retries.

## Power-user JSON tuning

Hand-edit `overlay_tuning.json` in any overlay directory (or set
`MATRISAVER_OVERLAY_TUNING_PATH`) to override individual fields:

```json
{
  "alpha_cutoff": 0.03,
  "luma_weights": [0.18, 0.74, 0.08],
  "auto_levels_enabled": false,
  "levels_low_percentile": 0.05,
  "levels_high_percentile": 0.95,
  "brightness_floor": 0.10,
  "brightness_scale": 0.95,
  "header_brightness_scale": 2.0,
  "intro_density_multiplier_x": 2.0,
  "intro_glyph_scale": 0.5,
  "intro_layer_brightness_scale": 1.0
}
```

All fields are optional. Settings dialog → "Auto-level overlay
luminance" wins over the JSON's `auto_levels_enabled` when they
disagree — the dialog is the source of truth.

Legacy JSON files with fields like `gamma`, `contrast`, `denoise_mode`,
`clahe_*`, or `unsharp_*` parse cleanly but their values are ignored —
those filters were removed in v0.2.0.
