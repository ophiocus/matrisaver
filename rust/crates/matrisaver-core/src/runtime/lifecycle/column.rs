// Per-column original lifecycle update and instance emission.
impl CoreRuntime {
    fn update_original_column(
        column: &mut RainColumn,
        column_index: usize,
        ctx: LifecycleTickContext,
        mutators: OriginalLifecycleMutators,
    ) {
        let effective_volatile_chance =
            (ctx.volatile_chance + mutators.volatile_chance_bias).clamp(0.0, 1.0);
        let effective_ghost_chance =
            (ctx.ghost_chance + mutators.ghost_chance_bias).clamp(0.0, 1.0);
        column.head_y += column.head_speed * ctx.frame_dt;
        column.eraser_y += column.eraser_speed * ctx.frame_dt;

        if column.eraser_y <= column.head_y {
            if column.eraser_y >= column.head_y - ctx.char_size as f32 {
                Self::reset_head(
                    column,
                    ctx.now,
                    ctx.rows,
                    ctx.char_size,
                    ctx.trail_length_multiplier,
                    mutators,
                );
                Self::reset_eraser(column, ctx.now, ctx.char_size, mutators);
            }
        } else if column.head_y >= column.eraser_y - ctx.char_size as f32 {
            Self::reset_eraser(column, ctx.now, ctx.char_size, mutators);
        }

        let limit = ctx.rows as f32 * ctx.char_size as f32;
        if column.eraser_y > limit + ctx.char_size as f32 * 2.0 {
            Self::reset_eraser(column, ctx.now, ctx.char_size, mutators);
        }
        if column.head_y > limit + ctx.char_size as f32 * 2.0 {
            Self::reset_head(
                column,
                ctx.now,
                ctx.rows,
                ctx.char_size,
                ctx.trail_length_multiplier,
                mutators,
            );
        }

        let head_row = (column.head_y / ctx.char_size as f32).floor() as i32;
        if head_row >= 0 && head_row < ctx.rows as i32 {
            if head_row != column.last_head_row {
                column.last_head_row = head_row;
                column.head_row_step = 1;
                column.glyph_cursor = column.glyph_cursor.wrapping_add(1);
                column.head_glyph_index =
                    Self::advance_head_glyph(column.head_glyph_index, ctx.now);
                let wrote_head_row = Self::write_head_row(
                    column,
                    head_row as usize,
                    ctx.now,
                    effective_volatile_chance,
                    column.head_glyph_index,
                );
                if wrote_head_row {
                    Self::maybe_spawn_ghost(
                        column,
                        head_row as usize,
                        ctx.now,
                        effective_ghost_chance,
                        ctx.ghost_swap_multiplier,
                    );
                }
            } else {
                let allow_triple_write =
                    hash01(column.column_slot, ctx.now.to_bits() ^ head_row as u32)
                        < mutators.extra_row_write_chance;
                let max_writes = if allow_triple_write { 3 } else { 2 };
                if column.head_row_step >= max_writes {
                    // fall through
                } else {
                    column.head_row_step += 1;
                    column.glyph_cursor = column.glyph_cursor.wrapping_add(1);
                    column.head_glyph_index =
                        Self::advance_head_glyph(column.head_glyph_index, ctx.now);
                    let _ = Self::write_head_row(
                        column,
                        head_row as usize,
                        ctx.now,
                        effective_volatile_chance,
                        column.head_glyph_index,
                    );
                }
            }
        }

        Self::update_volatile_cells(
            column,
            ctx.now,
            ctx.super_volatile_pulse_time,
            mutators.volatile_interval_scale,
            mutators.super_volatile_bonus,
        );

        let delete_row =
            ((column.head_y - column.delete_gap) / ctx.char_size as f32).floor() as i32;
        if delete_row >= 0 && delete_row < ctx.rows as i32 {
            Self::erase_row(column, delete_row as usize);
        }

        let eraser_row = (column.eraser_y / ctx.char_size as f32).floor() as i32;
        if eraser_row >= 0 && eraser_row < ctx.rows as i32 {
            if eraser_row != column.eraser_last_row {
                column.eraser_last_row = eraser_row;
            }
            Self::erase_row(column, eraser_row as usize);
        }

        Self::update_ghosts(
            column,
            ctx.now,
            head_row,
            ctx.rows,
            column_index,
            ctx.ghost_swap_multiplier,
        );
    }

    fn emit_original_instances(
        column: &mut RainColumn,
        char_size: u32,
        rows: u32,
        glyphs: &[renderer::AtlasGlyph],
        now: f32,
        ghost_swap_multiplier: f32,
        instances: &mut Vec<renderer::GlyphInstance>,
    ) {
        let column_pitch = Self::column_pitch(char_size);
        for (row_index, row_cell) in column.row_cells.iter().enumerate() {
            let Some(glyph_index) = row_cell.glyph_index else {
                continue;
            };
            let mut brightness = row_cell.brightness.clamp(0.0, 1.0);
            if brightness <= 0.0 {
                continue;
            }
            if row_cell.volatile {
                if row_cell.volatile_next > 0.0 {
                    let remaining = row_cell.volatile_next - now;
                    if remaining > 0.0 && remaining <= 1.0 {
                        let progress = 1.0 - remaining;
                        let pulse = 0.5 + 0.5 * ((now + row_index as f32 * 0.03) * 18.0).sin();
                        brightness += 0.25 * (0.7 * progress + 0.3 * pulse);
                    }
                }
                if row_cell.volatile_last > 0.0 {
                    let elapsed = now - row_cell.volatile_last;
                    if (0.0..=1.0).contains(&elapsed) {
                        brightness += (1.0 - elapsed) * 0.24;
                    }
                }
            }
            let recent_pulse = if row_cell.volatile_last > 0.0 {
                (1.0 - (now - row_cell.volatile_last)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let brightness = (brightness + recent_pulse * 0.28).clamp(0.0, 1.0);
            let glyph = glyphs[glyph_index as usize % glyphs.len()];
            let head_boost = brightness.powf(1.4).clamp(0.0, 1.0);
            let grain = hash01(column.column_slot, row_index as u32);
            let style_tag = if row_cell.volatile { 1.0 } else { 0.0 };
            let x = (column.column_slot as f32 + 0.5) * column_pitch;
            instances.push(renderer::GlyphInstance {
                position_size: [
                    x,
                    (row_index as f32 + 0.5) * char_size as f32,
                    char_size as f32,
                    char_size as f32,
                ],
                uv_rect: [glyph.u0, glyph.v0, glyph.u1, glyph.v1],
                params: [brightness, head_boost, grain, style_tag],
            });
        }

        for ghost in &mut column.ghosts {
            let row = ghost.row.floor() as i32;
            if row < 0 || row >= rows as i32 {
                continue;
            }
            if now >= ghost.next_swap_at {
                ghost.glyph_index = ghost.glyph_index.wrapping_add(1 + ((now * 37.0) as u32 % 7));
                ghost.next_swap_at = now + (0.33 + hash01(row as u32, ghost.glyph_index) * 4.67)
                    * ghost_swap_multiplier.max(1.0);
            }
            let glyph = glyphs[ghost.glyph_index as usize % glyphs.len()];
            let x = (column.column_slot as f32 + 0.5) * column_pitch;
            instances.push(renderer::GlyphInstance {
                position_size: [
                    x,
                    (row as f32 + 0.5) * char_size as f32,
                    char_size as f32,
                    char_size as f32,
                ],
                uv_rect: [glyph.u0, glyph.v0, glyph.u1, glyph.v1],
                params: [0.18, 0.03, 0.2, 2.0],
            });
        }
    }
}
