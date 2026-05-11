// Cell-level operations: head write, erase, ghost spawn, volatile update, ghost update.
impl CoreRuntime {
    fn write_head_row(
        column: &mut RainColumn,
        row_index: usize,
        now: f32,
        volatile_chance: f32,
        glyph_index: u32,
    ) -> bool {
        if row_index >= column.row_cells.len() {
            return false;
        }
        let cell = &mut column.row_cells[row_index];
        if cell.frozen {
            return false;
        }
        column.head_write_count = column.head_write_count.saturating_add(1);
        cell.glyph_index = Some(glyph_index);
        cell.brightness = 0.4 + hash01(row_index as u32, column.glyph_cursor) * 0.4;
        if hash01(row_index as u32, (now * 1000.0) as u32) < volatile_chance {
            cell.volatile = true;
            cell.volatile_next = 0.0;
            cell.volatile_last = 0.0;
            cell.super_volatile = hash01(column.glyph_cursor, row_index as u32) < 0.3;
        } else {
            cell.volatile = false;
            cell.volatile_next = 0.0;
            cell.volatile_last = 0.0;
            cell.super_volatile = false;
        }
        true
    }

    fn erase_row(column: &mut RainColumn, row_index: usize) {
        if row_index >= column.row_cells.len() {
            return;
        }
        let cell = &mut column.row_cells[row_index];
        if cell.frozen {
            return;
        }
        cell.glyph_index = None;
        cell.brightness = 0.0;
        cell.volatile = false;
        cell.volatile_next = 0.0;
        cell.volatile_last = 0.0;
        cell.super_volatile = false;
    }

    fn maybe_spawn_ghost(
        column: &mut RainColumn,
        row_index: usize,
        now: f32,
        ghost_chance: f32,
        ghost_swap_multiplier: f32,
    ) {
        if hash01(row_index as u32, (now * 1000.0) as u32) >= ghost_chance {
            return;
        }
        column.ghosts.push(GhostGlyph {
            row: row_index as f32,
            glyph_index: column.glyph_cursor,
            next_swap_at: now + (0.33 + hash01(row_index as u32, column.glyph_cursor) * 4.67)
                * ghost_swap_multiplier.max(1.0),
        });
    }

    fn update_volatile_cells(
        column: &mut RainColumn,
        now: f32,
        super_volatile_pulse_time: Option<f32>,
        volatile_interval_scale: f32,
        super_volatile_bonus: f32,
    ) {
        let interval_scale = volatile_interval_scale.clamp(0.2, 2.5);
        let super_volatile_trigger = (0.15 + super_volatile_bonus).clamp(0.0, 1.0);
        for (row_index, cell) in column.row_cells.iter_mut().enumerate() {
            if cell.frozen || !cell.volatile {
                continue;
            }
            let Some(glyph_index) = cell.glyph_index else {
                continue;
            };
            if cell.super_volatile {
                if let Some(pulse_time) = super_volatile_pulse_time {
                    cell.glyph_index = Some(glyph_index.wrapping_add(17));
                    cell.volatile_last = pulse_time;
                    continue;
                }
            }
            if cell.super_volatile
                && hash01(row_index as u32, (now * 10.0) as u32) < super_volatile_trigger
            {
                cell.glyph_index = Some(glyph_index.wrapping_add(13));
                cell.volatile_last = now;
                continue;
            }
            if cell.volatile_next <= 0.0 {
                cell.volatile_next =
                    now + (0.5 + hash01(row_index as u32, glyph_index) * 14.5) * interval_scale;
                continue;
            }
            if now >= cell.volatile_next {
                cell.glyph_index = Some(glyph_index.wrapping_add(5 + (hash01(glyph_index, row_index as u32) * 45.0) as u32));
                cell.volatile_last = now;
                cell.volatile_next =
                    now + (0.5 + hash01(glyph_index, row_index as u32) * 14.5) * interval_scale;
            }
        }
    }

    fn update_ghosts(
        column: &mut RainColumn,
        now: f32,
        head_row: i32,
        rows: u32,
        column_index: usize,
        ghost_swap_multiplier: f32,
    ) {
        let cutoff = 1.0;
        column.ghosts.retain_mut(|ghost| {
            let row = ghost.row.floor() as i32;
            if row < -1 || row > rows as i32 + 1 {
                return false;
            }
            if (ghost.row - head_row as f32).abs() <= cutoff {
                return false;
            }
            if now >= ghost.next_swap_at {
                ghost.glyph_index = ghost
                    .glyph_index
                    .wrapping_add(1 + (hash01(column_index as u32, row as u32) * 9.0) as u32);
                ghost.next_swap_at = now
                    + (0.33 + hash01(ghost.glyph_index, row as u32) * 4.67)
                        * ghost_swap_multiplier.max(1.0);
            }
            true
        });
    }
}
