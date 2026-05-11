# Overlay Use Folder

This folder is managed automatically by the app — **do not drop images here directly**.

## Workflow

Drop source images into the parent `assets/overlays/` folder.

On startup, the app syncs origin → `use/` as follows:

- **Images in `assets/overlays/` but not in `use/`** — preprocessed and copied here.
- **Images in `use/` but not in `assets/overlays/`** — deleted from here.
- **Images present in both** — left unchanged (not re-processed).

The app then reads images for display exclusively from this `use/` folder.
Source originals in `assets/overlays/` are never modified.

## Preprocessing applied

Each image is processed into a copy that maximises ASCII conversion quality:

1. **Auto-contrast** — stretches the luminance histogram to the full 0–255 range.
2. **Contrast boost** — 1.5× enhancement to separate light and dark regions.
3. **Unsharp mask** — sharpens edges so silhouettes read clearly as distinct glyphs.

## Notes

- Supported formats: `.png`, `.jpg`, `.jpeg`, `.bmp`, `.gif`, `.tga`, `.tiff`, `.webp`.
- Processed images are saved as PNG regardless of the source format.
- This directory and its image contents are local-only (gitignored).
