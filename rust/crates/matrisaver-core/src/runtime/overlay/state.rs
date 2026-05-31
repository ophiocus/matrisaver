// Overlay state machine: trigger, lock management, and header advancement.
//
// Five phases, in order each frame:
//
//   1. Disabled: settings toggle off → tear down all overlay state,
//      reset next-trigger clock.
//   2. Active-hold (overlay_active_until set): post-injection window
//      where intro ghost glyphs are visible but painting heads have
//      not yet started moving. Just returns each frame.
//   3. Painting (overlay_headers non-empty): painting heads sweep
//      down their columns; advance_overlay_headers paints frozen
//      silhouette cells as they reach each target row.
//   4. Post-reveal hold (overlay_dissolve_at set): NEW in v0.3.2.
//      After all painting heads complete, the silhouette dwells
//      with cells still frozen for `settings.overlay_persist_seconds`
//      (admin-panel slider since v0.3.3, default 15s) so the user
//      can actually see the finished image. Previously cleared
//      locks the same frame the last head completed, so the
//      fully-revealed silhouette was visible for ~one frame.
//      When the dwell expires, `dissolve_overlay_into_rain` spawns
//      a fresh rain head at the topmost silhouette row in every
//      affected column, so the silhouette vanishes INTO rain
//      top-down rather than thawing in place and waiting for the
//      column's ambient head to eventually sweep past.
//   5. Idle: waiting for overlay_next_trigger to fire, then inject.
impl CoreRuntime {
    fn update_overlay_state(&mut self, now: f32, rows: u32, frame_dt: f32) {
        if !self.settings.overlay_enabled {
            self.clear_overlay_locks();
            self.overlay_active_until = None;
            self.overlay_dissolve_at = None;
            self.overlay_next_trigger = now + OVERLAY_INITIAL_TRIGGER_SECONDS;
            self.overlay_injected_count = 0;
            self.overlay_image_name = "none".to_owned();
            self.overlay_headers.clear();
            self.overlay_intro_glyphs.clear();
            return;
        }

        // Phase 4: post-reveal hold. Silhouette is fully painted and
        // dwelling. When the dwell window expires, dissolve INTO rain:
        // spawn a fresh head at the topmost silhouette row of every
        // affected column so the silhouette vanishes top-down as rain
        // washes over it, then schedule the next overlay.
        if let Some(dissolve_at) = self.overlay_dissolve_at {
            if now < dissolve_at {
                return;
            }
            self.overlay_dissolve_at = None;
            self.dissolve_overlay_into_rain();
            self.overlay_injected_count = 0;
            self.overlay_image_name = "none".to_owned();
            let (gap_min, gap_range) = self.overlay_trigger_gap();
            self.overlay_next_trigger =
                now + gap_min + hash01(self.frame_index as u32, 0x0F0F_4422) * gap_range;
            return;
        }

        if let Some(active_until) = self.overlay_active_until {
            if now < active_until {
                return;
            }
            self.overlay_active_until = None;
        }

        if !self.overlay_headers.is_empty() {
            if self.advance_overlay_headers(frame_dt) {
                // Don't clear_overlay_locks() here — the silhouette is
                // *just now* fully visible. Enter post-reveal hold
                // instead. clear_overlay_locks() will fire when
                // overlay_dissolve_at elapses.
                //
                // v0.3.3: dwell time is `Settings.overlay_persist_seconds`,
                // exposed as an admin-panel slider (range 0..120,
                // default 15s). `.max(0.0)` guards against pathological
                // negatives if a hand-edited settings.json sneaks past
                // sanitize() with a NaN-then-clamp edge case.
                self.overlay_dissolve_at =
                    Some(now + self.settings.overlay_persist_seconds.max(0.0));
                // Full bloom reached. If this overlay came from a custom
                // folder that opted into sidecars, schedule the
                // render-to-PNG grab a short settle later.
                if self.overlay_capture_source.is_some() {
                    self.overlay_capture_at_frame =
                        Some(self.frame_index.wrapping_add(OVERLAY_CAPTURE_SETTLE_FRAMES));
                }
            }
            return;
        }

        if now < self.overlay_next_trigger {
            return;
        }

        if !self.inject_overlay_from_image(rows) {
            let (gap_min, gap_range) = self.overlay_trigger_gap();
            self.overlay_next_trigger =
                now + gap_min + hash01(self.frame_index as u32, 0x0A0A_2929) * gap_range;
            return;
        }

        // The active-hold window is the "dim pre-show": it displays the
        // intro ghost layer as a dim full-silhouette preview before the
        // painting heads move. Variants with `overlay_skip_preview` skip
        // it entirely (active_until = None), so the very next frame enters
        // the painting phase and the silhouette is revealed only as the
        // heads paint it in.
        self.overlay_active_until = if self.runtime_config.overlay_skip_preview {
            None
        } else {
            Some(now + self.overlay_hold_seconds())
        };
    }

    fn clear_overlay_locks(&mut self) {
        for (column_index, row_index) in self.overlay_locked_cells.drain(..) {
            if let Some(column) = self.rain_columns.get_mut(column_index) {
                if let Some(cell) = column.row_cells.get_mut(row_index) {
                    cell.frozen = false;
                }
            }
        }
    }

