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

    fn preprocess_overlay_luminance(
        luminance: &mut [f32],
        alpha: &[f32],
        cols: u32,
        rows: u32,
        tuning: OverlayTuning,
    ) {
        if luminance.is_empty()
            || alpha.is_empty()
            || luminance.len() != alpha.len()
            || cols == 0
            || rows == 0
        {
            return;
        }

        match tuning.denoise_mode {
            OverlayDenoiseMode::None => {}
            OverlayDenoiseMode::Median => {
                Self::apply_overlay_median_filter(
                    luminance,
                    alpha,
                    cols,
                    rows,
                    tuning.denoise_strength,
                    tuning.alpha_cutoff,
                );
            }
            OverlayDenoiseMode::Bilateral => {
                Self::apply_overlay_bilateral_filter(
                    luminance,
                    alpha,
                    cols,
                    rows,
                    tuning.denoise_strength,
                    tuning.alpha_cutoff,
                );
            }
        }

        if tuning.clahe_enabled {
            Self::apply_overlay_clahe(
                luminance,
                alpha,
                cols,
                rows,
                tuning.clahe_clip_limit,
                tuning.clahe_tile_grid,
                tuning.alpha_cutoff,
            );
        }

        if tuning.unsharp_enabled {
            Self::apply_overlay_unsharp(
                luminance,
                alpha,
                cols,
                rows,
                tuning.unsharp_amount,
                tuning.alpha_cutoff,
            );
        }
    }

    fn apply_overlay_median_filter(
        luminance: &mut [f32],
        alpha: &[f32],
        cols: u32,
        rows: u32,
        strength: f32,
        alpha_cutoff: f32,
    ) {
        let radius = if strength < 0.35 {
            1i32
        } else if strength < 0.75 {
            2i32
        } else {
            3i32
        };
        let src = luminance.to_vec();
        let mut window = Vec::with_capacity(((radius * 2 + 1).pow(2)) as usize);
        for row in 0..rows as i32 {
            for col in 0..cols as i32 {
                let idx = (row as u32 * cols + col as u32) as usize;
                if alpha[idx] < alpha_cutoff {
                    continue;
                }
                window.clear();
                for oy in -radius..=radius {
                    let sample_row = (row + oy).clamp(0, rows as i32 - 1) as u32;
                    for ox in -radius..=radius {
                        let sample_col = (col + ox).clamp(0, cols as i32 - 1) as u32;
                        let sample_idx = (sample_row * cols + sample_col) as usize;
                        if alpha[sample_idx] >= alpha_cutoff {
                            window.push(src[sample_idx]);
                        }
                    }
                }
                if window.is_empty() {
                    continue;
                }
                window.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                luminance[idx] = window[window.len() / 2];
            }
        }
    }

    fn apply_overlay_bilateral_filter(
        luminance: &mut [f32],
        alpha: &[f32],
        cols: u32,
        rows: u32,
        strength: f32,
        alpha_cutoff: f32,
    ) {
        let radius = if strength < 0.35 {
            1i32
        } else if strength < 0.75 {
            2i32
        } else {
            3i32
        };
        let spatial_sigma = 0.9 + strength * 2.2;
        let range_sigma = 0.06 + strength * 0.24;
        let spatial_sigma2 = (2.0 * spatial_sigma * spatial_sigma).max(1e-4);
        let range_sigma2 = (2.0 * range_sigma * range_sigma).max(1e-4);
        let src = luminance.to_vec();

        for row in 0..rows as i32 {
            for col in 0..cols as i32 {
                let idx = (row as u32 * cols + col as u32) as usize;
                if alpha[idx] < alpha_cutoff {
                    continue;
                }
                let center = src[idx];
                let mut weighted_sum = 0.0;
                let mut weight_sum = 0.0;
                for oy in -radius..=radius {
                    let sample_row = (row + oy).clamp(0, rows as i32 - 1) as u32;
                    for ox in -radius..=radius {
                        let sample_col = (col + ox).clamp(0, cols as i32 - 1) as u32;
                        let sample_idx = (sample_row * cols + sample_col) as usize;
                        if alpha[sample_idx] < alpha_cutoff {
                            continue;
                        }
                        let sample = src[sample_idx];
                        let distance2 = (ox * ox + oy * oy) as f32;
                        let delta = sample - center;
                        let spatial_weight = (-distance2 / spatial_sigma2).exp();
                        let range_weight = (-(delta * delta) / range_sigma2).exp();
                        let alpha_weight = alpha[sample_idx].clamp(0.05, 1.0);
                        let weight = spatial_weight * range_weight * alpha_weight;
                        weighted_sum += sample * weight;
                        weight_sum += weight;
                    }
                }
                if weight_sum > 1e-6 {
                    luminance[idx] = (weighted_sum / weight_sum).clamp(0.0, 1.0);
                }
            }
        }
    }

    fn apply_overlay_clahe(
        luminance: &mut [f32],
        alpha: &[f32],
        cols: u32,
        rows: u32,
        clip_limit: f32,
        tile_grid: (u32, u32),
        alpha_cutoff: f32,
    ) {
        let bins = 64usize;
        let tile_cols = tile_grid.0.max(1).min(cols.max(1));
        let tile_rows = tile_grid.1.max(1).min(rows.max(1));
        let tile_w = cols.div_ceil(tile_cols).max(1);
        let tile_h = rows.div_ceil(tile_rows).max(1);

        for tile_row in 0..tile_rows {
            for tile_col in 0..tile_cols {
                let x0 = tile_col * tile_w;
                let y0 = tile_row * tile_h;
                let x1 = ((tile_col + 1) * tile_w).min(cols);
                let y1 = ((tile_row + 1) * tile_h).min(rows);
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }

                let mut histogram = vec![0.0f32; bins];
                let mut sample_count = 0.0f32;
                for sample_row in y0..y1 {
                    for sample_col in x0..x1 {
                        let idx = (sample_row * cols + sample_col) as usize;
                        if alpha[idx] < alpha_cutoff {
                            continue;
                        }
                        let bin =
                            (luminance[idx].clamp(0.0, 1.0) * (bins - 1) as f32).round() as usize;
                        histogram[bin.min(bins - 1)] += 1.0;
                        sample_count += 1.0;
                    }
                }
                if sample_count <= 1.0 {
                    continue;
                }

                let clipped_limit = (clip_limit.max(0.5) * (sample_count / bins as f32)).max(1.0);
                let mut excess = 0.0;
                for bin in &mut histogram {
                    if *bin > clipped_limit {
                        excess += *bin - clipped_limit;
                        *bin = clipped_limit;
                    }
                }
                if excess > 0.0 {
                    let redistributed = excess / bins as f32;
                    for bin in &mut histogram {
                        *bin += redistributed;
                    }
                }

                let mut cumulative = vec![0.0f32; bins];
                let mut running = 0.0;
                for (index, count) in histogram.iter().enumerate() {
                    running += *count;
                    cumulative[index] = running;
                }
                let cdf_min = cumulative.iter().copied().find(|value| *value > 0.0).unwrap_or(0.0);
                let cdf_span = (running - cdf_min).max(1e-4);

                for sample_row in y0..y1 {
                    for sample_col in x0..x1 {
                        let idx = (sample_row * cols + sample_col) as usize;
                        if alpha[idx] < alpha_cutoff {
                            continue;
                        }
                        let bin =
                            (luminance[idx].clamp(0.0, 1.0) * (bins - 1) as f32).round() as usize;
                        let equalized = ((cumulative[bin.min(bins - 1)] - cdf_min) / cdf_span)
                            .clamp(0.0, 1.0);
                        luminance[idx] = equalized;
                    }
                }
            }
        }
    }

    fn apply_overlay_unsharp(
        luminance: &mut [f32],
        alpha: &[f32],
        cols: u32,
        rows: u32,
        amount: f32,
        alpha_cutoff: f32,
    ) {
        let sharpen_amount = amount.clamp(0.0, 2.0);
        if sharpen_amount <= 0.0 {
            return;
        }
        let src = luminance.to_vec();
        let mut blurred = src.clone();
        for row in 0..rows as i32 {
            for col in 0..cols as i32 {
                let idx = (row as u32 * cols + col as u32) as usize;
                if alpha[idx] < alpha_cutoff {
                    continue;
                }
                let mut sum = 0.0;
                let mut weight_sum = 0.0;
                for oy in -1..=1 {
                    let sample_row = (row + oy).clamp(0, rows as i32 - 1) as u32;
                    for ox in -1..=1 {
                        let sample_col = (col + ox).clamp(0, cols as i32 - 1) as u32;
                        let sample_idx = (sample_row * cols + sample_col) as usize;
                        if alpha[sample_idx] < alpha_cutoff {
                            continue;
                        }
                        let kernel_weight = if ox == 0 && oy == 0 {
                            4.0
                        } else if ox == 0 || oy == 0 {
                            2.0
                        } else {
                            1.0
                        };
                        let weighted = kernel_weight * alpha[sample_idx].clamp(0.05, 1.0);
                        sum += src[sample_idx] * weighted;
                        weight_sum += weighted;
                    }
                }
                if weight_sum > 1e-6 {
                    blurred[idx] = (sum / weight_sum).clamp(0.0, 1.0);
                }
            }
        }

        for row in 0..rows {
            for col in 0..cols {
                let idx = (row * cols + col) as usize;
                if alpha[idx] < alpha_cutoff {
                    continue;
                }
                let detail = src[idx] - blurred[idx];
                luminance[idx] = (src[idx] + detail * sharpen_amount).clamp(0.0, 1.0);
            }
        }
    }

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
