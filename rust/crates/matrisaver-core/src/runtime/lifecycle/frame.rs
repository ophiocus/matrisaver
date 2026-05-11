// Per-frame stream instance building, column pitch, and column count.
impl CoreRuntime {
    fn build_stream_instances(&mut self, delta_seconds: f32) -> Vec<renderer::GlyphInstance> {
        self.ensure_rain_columns();
        let height = self.surface_size.1.max(1);
        let char_size = self.settings.char_size.max(1) as u32;
        let rows = (height / char_size).max(1);
        let column_pitch = Self::column_pitch(char_size);
        let mut instances = Vec::new();
        let now = self.animation_seconds;
        let frame_dt = (delta_seconds * 60.0).max(0.1);
        let speed_min = self.runtime_config.speed_range.0 as f32;
        let speed_max =
            self.runtime_config.speed_range.1.max(self.runtime_config.speed_range.0) as f32;
        self.update_overlay_state(now, rows, frame_dt);
        let glyphs = if self.atlas.glyphs.is_empty() {
            return Vec::new();
        } else {
            &self.atlas.glyphs
        };
        // Temporary parity clamp: all variants currently run the original lifecycle model.
        let run_original_lifecycle_for_all_variants = true;
        let lifecycle_mutators = self.original_lifecycle_mutators();
        let lifecycle_ctx = LifecycleTickContext {
            now,
            frame_dt,
            rows,
            char_size,
            volatile_chance: self.runtime_config.volatile_chance,
            ghost_chance: self.runtime_config.ghost_chance,
            ghost_swap_multiplier: self.runtime_config.ghost_swap_multiplier,
            trail_length_multiplier: self.runtime_config.trail_length_multiplier,
            super_volatile_pulse_time: self.super_volatile_pulse_time,
        };

        for (column_index, column) in self.rain_columns.iter_mut().enumerate() {
            if run_original_lifecycle_for_all_variants {
                if column.row_cells.len() != rows as usize {
                    column.row_cells.resize(
                        rows as usize,
                        RowCell {
                            glyph_index: None,
                            brightness: 0.0,
                            volatile: false,
                            volatile_next: 0.0,
                            volatile_last: 0.0,
                            super_volatile: false,
                            frozen: false,
                        },
                    );
                }
                Self::update_original_column(column, column_index, lifecycle_ctx, lifecycle_mutators);
                Self::emit_original_instances(
                    column,
                    char_size,
                    rows,
                    glyphs,
                    now,
                    self.runtime_config.ghost_swap_multiplier,
                    &mut instances,
                );
                continue;
            }

            if now >= column.next_glyph_swap_at {
                for glyph_index in &mut column.glyph_indices {
                    *glyph_index = glyph_index.wrapping_add(1 + (hash01(column_index as u32, *glyph_index) * 31.0) as u32);
                }
                column.glyph_swap_count = column.glyph_swap_count.saturating_add(1);
                column.next_glyph_swap_at = now + 0.33 + hash01(column_index as u32, self.frame_index as u32) * 4.67;

                if !column.y_positions.is_empty() {
                    let lead_y = column.y_positions[0];
                    for (idx, y_value) in column.y_positions.iter().enumerate() {
                        if *y_value >= 0.0
                            && *y_value < height as f32
                            && hash01(column_index as u32, idx as u32 ^ self.frame_index as u32)
                                < self.runtime_config.ghost_chance
                        {
                            column.ghosts.push(GhostGlyph {
                                row: *y_value / char_size as f32,
                                glyph_index: column.glyph_indices.get(idx).copied().unwrap_or(0),
                                next_swap_at: now
                                    + (0.33
                                        + hash01(idx as u32, column_index as u32) * 4.67)
                                        * self.runtime_config.ghost_swap_multiplier.max(1.0),
                            });
                        }
                    }
                    let cutoff = char_size as f32 * 0.9;
                    column.ghosts.retain(|ghost| {
                        let y = ghost.row * char_size as f32;
                        (y - lead_y).abs() > cutoff
                            && y > -(char_size as f32)
                            && y < height as f32 + char_size as f32
                    });
                }
            }

            for idx in 0..column.y_positions.len() {
                let mut speed = *column.speeds.get(idx).unwrap_or(&speed_min);
                if hash01(self.frame_index as u32, (column_index as u32) ^ idx as u32)
                    < self.runtime_config.pause_chance
                {
                    speed *= 0.2;
                }
                if hash01(self.frame_index as u32 ^ 0x7777_7777, (column_index as u32) ^ idx as u32)
                    < self.runtime_config.jitter_chance
                {
                    let jitter = 0.4
                        + hash01(column_index as u32, idx as u32 ^ self.frame_index as u32) * 1.2;
                    speed *= jitter;
                }
                if let Some(current_speed) = column.current_speeds.get_mut(idx) {
                    *current_speed = speed;
                }
                if let Some(y_value) = column.y_positions.get_mut(idx) {
                    *y_value += speed * frame_dt;
                }
            }

            if column
                .y_positions
                .first()
                .copied()
                .unwrap_or(-1.0)
                > height as f32 + char_size as f32 * 4.0
            {
                Self::reset_column_non_original(
                    column,
                    now,
                    speed_min,
                    speed_max,
                    height,
                    rows,
                    char_size,
                );
            }

            let x = (column.column_slot as f32 + 0.5) * column_pitch;
            for (idx, y_value) in column.y_positions.iter().enumerate() {
                if *y_value < 0.0 || *y_value >= height as f32 {
                    continue;
                }
                let row = (*y_value / char_size as f32) as u32;
                let speed_value = column.current_speeds.get(idx).copied().unwrap_or(speed_min);
                let speed_factor = (speed_value / speed_max.max(1.0)).clamp(0.2, 1.2);
                let brightness = (0.25 + 0.65 * speed_factor).clamp(0.0, 1.0);
                let head_boost = if idx == 0 { 1.0 } else { 0.15 };
                let grain = hash01(column_index as u32, row.wrapping_add(idx as u32));
                let glyph_index = column.glyph_indices.get(idx).copied().unwrap_or(0) as usize;
                let glyph = glyphs[glyph_index % glyphs.len()];
                instances.push(renderer::GlyphInstance {
                    position_size: [x, *y_value + char_size as f32 * 0.5, char_size as f32, char_size as f32],
                    uv_rect: [glyph.u0, glyph.v0, glyph.u1, glyph.v1],
                    params: [brightness, head_boost, grain, 0.0],
                });
            }

            for ghost in &mut column.ghosts {
                let y = ghost.row * char_size as f32;
                if y < 0.0 || y >= height as f32 {
                    continue;
                }
                if now >= ghost.next_swap_at {
                    ghost.glyph_index = ghost.glyph_index.wrapping_add(
                        1 + (hash01(column_index as u32, ghost.glyph_index) * 23.0) as u32,
                    );
                    ghost.next_swap_at = now
                        + (0.33 + hash01(ghost.glyph_index, column_index as u32) * 4.67)
                            * self.runtime_config.ghost_swap_multiplier.max(1.0);
                }
                let glyph = glyphs[ghost.glyph_index as usize % glyphs.len()];
                instances.push(renderer::GlyphInstance {
                    position_size: [x, y + char_size as f32 * 0.5, char_size as f32, char_size as f32],
                    uv_rect: [glyph.u0, glyph.v0, glyph.u1, glyph.v1],
                    params: [0.18, 0.05, 0.2, 2.0],
                });
            }
        }

        self.emit_overlay_intro_instances(&mut instances, glyphs, height as f32, char_size);
        self.emit_overlay_header_instances(&mut instances, glyphs, height as f32, char_size);

        instances
    }
    fn column_pitch(char_size: u32) -> f32 {
        (char_size.max(1) as f32) * COLUMN_PITCH_SCALE
    }

    fn column_count(width: u32, char_size: u32) -> u32 {
        let pitch = Self::column_pitch(char_size).max(1.0);
        ((width.max(1) as f32) / pitch).floor().max(1.0) as u32
    }
}
