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

    /// Read one directory, append its image files (sorted, deduped by
    /// filename against `seen`) to `out`, tagging each with
    /// `write_ascii`. A missing/unreadable directory is a silent no-op.
    fn collect_overlay_dir(
        dir: &std::path::Path,
        write_ascii: bool,
        seen: &mut std::collections::HashSet<std::ffi::OsString>,
        out: &mut Vec<(std::path::PathBuf, bool)>,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
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
            out.push((path, write_ascii));
        }
    }

    /// The MSI installs the bundled overlay pack into
    /// `%ProgramData%\matrisaver\overlays\`. `ProgramData` is reliably
    /// set on Windows and absent elsewhere, so this naturally no-ops
    /// on Linux/macOS.
    fn programdata_overlays_dir() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("ProgramData")?;
        let dir = std::path::PathBuf::from(base)
            .join("matrisaver")
            .join("overlays");
        dir.is_dir().then_some(dir)
    }

    /// Resolved overlay image queue, each entry `(image_path,
    /// write_ascii_alongside)`, cycled by `overlay_image_cursor`.
    ///
    /// V2.2 per-variant queue model:
    ///
    ///   * Every variant leads with its own iconic-scene set under
    ///     `assets/overlays/<overlay_subdir>/` (or the ProgramData
    ///     equivalent). That's the queue when no folder is configured.
    ///   * `Settings.overlay_directories` (user folders) are collected
    ///     separately, then **interleaved** with the variant queue —
    ///     variant iconic first, then folder, then variant, then folder…
    ///     so the film's signature scene always opens the cycle and the
    ///     user's images are woven in.
    ///   * If both produced nothing (a variant with no populated subdir
    ///     and no user folders), fall back to the shared shipped pack
    ///     (`%ProgramData%\matrisaver\overlays\`) and then the legacy
    ///     `assets/overlays/` root, so a bare install still shows overlays.
    ///
    /// Dedup is by filename, first-occurrence-wins (so the variant copy
    /// of a same-named file shadows a folder copy).
    fn overlay_image_paths(&self) -> Vec<(std::path::PathBuf, bool)> {
        // 1. The variant's built-in iconic-scene queue (leads).
        let mut variant_queue = Vec::new();
        let mut variant_seen = std::collections::HashSet::<std::ffi::OsString>::new();
        if let Some(subdir) = self.runtime_config.overlay_subdir {
            for root in self.overlay_root_candidates() {
                let dir = root.join(subdir);
                if dir.is_dir() {
                    Self::collect_overlay_dir(&dir, false, &mut variant_seen, &mut variant_queue);
                }
            }
        }

        // 2. User-configured folders.
        let mut folder_queue = Vec::new();
        let mut folder_seen = std::collections::HashSet::<std::ffi::OsString>::new();
        for source in &self.settings.overlay_directories {
            if source.enabled {
                Self::collect_overlay_dir(
                    &source.path,
                    source.write_ascii_alongside,
                    &mut folder_seen,
                    &mut folder_queue,
                );
            }
        }

        // 3. Interleave (variant iconic first); dedup by filename.
        let combined = Self::interleave_overlay_queues(variant_queue, folder_queue);
        if !combined.is_empty() {
            return combined;
        }

        // 4. Fallback: shared shipped pack, then legacy root.
        let mut seen = std::collections::HashSet::<std::ffi::OsString>::new();
        let mut paths = Vec::new();
        if let Some(pack) = Self::programdata_overlays_dir() {
            Self::collect_overlay_dir(&pack, false, &mut seen, &mut paths);
        }
        if paths.is_empty() {
            if let Some(legacy) = self.resolve_overlay_directory() {
                Self::collect_overlay_dir(&legacy, false, &mut seen, &mut paths);
            }
        }
        paths
    }

    /// Interleave the variant iconic-scene queue with the user-folder
    /// queue, variant entry first at each index, deduping by filename
    /// (first occurrence wins). `[v0, v1]` + `[f0, f1, f2]` becomes
    /// `[v0, f0, v1, f1, f2]`.
    fn interleave_overlay_queues(
        variant: Vec<(std::path::PathBuf, bool)>,
        folder: Vec<(std::path::PathBuf, bool)>,
    ) -> Vec<(std::path::PathBuf, bool)> {
        let mut combined = Vec::with_capacity(variant.len() + folder.len());
        let mut seen = std::collections::HashSet::<std::ffi::OsString>::new();
        let max = variant.len().max(folder.len());
        for index in 0..max {
            for (path, write_ascii) in [variant.get(index), folder.get(index)].into_iter().flatten()
            {
                let key = path
                    .file_name()
                    .map(|name| name.to_os_string())
                    .unwrap_or_default();
                if seen.insert(key) {
                    combined.push((path.clone(), *write_ascii));
                }
            }
        }
        combined
    }

    /// Overlays-root candidates a variant-pinned subdirectory can live
    /// under: the ProgramData pack root (installed MSI) and the
    /// legacy-resolved root (dev / source tree). Both are joined with
    /// the variant subdir by `overlay_image_paths`.
    fn overlay_root_candidates(&self) -> Vec<std::path::PathBuf> {
        let mut roots = Vec::new();
        if let Some(pack) = Self::programdata_overlays_dir() {
            roots.push(pack);
        }
        if let Some(legacy) = self.resolve_overlay_directory() {
            roots.push(legacy);
        }
        roots
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

    /// If a full-bloom render-to-PNG sidecar grab is due this frame,
    /// return its target path (`<source-image>.overlay.png`, next to the
    /// source) and clear the one-shot. `None` when not due, no overlay
    /// source is armed, or the source directory isn't writable.
    fn take_due_overlay_capture(&mut self) -> Option<std::path::PathBuf> {
        let at = self.overlay_capture_at_frame?;
        if self.frame_index < at {
            return None;
        }
        self.overlay_capture_at_frame = None; // one-shot
        let source = self.overlay_capture_source.clone()?;
        let parent = source.parent()?.to_path_buf();
        if !self.probe_overlay_dir_writable(&parent) {
            return None;
        }
        let file_name = source.file_name()?.to_string_lossy().into_owned();
        Some(parent.join(format!("{file_name}.overlay.png")))
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
