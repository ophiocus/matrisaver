// Overlay injection from image files into the rain column grid.
type ColumnRowTargets = std::collections::BTreeMap<usize, (u32, f32)>;

impl CoreRuntime {
    fn inject_overlay_from_image(&mut self, _rows: u32) -> bool {
        self.clear_overlay_locks();
        self.overlay_injected_count = 0;
        self.overlay_image_name = "none".to_owned();
        self.overlay_headers.clear();
        self.overlay_intro_glyphs.clear();
        let mut tuning = self.load_overlay_tuning();
        // Settings.overlay_auto_levels is the dialog-facing toggle and
        // wins over whatever overlay_tuning.json says. The JSON
        // surface stays available for power-users who hand-edit it.
        tuning.auto_levels_enabled = self.settings.overlay_auto_levels;
        self.overlay_tuning = tuning;
        let image_paths = self.overlay_image_paths();
        if image_paths.is_empty() {
            return false;
        }
        let (image_path, write_ascii) =
            image_paths[self.overlay_image_cursor % image_paths.len()].clone();
        let image_path = image_path.as_path();
        self.overlay_image_cursor = self.overlay_image_cursor.wrapping_add(1);
        let image_name = image_path
            .file_name()
            .and_then(|value| value.to_str())
            .map(Self::sanitize_trace_token)
            .unwrap_or_else(|| "unknown".to_owned());

        let Ok(image) = image::open(image_path) else {
            return false;
        };
        let image = image.to_rgba8();
        let width = image.width().max(1);
        let height = image.height().max(1);
        let char_size = self.settings.char_size.max(1) as u32;
        let (ref_x, ref_y, ref_w, ref_h) = self
            .overlay_reference_rect
            .unwrap_or((0, 0, self.surface_size.0.max(1), self.surface_size.1.max(1)));
        let pitch = Self::column_pitch(char_size).max(1.0);
        let col_start = (ref_x as f32 / pitch).floor() as i32;
        let col_end = ((ref_x + ref_w) as f32 / pitch).ceil() as i32;
        let cols = (col_end - col_start).max(1) as u32;
        let ascii_cols = ((cols as f32) * COLUMN_PITCH_SCALE).floor().max(1.0) as u32;
        let column_span = (1.0 / COLUMN_PITCH_SCALE).round().max(1.0) as u32;
        let row_start = (ref_y / char_size) as i32;
        let row_end = ((ref_y + ref_h) as f32 / char_size as f32).ceil() as i32;
        let rows = (row_end - row_start).max(1) as u32;
        let image_aspect = width as f32 / height as f32;
        let target_aspect = ascii_cols as f32 / rows as f32;
        let (fit_cols, fit_rows) = if image_aspect > target_aspect {
            let fit_cols = ascii_cols;
            let fit_rows = ((fit_cols as f32 / image_aspect).round() as u32).max(1);
            (fit_cols, fit_rows)
        } else {
            let fit_rows = rows;
            let fit_cols = ((fit_rows as f32 * image_aspect).round() as u32).max(1);
            (fit_cols, fit_rows)
        };
        let col_offset = ((ascii_cols.saturating_sub(fit_cols)) / 2) as i32;
        let row_offset = (rows as i32 - fit_rows as i32).max(0);

        let sample_len = (fit_cols * fit_rows) as usize;
        let mut sampled_alpha = vec![0.0f32; sample_len];
        let mut sampled_luma = vec![0.0f32; sample_len];
        for cell_row in 0..fit_rows {
            for cell_col in 0..fit_cols {
                let (alpha, luminance) = Self::sample_overlay_cell(
                    &image,
                    CellGrid { cols: fit_cols, rows: fit_rows },
                    cell_col,
                    cell_row,
                    tuning.luma_weights,
                );
                let index = (cell_row * fit_cols + cell_col) as usize;
                sampled_alpha[index] = alpha;
                sampled_luma[index] = luminance;
            }
        }

        // V2: no preprocessing pipeline. Per the research synthesis,
        // canonical ASCII-conversion tools default to passthrough;
        // only `auto_levels` (a contrast-stretch) is exposed, and
        // only as a user opt-in. Compute the levels bounds once for
        // each grid (header + dense intro), but apply them only if
        // tuning.auto_levels_enabled.
        let (levels_low, levels_high) = if tuning.auto_levels_enabled {
            let mut valid_luma = Vec::with_capacity(sample_len / 2);
            for index in 0..sample_len {
                if sampled_alpha[index] >= tuning.alpha_cutoff {
                    valid_luma.push(sampled_luma[index]);
                }
            }
            Self::auto_levels(
                &mut valid_luma,
                tuning.levels_low_percentile,
                tuning.levels_high_percentile,
            )
        } else {
            (0.0, 1.0)
        };

        let intro_density_columns = tuning.intro_density_multiplier_x.round().max(1.0) as u32;
        let dense_fit_cols = fit_cols.saturating_mul(intro_density_columns).max(1);
        let dense_sample_len = (dense_fit_cols * fit_rows) as usize;
        let mut dense_alpha = vec![0.0f32; dense_sample_len];
        let mut dense_luma = vec![0.0f32; dense_sample_len];
        for cell_row in 0..fit_rows {
            for dense_col in 0..dense_fit_cols {
                let (alpha, luminance) = Self::sample_overlay_cell(
                    &image,
                    CellGrid { cols: dense_fit_cols, rows: fit_rows },
                    dense_col,
                    cell_row,
                    tuning.luma_weights,
                );
                let index = (cell_row * dense_fit_cols + dense_col) as usize;
                dense_alpha[index] = alpha;
                dense_luma[index] = luminance;
            }
        }
        let (dense_levels_low, dense_levels_high) = if tuning.auto_levels_enabled {
            let mut dense_valid_luma = Vec::with_capacity(dense_sample_len / 2);
            for index in 0..dense_sample_len {
                if dense_alpha[index] >= tuning.alpha_cutoff {
                    dense_valid_luma.push(dense_luma[index]);
                }
            }
            Self::auto_levels(
                &mut dense_valid_luma,
                tuning.levels_low_percentile,
                tuning.levels_high_percentile,
            )
        } else {
            (0.0, 1.0)
        };

        let mut slot_to_column = std::collections::HashMap::with_capacity(self.rain_columns.len());
        for (index, column) in self.rain_columns.iter().enumerate() {
            slot_to_column.insert(column.column_slot, index);
        }

        let glyph_lookup = self.overlay_glyph_lookup();
        let mut per_column_targets: std::collections::HashMap<usize, ColumnRowTargets> =
            std::collections::HashMap::new();
        let mut intro_targets: std::collections::HashMap<(u32, usize, u32), (u32, f32)> =
            std::collections::HashMap::new();
        for cell_row in 0..fit_rows {
            for cell_col in 0..fit_cols {
                let index = (cell_row * fit_cols + cell_col) as usize;
                let alpha = sampled_alpha[index];
                let raw_luminance = sampled_luma[index];
                if alpha < tuning.alpha_cutoff {
                    continue;
                }
                // V2: gamma/contrast stages dropped. Apply optional
                // auto-levels remap if enabled; otherwise raw luminance
                // goes straight to glyph mapping.
                let shaped = if tuning.auto_levels_enabled {
                    Self::remap_level(raw_luminance, levels_low, levels_high)
                } else {
                    raw_luminance.clamp(0.0, 1.0)
                };
                let glyph_index = Self::overlay_glyph_index_for_luminance(shaped, &glyph_lookup);
                let Some(glyph_index) = glyph_index else {
                    continue;
                };
                let grid_row = row_start + row_offset + cell_row as i32;
                if grid_row < row_start || grid_row >= row_end {
                    continue;
                }
                let logical_col = col_offset + cell_col as i32;
                let physical_start = col_start + (logical_col as f32 / COLUMN_PITCH_SCALE).round() as i32;
                for span in 0..column_span {
                    let grid_col = physical_start + span as i32;
                    if grid_col < col_start || grid_col >= col_end {
                        continue;
                    }
                    let Some(column_index) = slot_to_column.get(&(grid_col as u32)).copied() else {
                        continue;
                    };
                    let row_index = grid_row as usize;
                    if self
                        .rain_columns
                        .get(column_index)
                        .and_then(|column| column.row_cells.get(row_index))
                        .is_none()
                    {
                        continue;
                    }
                    let Some(column_slot) = self
                        .rain_columns
                        .get(column_index)
                        .map(|column| column.column_slot)
                    else {
                        continue;
                    };
                    let brightness = (tuning.brightness_floor + shaped * alpha * tuning.brightness_scale)
                        .clamp(tuning.brightness_floor, 1.0);
                    per_column_targets
                        .entry(column_index)
                        .or_default()
                        .entry(row_index)
                        .and_modify(|value| {
                            if brightness > value.1 {
                                *value = (glyph_index, brightness);
                            }
                        })
                        .or_insert((glyph_index, brightness));

                    for intro_index in 0..intro_density_columns {
                        let dense_col = cell_col * intro_density_columns + intro_index;
                        let dense_index = (cell_row * dense_fit_cols + dense_col) as usize;
                        let dense_alpha_value = dense_alpha[dense_index];
                        if dense_alpha_value < tuning.alpha_cutoff {
                            continue;
                        }
                        let dense_shaped = if tuning.auto_levels_enabled {
                            Self::remap_level(dense_luma[dense_index], dense_levels_low, dense_levels_high)
                        } else {
                            dense_luma[dense_index].clamp(0.0, 1.0)
                        };
                        let Some(dense_glyph_index) =
                            Self::overlay_glyph_index_for_luminance(dense_shaped, &glyph_lookup)
                        else {
                            continue;
                        };
                        let dense_brightness = (tuning.brightness_floor
                            + dense_shaped * dense_alpha_value * tuning.brightness_scale)
                            * tuning.intro_layer_brightness_scale;
                        let dense_brightness =
                            dense_brightness.clamp(tuning.brightness_floor, 1.0);
                        let key = (column_slot, row_index, intro_index);
                        intro_targets
                            .entry(key)
                            .and_modify(|value| {
                                if dense_brightness > value.1 {
                                    *value = (dense_glyph_index, dense_brightness);
                                }
                            })
                            .or_insert((dense_glyph_index, dense_brightness));
                    }
                }
            }
        }

        self.overlay_intro_glyphs.reserve(intro_targets.len());
        for ((column_slot, row_index, intro_index), (glyph_index, brightness)) in intro_targets {
            let intro_center = (intro_index as f32 + 0.5) / intro_density_columns as f32 - 0.5;
            self.overlay_intro_glyphs.push(OverlayIntroGlyph {
                column_slot,
                row_index,
                x_offset: intro_center * char_size as f32,
                glyph_index,
                brightness,
            });
        }

        let top_y = row_start as f32 * char_size as f32;
        let header_speed = (self.runtime_config.speed_range.1 as f32 * 3.0).max(1.0);
        let mut columns: Vec<(usize, ColumnRowTargets)> =
            per_column_targets.into_iter().collect();
        columns.sort_by_key(|(column_index, _)| *column_index);
        for (order, (column_index, targets)) in columns.into_iter().enumerate() {
            let Some(column_slot) = self.rain_columns.get(column_index).map(|column| column.column_slot)
            else {
                continue;
            };
            let target_list: Vec<OverlayTargetCell> = targets
                .into_iter()
                .map(|(row_index, (glyph_index, brightness))| OverlayTargetCell {
                    row_index,
                    glyph_index,
                    brightness,
                })
                .collect();
            if target_list.is_empty() {
                continue;
            }

            let start_y = match self.overlay_intro_mode {
                OverlayIntroMode::AllAtOnce => top_y - char_size as f32,
                OverlayIntroMode::WaveLeftToRight => {
                    top_y - char_size as f32 - order as f32 * (char_size as f32 * 0.75)
                }
            };
            self.overlay_headers.push(OverlayHeader {
                column_slot,
                y: start_y,
                speed: header_speed,
                glyph_index: target_list[0].glyph_index,
                brightness: target_list[0].brightness,
                next_target_index: 0,
                targets: target_list,
            });
        }

        // Side-effect snapshot for the user: write the rendered glyph
        // grid as plain text next to the source image. Skipped
        // silently when the directory's write probe failed (cached
        // per session) or when the source didn't opt in.
        if write_ascii {
            let auto_levels = if tuning.auto_levels_enabled {
                Some((levels_low, levels_high))
            } else {
                None
            };
            let grid_text = Self::render_overlay_grid_text(
                &sampled_alpha,
                &sampled_luma,
                fit_cols,
                fit_rows,
                tuning.alpha_cutoff,
                auto_levels,
            );
            self.write_overlay_ascii_alongside(image_path, &grid_text);
        }

        if !self.overlay_headers.is_empty() {
            self.overlay_image_name = image_name;
            true
        } else {
            self.overlay_intro_glyphs.clear();
            false
        }
    }
}
