#!/usr/bin/env python3
"""Derive the Bane overlay silhouette mask from a reference frame.

DEV-ONLY ASSET TOOL. The input is a Warner Bros. film frame; the output
is a local, gitignored dev mask used to iterate the `bane` variant's
look. Before tagging a release, the mask MUST be replaced with original
Bane-evoking silhouette art (Rogers v. Grimaldi) — see
memory/licensing_decisions.md.

The matrisaver overlay subsystem gates the painted silhouette on the
image's ALPHA channel (`alpha_cutoff`) and modulates internal glyph
brightness by LUMINANCE. The source screenshot is fully opaque, so we
synthesise alpha from luminance:

  * near-black background  -> transparent (no glyphs)
  * dark sunglasses/eyes   -> transparent (voids preserved)
  * bright amber figure    -> opaque, internal code-fire texture kept
                              in the grey RGB so glyph density varies.

Usage:
  python scripts/bane_mask.py <input.png> <output.png>
"""
import sys
from PIL import Image, ImageFilter


def smoothstep(edge0, edge1, x):
    if edge1 <= edge0:
        return 0.0 if x < edge0 else 1.0
    t = max(0.0, min(1.0, (x - edge0) / (edge1 - edge0)))
    return t * t * (3.0 - 2.0 * t)


# Alpha synthesis knobs. Tuned for the amber-on-black HOLD frame:
# below ALPHA_LOW luminance -> fully transparent (kills background haze
# and the dark sunglasses); above ALPHA_HIGH -> fully opaque (the lit
# face). The band between is the soft silhouette edge where code
# dissolves into the figure.
ALPHA_LOW = 0.14
ALPHA_HIGH = 0.34

# RGB contrast stretch so the internal code-fire detail spreads across
# the glyph-density gradient rather than clumping at the bright end.
RGB_BLACK = 0.10
RGB_WHITE = 0.85

# Shadow-recovery (the "invert and process the same way" pass). A second
# pass on the luminance negative surfaces areas that are a little too dark
# but still carry image information, so they pick up alpha + glyphs
# instead of vanishing. Guarded so genuinely-black background (which holds
# no detail) stays excluded.
#   INV_NEAR_BLACK : inverted-luma above this == original luma below
#                    (1 - this) == treated as pure black, recovered = 0.
#   INV_ALPHA_WEIGHT : how strongly recovered darks join the silhouette.
#   INV_GREY_WEIGHT  : glyph-density weight for recovered darks (kept < 1
#                      so they read as mid-tone detail, not as bright as
#                      the lit figure).
INV_NEAR_BLACK = 0.88
INV_ALPHA_WEIGHT = 0.85
INV_GREY_WEIGHT = 0.6


def stretch(value):
    t = (value - RGB_BLACK) / max(1e-4, (RGB_WHITE - RGB_BLACK))
    return max(0.0, min(1.0, t))


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(2)
    src_path, dst_path = sys.argv[1], sys.argv[2]

    img = Image.open(src_path).convert("RGB")
    w, h = img.size
    px = img.load()

    out = Image.new("RGBA", (w, h))
    op = out.load()

    for y in range(h):
        for x in range(w):
            r, g, b = px[x, y]
            # ITU-R BT.601 luma — matches the engine's default luma_weights.
            luma = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0

            # Normal pass: bright detail.
            alpha_n = smoothstep(ALPHA_LOW, ALPHA_HIGH, luma)
            shaped_n = stretch(luma)

            # Inverted ("invert hues") pass on the luminance negative,
            # processed the same way — recovers dark-but-informative areas.
            # `keep` rolls off toward genuinely-black pixels so the empty
            # background isn't dragged in.
            inv = 1.0 - luma
            keep = 1.0 - smoothstep(INV_NEAR_BLACK, 1.0, inv)
            alpha_i = smoothstep(ALPHA_LOW, ALPHA_HIGH, inv) * keep
            shaped_i = stretch(inv) * keep

            # Combine: union the silhouette, take the stronger glyph
            # density. Recovered darks ride in at reduced weight.
            alpha = max(alpha_n, alpha_i * INV_ALPHA_WEIGHT)
            shaped = max(shaped_n, shaped_i * INV_GREY_WEIGHT)
            grey = int(round(shaped * 255))
            op[x, y] = (grey, grey, grey, int(round(alpha * 255)))

    # Soften the alpha edge a touch so the silhouette boundary samples
    # cleanly into the cell grid (the engine averages 4 sub-samples per
    # cell, but a 1px blur removes single-pixel jaggies).
    rgb = out.convert("RGB")
    a = out.getchannel("A").filter(ImageFilter.GaussianBlur(radius=1.2))
    out = Image.merge("RGBA", (*rgb.split(), a))

    out.save(dst_path)
    # Report coverage so we can sanity-check the silhouette isn't empty
    # or fully filled.
    hist = a.histogram()
    opaque = sum(hist[200:])
    total = w * h
    print(f"wrote {dst_path} ({w}x{h}) — opaque-ish cells: {opaque}/{total} "
          f"({100.0 * opaque / total:.1f}%)")


if __name__ == "__main__":
    main()
