// Variant lifecycle mutator tables and style parameter computation.
impl CoreRuntime {
    fn original_lifecycle_mutators(&self) -> OriginalLifecycleMutators {
        match self.settings.variant.as_str() {
            "reloaded" => OriginalLifecycleMutators {
                head_speed_scale: 1.18,
                eraser_speed_scale: 1.08,
                eraser_offset_scale: 0.9,
                delete_gap_scale: 0.82,
                volatile_chance_bias: 0.04,
                ghost_chance_bias: 0.05,
                extra_row_write_chance: 0.22,
                volatile_interval_scale: 0.8,
                super_volatile_bonus: 0.1,
            },
            "revolutions" => OriginalLifecycleMutators {
                head_speed_scale: 0.92,
                eraser_speed_scale: 0.95,
                eraser_offset_scale: 1.15,
                delete_gap_scale: 1.2,
                volatile_chance_bias: 0.08,
                ghost_chance_bias: 0.07,
                extra_row_write_chance: 0.1,
                volatile_interval_scale: 0.65,
                super_volatile_bonus: 0.2,
            },
            "resurrections" => OriginalLifecycleMutators {
                head_speed_scale: 1.0,
                eraser_speed_scale: 0.9,
                eraser_offset_scale: 1.05,
                delete_gap_scale: 0.95,
                volatile_chance_bias: 0.03,
                ghost_chance_bias: 0.03,
                extra_row_write_chance: 0.15,
                volatile_interval_scale: 0.9,
                super_volatile_bonus: 0.05,
            },
            _ => OriginalLifecycleMutators::default(),
        }
    }

    fn variant_style_params(&self) -> [f32; 4] {
        let gamma = self.runtime_config.vfx_gamma.clamp(0.5, 2.2);
        let head_mix = (self.runtime_config.head_bloom / 2.4).clamp(0.5, 1.6);
        let ghost_alpha = (0.32 + self.runtime_config.ghost_chance * 0.9).clamp(0.25, 0.7);
        let volatile_pulse = (self.runtime_config.volatile_chance * 0.8
            + (self.runtime_config.vfx_glow_strength - 1.0).max(0.0) * 0.3)
            .clamp(0.0, 0.8);
        [gamma, head_mix, ghost_alpha, volatile_pulse]
    }
}
