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

    /// Each entry is `(image_path, write_ascii_alongside)`. The second
    /// element rides along so the injector knows whether the source
    /// directory opted into the ASCII-alongside writer. If
    /// `Settings.overlay_directories` is non-empty, walk those in
    /// priority order (deduped by filename, first match wins).
    /// Otherwise fall back to the legacy single-directory resolution.
    fn overlay_image_paths(&self) -> Vec<(std::path::PathBuf, bool)> {
        let mut seen = std::collections::HashSet::<std::ffi::OsString>::new();
        let mut paths = Vec::new();

        if !self.settings.overlay_directories.is_empty() {
            for source in &self.settings.overlay_directories {
                if !source.enabled {
                    continue;
                }
                let Ok(entries) = std::fs::read_dir(&source.path) else {
                    continue;
                };
                let mut bucket: Vec<std::path::PathBuf> = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_file())
                    .filter(|path| {
                        path.extension().and_then(|v| v.to_str()).is_some_and(|ext| {
                            OVERLAY_IMAGE_EXTENSIONS
                                .iter()
                                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
                        })
                    })
                    .collect();
                bucket.sort();
                for path in bucket {
                    let key = path
                        .file_name()
                        .map(|n| n.to_os_string())
                        .unwrap_or_default();
                    if !seen.insert(key) {
                        continue;
                    }
                    paths.push((path, source.write_ascii_alongside));
                }
            }
            return paths;
        }

        // Legacy single-directory fallback (preserves behavior before
        // Settings.overlay_directories existed).
        let Some(overlay_dir) = self.resolve_overlay_directory() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(overlay_dir) else {
            return Vec::new();
        };
        let mut legacy_paths: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| {
                path.extension().and_then(|v| v.to_str()).is_some_and(|ext| {
                    OVERLAY_IMAGE_EXTENSIONS
                        .iter()
                        .any(|allowed| ext.eq_ignore_ascii_case(allowed))
                })
            })
            .collect();
        legacy_paths.sort();
        legacy_paths.into_iter().map(|p| (p, false)).collect()
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

    /// Idempotent per-session writability probe. Writes a zero-byte
    /// `.matrisaver-write-probe` file into `dir` and immediately
    /// removes it. Caches the result so subsequent calls are no-ops.
    /// Per the v0.2.0 contract: silent on failure, no retries.
    fn probe_overlay_dir_writable(&mut self, dir: &std::path::Path) -> bool {
        let key = dir.to_path_buf();
        if let Some(&cached) = self.overlay_dir_writable.get(&key) {
            return cached;
        }
        let probe = dir.join(".matrisaver-write-probe");
        let result = std::fs::write(&probe, b"").is_ok();
        if result {
            let _ = std::fs::remove_file(&probe);
        }
        self.overlay_dir_writable.insert(key, result);
        result
    }

    /// Side-effect snapshot of the rendered overlay grid as a text
    /// file living next to the source image. Filename is
    /// `<image>.<extension>.ascii.txt`. Silently no-ops on permission
    /// failure (probe-cached so repeated injections from a read-only
    /// directory don't keep retrying).
    fn write_overlay_ascii_alongside(
        &mut self,
        image_path: &std::path::Path,
        grid_text: &str,
    ) {
        let Some(parent) = image_path.parent() else {
            return;
        };
        if !self.probe_overlay_dir_writable(parent) {
            return;
        }
        let Some(stem) = image_path.file_name() else {
            return;
        };
        let ascii_name = format!("{}.ascii.txt", stem.to_string_lossy());
        let ascii_path = parent.join(ascii_name);
        // Best-effort: if write fails after a successful probe (e.g.
        // disk full, file locked), mark the directory unwritable so
        // we don't retry every overlay cycle.
        if std::fs::write(&ascii_path, grid_text).is_err() {
            self.overlay_dir_writable
                .insert(parent.to_path_buf(), false);
        }
    }

    /// Walk a sampled luminance grid and produce the same density-ramp
    /// glyph for each cell as the live renderer chooses — text-mode
    /// counterpart of overlay_glyph_index_for_luminance. Cells below
    /// the alpha cutoff render as spaces so the silhouette boundary
    /// is visually obvious in the .ascii.txt snapshot.
    fn render_overlay_grid_text(
        sampled_alpha: &[f32],
        sampled_luma: &[f32],
        cols: u32,
        rows: u32,
        alpha_cutoff: f32,
        auto_levels: Option<(f32, f32)>,
    ) -> String {
        let gradient = OVERLAY_DENSITY_GLYPHS.chars().collect::<Vec<_>>();
        if gradient.is_empty() {
            return String::new();
        }
        let mut out = String::with_capacity((cols as usize + 1) * rows as usize);
        for row in 0..rows {
            for col in 0..cols {
                let index = (row * cols + col) as usize;
                let alpha = sampled_alpha.get(index).copied().unwrap_or(0.0);
                if alpha < alpha_cutoff {
                    out.push(' ');
                    continue;
                }
                let raw = sampled_luma.get(index).copied().unwrap_or(0.0);
                let shaped = match auto_levels {
                    Some((low, high)) if high > low => {
                        ((raw - low) / (high - low)).clamp(0.0, 1.0)
                    }
                    _ => raw.clamp(0.0, 1.0),
                };
                let gradient_index = ((shaped * (gradient.len() - 1) as f32).round() as usize)
                    .min(gradient.len() - 1);
                out.push(gradient[gradient_index]);
            }
            out.push('\n');
        }
        out
    }
}
