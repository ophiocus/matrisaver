// Overlay I/O: tuning config loading, image path resolution, and glyph lookup.
impl CoreRuntime {
    fn load_overlay_tuning(&self) -> OverlayTuning {
        let default = OverlayTuning::default();
        let Some(path) = self.resolve_overlay_tuning_path() else {
            return default;
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return default;
        };
        let Ok(config) = serde_json::from_str::<OverlayTuningConfig>(&raw) else {
            return default;
        };

        default.with_overrides(config)
    }

    fn resolve_overlay_tuning_path(&self) -> Option<std::path::PathBuf> {
        if let Ok(raw) = std::env::var("MATRISAVER_OVERLAY_TUNING_PATH") {
            let candidate = std::path::PathBuf::from(raw);
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        let overlay_dir = self.resolve_overlay_directory()?;
        let preferred = overlay_dir.join("overlay_tuning.json");
        if preferred.is_file() {
            return Some(preferred);
        }
        let compatibility = overlay_dir.join("overlay_config.json");
        if compatibility.is_file() {
            return Some(compatibility);
        }
        None
    }

    fn overlay_image_paths(&self) -> Vec<std::path::PathBuf> {
        let Some(overlay_dir) = self.resolve_overlay_directory() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(overlay_dir) else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            if OVERLAY_IMAGE_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
            {
                paths.push(path);
            }
        }
        paths.sort();
        paths
    }

    fn resolve_overlay_directory(&self) -> Option<std::path::PathBuf> {
        if let Ok(raw) = std::env::var("MATRISAVER_OVERLAY_DIR") {
            let candidate = std::path::PathBuf::from(raw);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }

        if let Ok(exe_path) = std::env::current_exe() {
            for parent in exe_path.ancestors() {
                let candidate = parent.join("assets").join("overlays");
                if candidate.is_dir() {
                    return Some(candidate);
                }
            }
        }

        if let Ok(cwd) = std::env::current_dir() {
            for parent in cwd.ancestors() {
                let candidate = parent.join("assets").join("overlays");
                if candidate.is_dir() {
                    return Some(candidate);
                }
            }
        }

        if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
            let candidate = std::path::Path::new(manifest_dir)
                .join("..")
                .join("..")
                .join("..")
                .join("assets")
                .join("overlays");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }

        None
    }

    fn overlay_glyph_lookup(&self) -> Vec<(char, u32)> {
        let mut lookup = Vec::new();
        for (index, glyph) in self.atlas.glyphs.iter().enumerate() {
            lookup.push((glyph.glyph, index as u32));
        }
        lookup
    }
}
