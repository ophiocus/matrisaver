// Column reset operations: head glyph advance, head reset, eraser reset, non-original reset, ensure columns.
impl CoreRuntime {
    fn advance_head_glyph(current: u32, now: f32) -> u32 {
        let offset = 1 + (hash01(current ^ 0xABCD_1020, (now * 1000.0) as u32) * 49.0) as u32;
        current.wrapping_add(offset)
    }

    fn reset_head(
        column: &mut RainColumn,
        now: f32,
        rows: u32,
        char_size: u32,
        trail_length_multiplier: f32,
        mutators: OriginalLifecycleMutators,
    ) {
        column.head_reset_count = column.head_reset_count.saturating_add(1);
        let seed = hash01((now * 1000.0) as u32, column.glyph_cursor);
        column.head_y = -(rows as f32 * char_size as f32) * seed;
        let base_head_speed = 2.5 + hash01(column.glyph_cursor, seed.to_bits()) * 3.5;
        column.head_speed = (base_head_speed * mutators.head_speed_scale).clamp(0.5, 24.0);
        column.delete_gap = rows as f32
            * char_size as f32
            * trail_length_multiplier
            * mutators.delete_gap_scale
            + hash01(seed.to_bits(), 11) * 0.2;
        column.last_head_row = -1;
        column.head_row_step = 0;
        column.head_glyph_index = column.glyph_cursor.wrapping_add((seed * 1024.0) as u32);
    }

    fn reset_eraser(
        column: &mut RainColumn,
        now: f32,
        char_size: u32,
        mutators: OriginalLifecycleMutators,
    ) {
        column.eraser_reset_count = column.eraser_reset_count.saturating_add(1);
        let seed = hash01((now * 1000.0) as u32, column.glyph_cursor.wrapping_add(91));
        let speed_factor = (0.6 + seed * 0.8) * mutators.eraser_speed_scale;
        column.eraser_speed = (column.head_speed * speed_factor).max(0.4);
        let trail_height = (column.row_cells.len() as f32 * char_size as f32 * 0.35)
            .max(char_size as f32 * 6.0);
        column.eraser_offset =
            (char_size as f32 * 6.0 + seed * (trail_height - char_size as f32 * 6.0))
                * mutators.eraser_offset_scale;
        if hash01(seed.to_bits(), column.glyph_cursor) < 0.25 {
            column.eraser_y = column.head_y + column.eraser_offset;
        } else {
            column.eraser_y = column.head_y - column.eraser_offset;
        }
        column.eraser_last_row = -1;
    }

    fn reset_column_non_original(
        column: &mut RainColumn,
        now: f32,
        speed_min: f32,
        speed_max: f32,
        height: u32,
        _rows: u32,
        _char_size: u32,
    ) {
        column.chain_reset_count = column.chain_reset_count.saturating_add(1);
        let chain_length = 12 + (hash01(column.column_slot, (now * 1000.0) as u32) * 21.0) as usize;
        column.y_positions.clear();
        column.speeds.clear();
        column.current_speeds.clear();
        column.glyph_indices.clear();
        column.y_positions.reserve(chain_length);
        column.speeds.reserve(chain_length);
        column.current_speeds.reserve(chain_length);
        column.glyph_indices.reserve(chain_length);

        for idx in 0..chain_length {
            let y_seed = hash01(column.column_slot, idx as u32 ^ 0x5555_5555);
            let speed_seed = hash01(column.column_slot, idx as u32 ^ 0x6666_6666);
            let glyph_seed = hash01(column.column_slot, idx as u32 ^ 0x7777_7777);
            column
                .y_positions
                .push(-(height as f32) + y_seed * height as f32);
            let glyph_speed = speed_min + (speed_max - speed_min) * speed_seed;
            column.speeds.push(glyph_speed);
            column.current_speeds.push(glyph_speed);
            column.glyph_indices.push((glyph_seed * 1024.0) as u32);
        }

        column.next_glyph_swap_at = now + 0.33 + hash01(column.column_slot, 0x8888_8888) * 4.67;
        column.ghosts.clear();
    }

