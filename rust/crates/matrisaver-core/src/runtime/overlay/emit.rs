// Overlay instance emission: intro glyphs and falling header glyphs.
impl CoreRuntime {
    fn emit_overlay_intro_instances(
        &self,
        instances: &mut Vec<renderer::GlyphInstance>,
        glyphs: &[renderer::AtlasGlyph],
        surface_height: f32,
        char_size: u32,
    ) {
        if self.overlay_intro_glyphs.is_empty() || self.overlay_headers.is_empty() {
            return;
        }

        let column_pitch = Self::column_pitch(char_size);
        let intro_size = (char_size as f32 * self.overlay_tuning.intro_glyph_scale).max(1.0);
        let half_intro_size = intro_size * 0.5;
        for glyph in &self.overlay_intro_glyphs {
            let y = (glyph.row_index as f32 + 0.5) * char_size as f32;
            if y < -half_intro_size || y >= surface_height + half_intro_size {
                continue;
            }
            let atlas_glyph = glyphs[glyph.glyph_index as usize % glyphs.len()];
            let x = (glyph.column_slot as f32 + 0.5) * column_pitch + glyph.x_offset;
            instances.push(renderer::GlyphInstance {
                position_size: [x, y, intro_size, intro_size],
                uv_rect: [atlas_glyph.u0, atlas_glyph.v0, atlas_glyph.u1, atlas_glyph.v1],
                params: [
                    glyph.brightness,
                    0.45,
                    hash01(glyph.column_slot, (glyph.row_index as u32) ^ self.frame_index as u32),
                    0.0,
                ],
            });
        }
    }

    fn emit_overlay_header_instances(
        &self,
        instances: &mut Vec<renderer::GlyphInstance>,
        glyphs: &[renderer::AtlasGlyph],
        surface_height: f32,
        char_size: u32,
    ) {
        if self.overlay_active_until.is_some() {
            return;
        }
        let column_pitch = Self::column_pitch(char_size);
        for header in &self.overlay_headers {
            if header.next_target_index >= header.targets.len() {
                continue;
            }
            let y = header.y;
            if y < -(char_size as f32) || y >= surface_height + char_size as f32 {
                continue;
            }
            let glyph = glyphs[header.glyph_index as usize % glyphs.len()];
            let brightness =
                (header.brightness * self.overlay_tuning.header_brightness_scale).clamp(0.0, 1.0);
            let x = (header.column_slot as f32 + 0.5) * column_pitch;
            instances.push(renderer::GlyphInstance {
                position_size: [x, y + char_size as f32 * 0.5, char_size as f32, char_size as f32],
                uv_rect: [glyph.u0, glyph.v0, glyph.u1, glyph.v1],
                params: [brightness, 1.0, hash01(header.column_slot, self.frame_index as u32), 0.0],
            });
        }
    }
}
