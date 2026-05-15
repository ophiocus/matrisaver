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
        // dwelling. When the dwell window expires, clear locks (which
        // unfreezes the painted cells, returning them to normal rain)
        // and schedule the next overlay.
        if let Some(dissolve_at) = self.overlay_dissolve_at {
            if now < dissolve_at {
                return;
            }
            self.overlay_dissolve_at = None;
            self.clear_overlay_locks();
            self.overlay_injected_count = 0;
            self.overlay_image_name = "none".to_owned();
            self.overlay_next_trigger =
                now + OVERLAY_TRIGGER_MIN_SECONDS
                    + hash01(self.frame_index as u32, 0x0F0F_4422)
                        * OVERLAY_TRIGGER_RANGE_SECONDS;
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
            }
            return;
        }

        if now < self.overlay_next_trigger {
            return;
        }

        if !self.inject_overlay_from_image(rows) {
            self.overlay_next_trigger =
                now + OVERLAY_TRIGGER_MIN_SECONDS
                    + hash01(self.frame_index as u32, 0x0A0A_2929) * OVERLAY_TRIGGER_RANGE_SECONDS;
            return;
        }

        self.overlay_active_until = Some(now + OVERLAY_HOLD_SECONDS);
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
