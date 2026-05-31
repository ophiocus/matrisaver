// Overlay image sampling, luminance preprocessing, and glyph index mapping.
impl CoreRuntime {
    /// Returns `(alpha, luminance, [r, g, b])` for one grid cell, 4x
    /// supersampled. The RGB is the cell's average colour (0..1), used by
    /// sample-colour variants so the painted glyphs carry the source
    /// scene's hues; alpha/luminance drive the silhouette and density.
    ///
    /// `plan.visible_rect = (x0, y0, x1, y1)` defines the window of the
    /// source image (in source-pixel coords) that maps onto the grid.
    /// The grid stretches that window across `(grid.cols × grid.rows)`
    /// cells — each cell samples
    /// `(x0 + (col+0.5)/cols * (x1-x0), y0 + ...)`. For COVER scaling
    /// (`fit_cols = ascii_cols`, `fit_rows = rows`),
    /// `inject_overlay_from_image` computes this window from the image
    /// aspect vs the grid aspect so the image's shorter dimension fills
    /// the grid and the excess on the longer axis is cropped
    /// symmetrically (`pad / 2` on each side). For "sample the whole
    /// image" callers, pass `(0.0, 0.0, image.width() as f32, image.height() as f32)`.
    fn sample_overlay_cell(
        image: &image::RgbaImage,
        plan: &OverlaySamplePlan,
        cell_col: u32,
        cell_row: u32,
    ) -> (f32, f32, [f32; 3]) {
        let width = image.width();
        let height = image.height();
        let (rx0, ry0, rx1, ry1) = plan.visible_rect;
        let rw = (rx1 - rx0).max(1.0);
        let rh = (ry1 - ry0).max(1.0);
        let offsets = [(-0.25f32, -0.25f32), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)];
        let mut alpha_sum = 0.0;
        let mut luma_sum = 0.0;
        let mut rgb_sum = [0.0f32; 3];
        let mut weight_sum = 0.0;
        let (lw_r, lw_g, lw_b) = plan.luma_weights;
        for (ox, oy) in offsets {
            let sx = rx0 + ((cell_col as f32 + 0.5 + ox) / plan.grid.cols as f32) * rw;
            let sy = ry0 + ((cell_row as f32 + 0.5 + oy) / plan.grid.rows as f32) * rh;
            let px = sx.floor().clamp(0.0, (width - 1) as f32) as u32;
            let py = sy.floor().clamp(0.0, (height - 1) as f32) as u32;
            let pixel = image.get_pixel(px, py);
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;
            let alpha = pixel[3] as f32 / 255.0;
            let luminance = (r * lw_r + g * lw_g + b * lw_b).clamp(0.0, 1.0);
            alpha_sum += alpha;
            luma_sum += luminance;
            rgb_sum[0] += r;
            rgb_sum[1] += g;
            rgb_sum[2] += b;
            weight_sum += 1.0;
        }
        if weight_sum <= 0.0 {
            return (0.0, 0.0, [0.0; 3]);
        }
        (
            alpha_sum / weight_sum,
            luma_sum / weight_sum,
            [
                rgb_sum[0] / weight_sum,
                rgb_sum[1] / weight_sum,
                rgb_sum[2] / weight_sum,
            ],
        )
    }

    /// Sample shape (alpha + luminance) from `shape` and colour (RGB)
    /// from `color`. With no mask (`has_mask = false`, `shape == color`)
    /// it's a single sample. With a mask, alpha/luminance come from the
    /// high-contrast mask (silhouette + glyph density) and the hue comes
    /// from the colour original — the "chromatic overlay, bane-look ASCII"
    /// split.
    fn sample_overlay_shape_color(
        shape: &image::RgbaImage,
        color: &image::RgbaImage,
        plan: &OverlaySamplePlan,
        cell_col: u32,
        cell_row: u32,
        has_mask: bool,
    ) -> (f32, f32, [f32; 3]) {
        let (alpha, luminance, rgb) = Self::sample_overlay_cell(shape, plan, cell_col, cell_row);
        if !has_mask {
            return (alpha, luminance, rgb);
        }
        let (_, _, color_rgb) = Self::sample_overlay_cell(color, plan, cell_col, cell_row);
        (alpha, luminance, color_rgb)
    }

    /// Optional contrast-normalization: percentile-based luminance
    /// remapping. Gated behind `tuning.auto_levels_enabled`; off by
    /// default. Defensible for low-contrast / clustered-histogram
    /// inputs; harmful for already-high-contrast portraits. Per the
    /// v0.2.0 research synthesis, this is the *only* preprocessing
    /// step canonical ASCII-conversion tools (jp2a, libcaca, Paul
    /// Bourke's reference) expose, and even they expose it as
    /// passthrough-by-default.
    fn auto_levels(values: &mut Vec<f32>, low_percentile: f32, high_percentile: f32) -> (f32, f32) {
        if values.is_empty() {
            return (0.0, 1.0);
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let low = Self::percentile(values.as_slice(), low_percentile.clamp(0.0, 1.0));
        let high = Self::percentile(values.as_slice(), high_percentile.clamp(0.0, 1.0));
        if (high - low).abs() < 1e-4 {
            (0.0, 1.0)
        } else {
            (low, high)
        }
    }

    fn percentile(sorted_values: &[f32], p: f32) -> f32 {
        if sorted_values.is_empty() {
            return 0.0;
        }
        let index = ((sorted_values.len() - 1) as f32 * p).round() as usize;
        sorted_values[index.min(sorted_values.len() - 1)]
    }

    fn remap_level(value: f32, low: f32, high: f32) -> f32 {
        if high <= low {
            return value.clamp(0.0, 1.0);
        }
        ((value - low) / (high - low)).clamp(0.0, 1.0)
    }

    /// Map a target tone (0 = dark, 1 = bright) to an atlas glyph index
    /// using the coverage-ranked ramp built from the embedded font
    /// ([`GlyphAtlas::coverage_ramp`]). Higher tone picks denser glyphs,
    /// so the silhouette's bright regions fill with heavy katakana and
    /// dim regions with sparse marks — a true tonal ramp rather than the
    /// old hardcoded ASCII punctuation string. A per-cell `variety` seed
    /// jitters the choice within a small tonal window so equal-tone cells
    /// don't tile the identical glyph, keeping the Matrix-code texture
    /// alive while staying tonally faithful.
    fn overlay_glyph_index_by_coverage(tone: f32, ramp: &[(f32, u32)], variety: u32) -> Option<u32> {
        match ramp.len() {
            0 => None,
            1 => Some(ramp[0].1),
            len => {
                let t = tone.clamp(0.0, 1.0);
                let base = t * (len - 1) as f32;
                // Window scales with ramp size; clamped so variety never
                // overwhelms tonal accuracy.
                let window = ((len as f32) * 0.08).clamp(1.0, 4.0);
                let jitter = (hash01(variety, 0x00C0_FFEE) * 2.0 - 1.0) * window;
                let idx = (base + jitter).round().clamp(0.0, (len - 1) as f32) as usize;
                Some(ramp[idx].1)
            }
        }
    }

    fn sanitize_trace_token(raw: &str) -> String {
        let mut sanitized = String::with_capacity(raw.len());
        for ch in raw.chars() {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                sanitized.push(ch);
            } else {
                sanitized.push('_');
            }
        }
        if sanitized.is_empty() {
            "unknown".to_owned()
        } else {
            sanitized
        }
    }
}
