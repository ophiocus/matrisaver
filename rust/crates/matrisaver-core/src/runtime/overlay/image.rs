// Overlay image sampling, luminance preprocessing, and glyph index mapping.
impl CoreRuntime {
    fn sample_overlay_cell(
        image: &image::RgbaImage,
        grid: CellGrid,
        cell_col: u32,
        cell_row: u32,
        luma_weights: (f32, f32, f32),
    ) -> (f32, f32) {
        let width = image.width();
        let height = image.height();
        let offsets = [(-0.25f32, -0.25f32), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)];
        let mut alpha_sum = 0.0;
        let mut luma_sum = 0.0;
        let mut weight_sum = 0.0;
        for (ox, oy) in offsets {
            let sx = ((cell_col as f32 + 0.5 + ox) / grid.cols as f32) * width as f32;
            let sy = ((cell_row as f32 + 0.5 + oy) / grid.rows as f32) * height as f32;
            let px = sx.floor().clamp(0.0, (width - 1) as f32) as u32;
            let py = sy.floor().clamp(0.0, (height - 1) as f32) as u32;
            let pixel = image.get_pixel(px, py);
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;
            let alpha = pixel[3] as f32 / 255.0;
            let luminance = (r * luma_weights.0 + g * luma_weights.1 + b * luma_weights.2)
                .clamp(0.0, 1.0);
            alpha_sum += alpha;
            luma_sum += luminance;
            weight_sum += 1.0;
        }
        if weight_sum <= 0.0 {
            return (0.0, 0.0);
        }
        (alpha_sum / weight_sum, luma_sum / weight_sum)
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

    fn overlay_glyph_index_for_luminance(luminance: f32, glyph_lookup: &[(char, u32)]) -> Option<u32> {
        if glyph_lookup.is_empty() {
            return None;
        }
        let gradient = OVERLAY_DENSITY_GLYPHS.as_bytes();
        if gradient.is_empty() {
            return Some(glyph_lookup[0].1);
        }
        let gradient_index = ((luminance.clamp(0.0, 1.0) * (gradient.len() - 1) as f32).round()
            as usize)
            .min(gradient.len() - 1);
        let desired = gradient[gradient_index] as char;
        if let Some((_, index)) = glyph_lookup.iter().find(|(glyph, _)| *glyph == desired) {
            return Some(*index);
        }
        if let Some((_, index)) = glyph_lookup.iter().find(|(glyph, _)| *glyph == '*') {
            return Some(*index);
        }
        if let Some((_, index)) = glyph_lookup.iter().find(|(glyph, _)| *glyph == '+') {
            return Some(*index);
        }
        Some(glyph_lookup[(gradient_index * glyph_lookup.len() / gradient.len()).min(glyph_lookup.len() - 1)].1)
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