    fn ensure_rain_columns(&mut self) {
        let width = self.surface_size.0.max(1);
        let height = self.surface_size.1.max(1);
        let char_size = self.settings.char_size.max(1);
        let cols = Self::column_count(width, char_size as u32);
        if self.rain_layout == (width, height, char_size) && self.rain_columns.len() == cols as usize {
            return;
        }

        self.rain_layout = (width, height, char_size);
        self.rain_columns.clear();
        self.rain_columns.reserve(cols as usize);

        let rows = (height / char_size as u32).max(1);
        let speed_min = self.runtime_config.speed_range.0 as f32;
        let speed_max = self.runtime_config.speed_range.1.max(self.runtime_config.speed_range.0) as f32;
        let density = self.runtime_config.density.clamp(0.3, 1.0);
        let lifecycle_mutators = self.original_lifecycle_mutators();

        for column_index in 0..cols {
            if !self.settings.overlay_enabled && hash01(column_index, 0xDEAD_BEEF) > density {
                continue;
            }
            let seed_a = hash01(column_index, 0xC0FF_EE11);
            let seed_b = hash01(column_index, 0x9E37_79B9);
            let seed_c = hash01(column_index, 0x1234_5678);
            let chain_length = 12 + (hash01(column_index, 0xABCD_1234) * 21.0) as usize;
            let mut y_positions = Vec::with_capacity(chain_length);
            let mut speeds = Vec::with_capacity(chain_length);
            let mut current_speeds = Vec::with_capacity(chain_length);
            let mut glyph_indices = Vec::with_capacity(chain_length);
            for idx in 0..chain_length {
                let y_seed = hash01(column_index, idx as u32 ^ 0x1111_1111);
                let speed_seed = hash01(column_index, idx as u32 ^ 0x2222_2222);
                let glyph_seed = hash01(column_index, idx as u32 ^ 0x3333_3333);
                y_positions.push(-(height as f32) + y_seed * height as f32);
                let glyph_speed = speed_min + (speed_max - speed_min) * speed_seed;
                speeds.push(glyph_speed);
                current_speeds.push(glyph_speed);
                glyph_indices.push((glyph_seed * 1024.0) as u32);
            }
            let next_glyph_swap_at = self.animation_seconds + 0.33 + hash01(column_index, 0x4444_4444) * 4.67;
            let trail_rows = ((rows as f32)
                * self.runtime_config.trail_length_multiplier
                * (0.85 + seed_b * 0.3))
                .max(2.0);
            let mut row_cells = vec![
                RowCell {
                    glyph_index: None,
                    brightness: 0.0,
                    volatile: false,
                    volatile_next: 0.0,
                    volatile_last: 0.0,
                    super_volatile: false,
                    frozen: false,
                };
                rows as usize
            ];
            let head_y = (seed_c * 1.2 - 0.2) * (height as f32);
            let head_row = (head_y / char_size as f32).floor() as i32;
            for trail_index in 0..trail_rows as i32 {
                let row = head_row - trail_index;
                if row < 0 || row >= rows as i32 {
                    continue;
                }
                let trail_t = 1.0 - (trail_index as f32 / trail_rows.max(1.0));
                row_cells[row as usize].glyph_index = Some((seed_b * 255.0) as u32 + trail_index as u32);
                row_cells[row as usize].brightness = (0.25 + trail_t * 0.6).clamp(0.0, 1.0);
            }
            self.rain_columns.push(RainColumn {
                column_slot: column_index,
                y_positions,
                speeds,
                current_speeds,
                glyph_indices,
                next_glyph_swap_at,
                head_y: -(height as f32) * seed_c,
                head_speed: ((2.5 + seed_a * 3.5) * lifecycle_mutators.head_speed_scale)
                    .clamp(0.5, 24.0),
                glyph_cursor: (seed_b * 255.0) as u32,
                head_glyph_index: (seed_b * 1024.0) as u32,
                delete_gap: height as f32
                    * (self.runtime_config.trail_length_multiplier + seed_b * 0.2)
                    * lifecycle_mutators.delete_gap_scale,
                last_head_row: -1,
                head_row_step: 0,
                eraser_y: if seed_a < 0.25 {
                    -(height as f32) * seed_c + (char_size as f32 * 6.0 + seed_b * (height as f32 * 0.35 - char_size as f32 * 6.0).max(0.0)) * lifecycle_mutators.eraser_offset_scale
                } else {
                    -(height as f32) * seed_c - (char_size as f32 * 6.0 + seed_b * (height as f32 * 0.35 - char_size as f32 * 6.0).max(0.0)) * lifecycle_mutators.eraser_offset_scale
                },
                eraser_speed: ((2.5 + seed_a * 3.5)
                    * lifecycle_mutators.head_speed_scale
                    * (0.6 + seed_a * 0.8)
                    * lifecycle_mutators.eraser_speed_scale)
                    .max(0.4),
                eraser_offset: (char_size as f32 * 6.0
                    + seed_b * (height as f32 * 0.35 - char_size as f32 * 6.0).max(0.0))
                    * lifecycle_mutators.eraser_offset_scale,
                eraser_last_row: -1,
                head_reset_count: 0,
                eraser_reset_count: 0,
                head_write_count: 0,
                chain_reset_count: 0,
                glyph_swap_count: 0,
                row_cells,
                ghosts: Vec::new(),
            });
        }

        if self.rain_columns.is_empty() {
            let slot = cols / 2;
            let seed_a = hash01(slot, 0xC0FF_EE11);
            let seed_b = hash01(slot, 0x9E37_79B9);
            let seed_c = hash01(slot, 0x1234_5678);
            let chain_length = 12 + (hash01(slot, 0xABCD_1234) * 21.0) as usize;
            let mut y_positions = Vec::with_capacity(chain_length);
            let mut speeds = Vec::with_capacity(chain_length);
            let mut current_speeds = Vec::with_capacity(chain_length);
            let mut glyph_indices = Vec::with_capacity(chain_length);
            for idx in 0..chain_length {
                let y_seed = hash01(slot, idx as u32 ^ 0x1111_1111);
                let speed_seed = hash01(slot, idx as u32 ^ 0x2222_2222);
                let glyph_seed = hash01(slot, idx as u32 ^ 0x3333_3333);
                y_positions.push(-(height as f32) + y_seed * height as f32);
                let glyph_speed = speed_min + (speed_max - speed_min) * speed_seed;
                speeds.push(glyph_speed);
                current_speeds.push(glyph_speed);
                glyph_indices.push((glyph_seed * 1024.0) as u32);
            }
            let next_glyph_swap_at = self.animation_seconds + 0.33 + hash01(slot, 0x4444_4444) * 4.67;
            let trail_rows = ((rows as f32)
                * self.runtime_config.trail_length_multiplier
                * (0.85 + seed_b * 0.3))
                .max(2.0);
            let mut row_cells = vec![
                RowCell {
                    glyph_index: None,
                    brightness: 0.0,
                    volatile: false,
                    volatile_next: 0.0,
                    volatile_last: 0.0,
                    super_volatile: false,
                    frozen: false,
                };
                rows as usize
            ];
            let head_y = (seed_c * 1.2 - 0.2) * (height as f32);
            let head_row = (head_y / char_size as f32).floor() as i32;
            for trail_index in 0..trail_rows as i32 {
                let row = head_row - trail_index;
                if row < 0 || row >= rows as i32 {
                    continue;
                }
                let trail_t = 1.0 - (trail_index as f32 / trail_rows.max(1.0));
                row_cells[row as usize].glyph_index = Some((seed_b * 255.0) as u32 + trail_index as u32);
                row_cells[row as usize].brightness = (0.25 + trail_t * 0.6).clamp(0.0, 1.0);
            }
            self.rain_columns.push(RainColumn {
                column_slot: slot,
                y_positions,
                speeds,
                current_speeds,
                glyph_indices,
                next_glyph_swap_at,
                head_y: -(height as f32) * seed_c,
                head_speed: ((2.5 + seed_a * 3.5) * lifecycle_mutators.head_speed_scale)
                    .clamp(0.5, 24.0),
                glyph_cursor: (seed_b * 255.0) as u32,
                head_glyph_index: (seed_b * 1024.0) as u32,
                delete_gap: height as f32
                    * (self.runtime_config.trail_length_multiplier + seed_b * 0.2)
                    * lifecycle_mutators.delete_gap_scale,
                last_head_row: -1,
                head_row_step: 0,
                eraser_y: if seed_a < 0.25 {
                    -(height as f32) * seed_c + (char_size as f32 * 6.0 + seed_b * (height as f32 * 0.35 - char_size as f32 * 6.0).max(0.0)) * lifecycle_mutators.eraser_offset_scale
                } else {
                    -(height as f32) * seed_c - (char_size as f32 * 6.0 + seed_b * (height as f32 * 0.35 - char_size as f32 * 6.0).max(0.0)) * lifecycle_mutators.eraser_offset_scale
                },
                eraser_speed: ((2.5 + seed_a * 3.5)
                    * lifecycle_mutators.head_speed_scale
                    * (0.6 + seed_a * 0.8)
                    * lifecycle_mutators.eraser_speed_scale)
                    .max(0.4),
                eraser_offset: (char_size as f32 * 6.0
                    + seed_b * (height as f32 * 0.35 - char_size as f32 * 6.0).max(0.0))
                    * lifecycle_mutators.eraser_offset_scale,
                eraser_last_row: -1,
                head_reset_count: 0,
                eraser_reset_count: 0,
                head_write_count: 0,
                chain_reset_count: 0,
                glyph_swap_count: 0,
                row_cells,
                ghosts: Vec::new(),
            });
        }
    }
}
