# Overlay Assets

This directory stores optional source images for the ASCII overlay effect used by the
Rust runtime (and the legacy Python/Pygame prototype).

## How the ASCII overlay behaves

- When overlay mode is enabled (`--enable-overlay`), the app loads images from this folder.
- Images are converted into a grid of glyphs using an internal density gradient.
- Darker pixels map to lighter/sparser glyphs (left side of the gradient), and brighter pixels map
  to denser glyphs (right side).
- The matrix rain animation then reveals and fades that glyph image in timed phases (`intro`,
  hold, `outro`) while regular rain continues around and through it.

## Asset guidance

- Prefer high-contrast images for clearer ASCII silhouettes.
- Keep source files local; this repository ignores all files in `assets/overlays/` except this
  README and the `use/` subfolder README.
- Supported image types include `.png`, `.jpg`, `.jpeg`, `.bmp`, `.gif`, `.tga`, `.tiff`, `.webp`.

## The `use/` subfolder

`assets/overlays/use/` is a managed cache — **do not put images there directly**.
Drop source images into this directory instead.

On startup the app syncs origin → `use/`:
- New images in this folder are preprocessed (auto-contrast, 1.5× contrast boost, unsharp
  mask) and copied into `use/`.
- Images removed from this folder are deleted from `use/`.
- The app displays images exclusively from `use/`; source originals are never modified.

## Overlay Filter Tuning Config

You can optionally tune overlay filter parameters with a local JSON file.

Resolution order:

- `MATRISAVER_OVERLAY_TUNING_PATH` (explicit file path), otherwise
- `assets/overlays/overlay_tuning.json`, otherwise
- `assets/overlays/overlay_config.json`.

Example:

```json
{
  "alpha_cutoff": 0.03,
  "luma_weights": [0.18, 0.74, 0.08],
  "gamma": 1.0,
  "contrast": 1.0,
  "levels_low_percentile": 0.05,
  "levels_high_percentile": 0.95,
  "brightness_floor": 0.10,
  "brightness_scale": 0.95,
  "header_brightness_scale": 2.0,
  "intro_density_multiplier_x": 2.0,
  "intro_glyph_scale": 0.5,
  "intro_layer_brightness_scale": 1.0,
  "denoise_mode": "none",
  "denoise_strength": 0.25,
  "clahe_enabled": false,
  "clahe_clip_limit": 2.0,
  "clahe_tile_grid": [8, 8],
  "unsharp_enabled": false,
  "unsharp_amount": 0.35
}
```

Defaults are intentionally neutral for contrast shaping (`gamma = 1.0`, `contrast = 1.0`).
The preprocessing stages are opt-in by default (`denoise_mode = "none"`, `clahe_enabled = false`,
`unsharp_enabled = false`).
