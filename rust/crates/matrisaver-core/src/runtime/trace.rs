impl CoreRuntime {
    pub fn lifecycle_trace_line(&self) -> String {
        let char_size = self.settings.char_size.max(1) as f32;
        let rows = (self.surface_size.1.max(1) / self.settings.char_size.max(1) as u32).max(1) as i32;
        let mut active_heads = 0u32;
        let mut visible_cells = 0u32;
        let mut visible_chain_glyphs = 0u32;
        let mut min_lead_y = f32::INFINITY;
        let mut max_lead_y = f32::NEG_INFINITY;
        let mut sum_lead_y = 0.0f32;
        let mut lead_count = 0u32;
        let mut ghosts = 0u32;
        let mut head_resets = 0u64;
        let mut eraser_resets = 0u64;
        let mut head_writes = 0u64;
        let mut chain_resets = 0u64;
        let mut glyph_swaps = 0u64;
        let mut frozen_cells = 0u32;
        let height = self.surface_size.1.max(1) as f32;

        for column in &self.rain_columns {
            let head_row = (column.head_y / char_size).floor() as i32;
            if head_row >= 0 && head_row < rows {
                active_heads += 1;
            }
            if let Some(lead_y) = column.y_positions.first().copied() {
                min_lead_y = min_lead_y.min(lead_y);
                max_lead_y = max_lead_y.max(lead_y);
                sum_lead_y += lead_y;
                lead_count += 1;
            }
            visible_chain_glyphs += column
                .y_positions
                .iter()
                .filter(|y| **y >= 0.0 && **y < height)
                .count() as u32;
            visible_cells += column
                .row_cells
                .iter()
                .filter(|cell| cell.glyph_index.is_some() && cell.brightness > 0.0)
                .count() as u32;
            frozen_cells += column.row_cells.iter().filter(|cell| cell.frozen).count() as u32;
            ghosts += column.ghosts.len() as u32;
            head_resets += column.head_reset_count;
            eraser_resets += column.eraser_reset_count;
            head_writes += column.head_write_count;
            chain_resets += column.chain_reset_count;
            glyph_swaps += column.glyph_swap_count;
        }

        let avg_lead_y = if lead_count == 0 {
            0.0
        } else {
            sum_lead_y / lead_count as f32
        };
        let min_lead_y = if min_lead_y.is_finite() { min_lead_y } else { 0.0 };
        let max_lead_y = if max_lead_y.is_finite() { max_lead_y } else { 0.0 };
        let overlay_intro_glyphs = self.overlay_intro_glyphs.len();

        format!(
            "LIFECYCLE frame={} t={:.3} variant={} cols={} active_heads={} visible_cells={} visible_chain_glyphs={} ghosts={} head_resets={} eraser_resets={} head_writes={} chain_resets={} glyph_swaps={} overlay_active={} overlay_locked={} overlay_injected={} overlay_intro={} overlay_image={} lead_y_min={:.2} lead_y_avg={:.2} lead_y_max={:.2}",
            self.frame_index,
            self.animation_seconds,
            self.settings.variant,
            self.rain_columns.len(),
            active_heads,
            visible_cells,
            visible_chain_glyphs,
            ghosts,
            head_resets,
            eraser_resets,
            head_writes,
            chain_resets,
            glyph_swaps,
            self.overlay_active_until.is_some() || !self.overlay_headers.is_empty(),
            frozen_cells,
            self.overlay_injected_count,
            overlay_intro_glyphs,
            self.overlay_image_name,
            min_lead_y,
            avg_lead_y,
            max_lead_y
        )
    }

    pub fn exit_reason(&self) -> Option<ExitReason> {
        self.exit_reason
    }
}