    /// Dissolve the silhouette into rain at the end of the post-reveal
    /// hold (Phase 4 → Idle). For every column that holds locked
    /// silhouette cells, yank that column's rain head up to the topmost
    /// silhouette row, install a slow `outro_speed_override` that the
    /// lifecycle tick will use until the head crosses the silhouette's
    /// bottom edge, and thaw the locked cells. The lifecycle then
    /// writes fresh head + trail glyphs over the just-thawed silhouette
    /// cells at the slow Stage-3 speed, top to bottom — the figure
    /// ablates as a graceful wash instead of snapping out the moment
    /// the dwell expires.
    ///
    /// Field notes:
    ///
    ///   * `last_head_row = top_row - 1` so the next tick writes the
    ///     head glyph AT `top_row` instead of skipping it (the
    ///     lifecycle's "did we cross a new row?" check compares the
    ///     current row to `last_head_row`). For `top_row == 0` this
    ///     evaluates to `-1`, the standard "no previous head" sentinel
    ///     — handled by the i32 type.
    ///   * `outro_release_y = (bottom_row + 2) * char_size` so the
    ///     slowdown holds through the whole silhouette plus a two-row
    ///     buffer past the bottom edge, then auto-clears (in
    ///     `update_original_column`) so subsequent ambient rain is
    ///     full speed again.
    ///   * `head_reset_count` is bumped so the metric reflects the
    ///     forced reset.
    ///   * `clear_overlay_locks()` runs LAST so cells thaw AFTER the
    ///     head is repositioned — otherwise a column whose existing
    ///     head was sitting on a silhouette row would write a rain
    ///     glyph there on the same frame and the dissolve would start
    ///     one row too low.
    fn dissolve_overlay_into_rain(&mut self) {
        let char_size = self.settings.char_size.max(1) as f32;
        // (column_index) -> (topmost_row, bottommost_row)
        let mut span_by_column: std::collections::HashMap<usize, (usize, usize)> =
            std::collections::HashMap::with_capacity(self.overlay_locked_cells.len());
        for &(column_index, row_index) in &self.overlay_locked_cells {
            span_by_column
                .entry(column_index)
                .and_modify(|(top, bottom)| {
                    if row_index < *top {
                        *top = row_index;
                    }
                    if row_index > *bottom {
                        *bottom = row_index;
                    }
                })
                .or_insert((row_index, row_index));
        }
        for (column_index, (top_row, bottom_row)) in span_by_column {
            if let Some(column) = self.rain_columns.get_mut(column_index) {
                column.head_y = top_row as f32 * char_size;
                column.last_head_row = top_row as i32 - 1;
                column.head_reset_count = column.head_reset_count.saturating_add(1);
                let slow_speed = column.head_speed * OVERLAY_OUTRO_SPEED_MULTIPLIER;
                column.outro_speed_override = Some(slow_speed);
                column.outro_release_y = (bottom_row as f32 + 2.0) * char_size;
            }
        }
        self.clear_overlay_locks();
    }

    fn advance_overlay_headers(&mut self, frame_dt: f32) -> bool {
        let char_size = self.settings.char_size.max(1) as f32;
        let mut slot_to_column = std::collections::HashMap::with_capacity(self.rain_columns.len());
        for (index, column) in self.rain_columns.iter().enumerate() {
            slot_to_column.insert(column.column_slot, index);
        }
        let mut retired_intro_cells = std::collections::HashSet::new();

        let mut all_done = true;
        for header in &mut self.overlay_headers {
            header.y += header.speed * frame_dt;
            let reached_row = (header.y / char_size).floor() as i32;

            while header.next_target_index < header.targets.len()
                && header.targets[header.next_target_index].row_index as i32 <= reached_row
            {
                let target = &header.targets[header.next_target_index];
                if let Some(column_index) = slot_to_column.get(&header.column_slot).copied() {
                    if let Some(column) = self.rain_columns.get_mut(column_index) {
                        if let Some(cell) = column.row_cells.get_mut(target.row_index) {
                            cell.glyph_index = Some(target.glyph_index);
                            cell.brightness = target.brightness;
                            cell.overlay_color = target.color;
                            cell.volatile = false;
                            cell.volatile_next = 0.0;
                            cell.volatile_last = 0.0;
                            cell.super_volatile = false;
                            if !cell.frozen {
                                cell.frozen = true;
                                self.overlay_locked_cells.push((column_index, target.row_index));
                                self.overlay_injected_count = self.overlay_injected_count.saturating_add(1);
                            }
                            retired_intro_cells.insert((header.column_slot, target.row_index));
                        }
                    }
                }
                header.glyph_index = target.glyph_index;
                header.brightness = target.brightness;
                header.next_target_index += 1;
            }

            if header.next_target_index < header.targets.len() {
                all_done = false;
            } else if let Some(last) = header.targets.last() {
                if reached_row <= last.row_index as i32 {
                    all_done = false;
                }
            }
        }

        if !retired_intro_cells.is_empty() {
            self.overlay_intro_glyphs.retain(|glyph| {
                !retired_intro_cells.contains(&(glyph.column_slot, glyph.row_index))
            });
        }

        if all_done {
            self.overlay_headers.clear();
            self.overlay_intro_glyphs.clear();
            return true;
        }

        false
    }
}
