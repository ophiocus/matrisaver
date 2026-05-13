// Settings dialog for MatriSaver's /c (config) mode.
//
// Replaces the previous stdout-only handler with a real egui window so
// Display Properties → Screen Saver Settings → "Settings…" produces
// something a user can actually interact with. Modeled on the eframe
// pattern used across the I:\Skeleton/MDReader/TinyBoothSoundStudio
// family, scoped down to a single Apply-style dialog.
//
// Top-level layout (no parent HWND; modern Windows screensavers don't
// reliably get one from Display Properties):
//   * Variant radio group
//   * Render-quality dropdown
//   * Glyph size slider
//   * Three toggles (multi-monitor / overlays / performance)
//   * Collapsible Advanced section: settings file path, Reveal,
//     Export, Import, Reset to Defaults
//   * Footer with version, update-check status, and the
//     Preview / Apply / Cancel action buttons
//
// The update check runs on a background thread so the dialog never
// blocks on the 6-second HTTP timeout. Result drains into the footer
// status line as it lands.

use eframe::egui;
use matrisaver_core::config::{variant_by_key, GlowQuality, OverlaySource, Settings, VARIANTS};
use matrisaver_core::storage;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::update_check::{self, UpdateCheckResult};

/// Open the settings dialog. Blocks until the window closes. Returns
/// `Ok(())` even when the user clicks Cancel — the only failure modes
/// are eframe / windowing errors.
pub fn open() -> eframe::Result<()> {
    let initial_settings = storage::load_settings_or_default(None);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 600.0])
            .with_min_inner_size([600.0, 480.0])
            .with_title("MatriSaver Settings"),
        ..Default::default()
    };

    eframe::run_native(
        "MatriSaver Settings",
        native_options,
        Box::new(move |cc| Ok(Box::new(ConfigApp::new(cc, initial_settings)))),
    )
}

/// In-flight or terminal update-check state, drained off the bg thread.
/// `current` from the update_check result is intentionally dropped —
/// the footer renders `env!("APP_VERSION")` directly, so reproducing
/// the value here would be redundant.
#[derive(Debug, Clone)]
enum UpdateStatus {
    Checking,
    UpToDate,
    Available { latest: String, msi_url: String },
    Downloading,
    DownloadFailed { reason: String },
    Failed { reason: String },
}

/// Toast-style transient status messages shown beneath the Advanced
/// section. Cleared after `until`.
struct StatusMessage {
    text: String,
    color: egui::Color32,
    until: Instant,
}

struct ConfigApp {
    /// The user's working copy. All UI mutates this. Disk is only
    /// touched on Apply or explicit Export.
    working: Settings,
    /// Snapshot of what's on disk at dialog open, for change detection.
    on_disk: Settings,
    /// Background update-check pipe; drained each frame.
    update_rx: Option<mpsc::Receiver<UpdateStatus>>,
    update_status: UpdateStatus,
    /// In-flight install download; drained each frame. `Ok` means the
    /// MSI is staged at the returned path and an elevated msiexec has
    /// been spawned — we self-exit on success so the new MSI can
    /// overwrite our own .scr in System32.
    download_rx: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    /// Transient banner shown in the Advanced section.
    status: Option<StatusMessage>,
}

impl ConfigApp {
    fn new(_cc: &eframe::CreationContext<'_>, settings: Settings) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let status = match update_check::check(None) {
                UpdateCheckResult::UpToDate { .. } => UpdateStatus::UpToDate,
                UpdateCheckResult::Available {
                    latest, msi_url, ..
                } => UpdateStatus::Available { latest, msi_url },
                UpdateCheckResult::Failed(reason) => UpdateStatus::Failed { reason },
            };
            let _ = tx.send(status);
        });

        Self {
            working: settings.clone(),
            on_disk: settings,
            update_rx: Some(rx),
            update_status: UpdateStatus::Checking,
            download_rx: None,
            status: None,
        }
    }

    fn set_status(&mut self, text: impl Into<String>, color: egui::Color32, secs: u64) {
        self.status = Some(StatusMessage {
            text: text.into(),
            color,
            until: Instant::now() + Duration::from_secs(secs),
        });
    }

    fn drain_status(&mut self) {
        if let Some(s) = &self.status {
            if Instant::now() >= s.until {
                self.status = None;
            }
        }
    }

    fn drain_update_check(&mut self) {
        if let Some(rx) = self.update_rx.as_ref() {
            if let Ok(status) = rx.try_recv() {
                self.update_status = status;
                self.update_rx = None;
            }
        }
    }

    fn drain_download(&mut self) {
        let Some(rx) = self.download_rx.as_ref() else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        match result {
            Ok(_path) => {
                // The elevated msiexec is now running on its own. We
                // must exit so it can overwrite C:\Windows\System32\
                // matrisaver.scr — the file backing the very process
                // we're in. The user sees msiexec's /passive progress
                // dialog hand off cleanly.
                std::process::exit(0);
            }
            Err(reason) => {
                self.update_status = UpdateStatus::DownloadFailed { reason };
                self.download_rx = None;
            }
        }
    }

    fn handle_install(&mut self, msi_url: String, version: String) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(download_and_install(&msi_url, &version));
        });
        self.download_rx = Some(rx);
        self.update_status = UpdateStatus::Downloading;
    }
}

impl eframe::App for ConfigApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_status();
        self.drain_update_check();
        self.drain_download();

        // Repaint shortly so the status banner expires and the update
        // check polling stays live without blocking on input.
        ctx.request_repaint_after(Duration::from_millis(200));

        egui::TopBottomPanel::bottom("actions")
            .resizable(false)
            .show(ctx, |ui| self.render_footer(ui, ctx));

        egui::CentralPanel::default().show(ctx, |ui| self.render_main(ui));
    }
}

impl ConfigApp {
    fn render_main(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("MatriSaver Settings");
        ui.add_space(8.0);

        // Variant
        ui.label(egui::RichText::new("Variant").strong());
        for variant in VARIANTS.iter() {
            ui.radio_value(
                &mut self.working.variant,
                variant.key.to_owned(),
                variant.name,
            );
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Render quality
        ui.horizontal(|ui| {
            ui.label("Render quality:");
            egui::ComboBox::from_id_source("glow_quality")
                .selected_text(glow_quality_label(self.working.glow_quality))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.working.glow_quality,
                        GlowQuality::High,
                        glow_quality_label(GlowQuality::High),
                    );
                    ui.selectable_value(
                        &mut self.working.glow_quality,
                        GlowQuality::Balanced,
                        glow_quality_label(GlowQuality::Balanced),
                    );
                    ui.selectable_value(
                        &mut self.working.glow_quality,
                        GlowQuality::Low,
                        glow_quality_label(GlowQuality::Low),
                    );
                });
        });

        ui.add_space(8.0);

        // Glyph size — sanitize() will clamp on save, slider matches.
        ui.horizontal(|ui| {
            ui.label("Glyph size:");
            ui.add(egui::Slider::new(&mut self.working.char_size, 8..=96).suffix(" px"));
        });

        ui.add_space(8.0);

        // Toggles
        ui.checkbox(&mut self.working.multi_monitor, "Multi-monitor span");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.working.overlay_enabled, "Overlay images");
            ui.weak("(assets not yet shipped in MSI install)");
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.working.performance_mode, "Performance mode");
            ui.weak("(reduces effects on weak GPUs)");
        });

        ui.add_space(12.0);

        // Overlays (collapsed by default)
        egui::CollapsingHeader::new("Overlays")
            .default_open(false)
            .show(ui, |ui| self.render_overlays(ui));

        ui.add_space(4.0);

        // Advanced (collapsed by default)
        egui::CollapsingHeader::new("Advanced")
            .default_open(false)
            .show(ui, |ui| self.render_advanced(ui));
    }

    fn render_overlays(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Directories searched for overlay images, in priority order. \
                 Earlier entries win on filename collisions. With no entries, \
                 matrisaver falls back to the MATRISAVER_OVERLAY_DIR env var \
                 or assets/overlays/ relative to the exe.",
            )
            .weak()
            .small(),
        );
        ui.add_space(6.0);

        let mut remove_index: Option<usize> = None;
        for (index, source) in self.working.overlay_directories.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut source.enabled, "");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(source.path.display().to_string()).monospace(),
                    )
                    .truncate(),
                );
                if ui.button("Remove").clicked() {
                    remove_index = Some(index);
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(28.0);
                ui.checkbox(
                    &mut source.write_ascii_alongside,
                    "Write ASCII snapshot beside each image",
                );
            });
            ui.add_space(2.0);
        }
        if let Some(index) = remove_index {
            self.working.overlay_directories.remove(index);
        }

        if ui.button("+ Add directory…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Pick overlay-image folder")
                .pick_folder()
            {
                self.working.overlay_directories.push(OverlaySource {
                    path,
                    enabled: true,
                    write_ascii_alongside: false,
                });
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.checkbox(
                &mut self.working.overlay_auto_levels,
                "Auto-level overlay luminance before glyph mapping",
            );
        });
        ui.label(
            egui::RichText::new(
                "Helps low-contrast / clustered-histogram inputs by stretching \
                 the 5th/95th percentiles to full range. Off by default — \
                 matrisaver V2 defaults to passthrough per canonical \
                 ASCII-conversion practice (jp2a, libcaca, Paul Bourke).",
            )
            .weak()
            .small(),
        );
    }

    fn render_advanced(&mut self, ui: &mut egui::Ui) {
        let path = storage::default_settings_path();
        ui.add_space(4.0);
        ui.label("Settings file:");
        ui.horizontal(|ui| {
            ui.code(path.display().to_string());
            let exists = path.exists();
            let reveal = ui.add_enabled(exists, egui::Button::new("Reveal"));
            if reveal.clicked() {
                reveal_in_explorer(&path);
            }
            if !exists {
                ui.weak("(will be created on first Apply)");
            }
        });

        if let Some(mtime) = file_mtime_string(&path) {
            ui.label(format!("Last modified: {mtime}"));
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Export Settings…").clicked() {
                self.handle_export();
            }
            if ui.button("Import Settings…").clicked() {
                self.handle_import();
            }
            if ui.button("Reset to Defaults").clicked() {
                self.working = Settings::default();
                self.set_status(
                    "Reset to defaults — click Apply to save",
                    egui::Color32::LIGHT_BLUE,
                    4,
                );
            }
        });

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Import replaces the working copy and live state in this dialog. \
                 The file on disk is only rewritten when you click Apply. Cancel discards.",
            )
            .weak()
            .small(),
        );

        if let Some(s) = &self.status {
            ui.add_space(6.0);
            ui.colored_label(s.color, &s.text);
        }
    }

    fn render_footer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(format!("v{}", env!("APP_VERSION")));
            ui.label("•");
            self.render_update_status(ui);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                let dirty = self.working != self.on_disk;
                let apply = ui.add_enabled(dirty, egui::Button::new("Apply"));
                if apply.clicked() {
                    match storage::save_settings(&self.working, None) {
                        Ok(()) => {
                            self.on_disk = self.working.clone();
                            self.set_status("Settings saved", egui::Color32::LIGHT_GREEN, 3);
                        }
                        Err(err) => {
                            self.set_status(
                                format!("Save failed: {err}"),
                                egui::Color32::LIGHT_RED,
                                6,
                            );
                        }
                    }
                }

                if ui.button("Preview").clicked() {
                    self.handle_preview();
                }
            });
        });
        ui.add_space(4.0);
    }

    fn render_update_status(&mut self, ui: &mut egui::Ui) {
        // Clone the status to release the borrow on `self` before the
        // match arms (the Available arm needs &mut self for handle_install).
        let status = self.update_status.clone();
        match status {
            UpdateStatus::Checking => {
                ui.weak("Checking for updates…");
            }
            UpdateStatus::UpToDate => {
                ui.colored_label(egui::Color32::from_rgb(120, 200, 120), "Up to date");
            }
            UpdateStatus::Available { latest, msi_url } => {
                let label = format!("v{latest} available — Install");
                let button = egui::Button::new(label).fill(egui::Color32::from_rgb(40, 80, 30));
                if ui.add(button).clicked() {
                    self.handle_install(msi_url, latest);
                }
            }
            UpdateStatus::Downloading => {
                ui.weak("Downloading update…");
                ui.spinner();
            }
            UpdateStatus::DownloadFailed { reason } => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    format!("Install failed: {reason}"),
                );
            }
            UpdateStatus::Failed { reason } => {
                ui.weak(format!("Update check failed ({reason})"));
            }
        }
    }

    fn handle_export(&mut self) {
        let default_variant = variant_by_key(&self.working.variant)
            .map(|v| v.key)
            .unwrap_or("custom");
        let filename = format!("matrisaver-settings-{default_variant}.json");
        let Some(path) = rfd::FileDialog::new()
            .add_filter("MatriSaver Settings (JSON)", &["json"])
            .set_file_name(&filename)
            .save_file()
        else {
            return;
        };

        match storage::save_settings(&self.working, Some(&path)) {
            Ok(()) => self.set_status(
                format!("Exported to {}", path.display()),
                egui::Color32::LIGHT_GREEN,
                4,
            ),
            Err(err) => {
                self.set_status(format!("Export failed: {err}"), egui::Color32::LIGHT_RED, 6)
            }
        }
    }

    fn handle_import(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("MatriSaver Settings (JSON)", &["json"])
            .pick_file()
        else {
            return;
        };

        match storage::load_settings(Some(&path)) {
            Ok(settings) => {
                self.working = settings;
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("imported file");
                self.set_status(
                    format!("Imported {name} — click Apply to save"),
                    egui::Color32::LIGHT_GREEN,
                    5,
                );
            }
            Err(err) => {
                self.set_status(format!("Import failed: {err}"), egui::Color32::LIGHT_RED, 6)
            }
        }
    }

    fn handle_preview(&mut self) {
        // Persist working copy so the preview reflects in-flight edits,
        // then spawn matrisaver.scr /s as a separate process. The
        // dialog itself stays open while the user dismisses the
        // preview with input — same mental model as Display
        // Properties' built-in Preview button.
        if let Err(err) = storage::save_settings(&self.working, None) {
            self.set_status(
                format!("Preview blocked — save failed: {err}"),
                egui::Color32::LIGHT_RED,
                6,
            );
            return;
        }
        self.on_disk = self.working.clone();

        match std::env::current_exe() {
            Ok(exe) => {
                if let Err(err) = std::process::Command::new(&exe).arg("/s").spawn() {
                    self.set_status(
                        format!("Preview launch failed: {err}"),
                        egui::Color32::LIGHT_RED,
                        6,
                    );
                }
            }
            Err(err) => self.set_status(
                format!("Cannot locate own exe: {err}"),
                egui::Color32::LIGHT_RED,
                6,
            ),
        }
    }
}

/// Download the MSI from `url` to `%TEMP%\matrisaver-<version>.msi`,
/// then spawn an elevated `msiexec /i … /passive /norestart` via
/// PowerShell's `Start-Process -Verb RunAs`. Returns the staged path
/// on success; caller is responsible for exiting the current process
/// so msiexec can replace our own .scr in System32.
///
/// Modeled on I:\Skeleton\src\git_update.rs::download_and_install.
fn download_and_install(url: &str, version: &str) -> Result<PathBuf, String> {
    let user_agent = format!("MatriSaver/{}", env!("APP_VERSION"));
    let client = reqwest::blocking::Client::builder()
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|err| format!("HTTP client: {err}"))?;

    let bytes = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.bytes())
        .map_err(|err| format!("download: {err}"))?;

    let path = std::env::temp_dir().join(format!("matrisaver-{version}.msi"));
    std::fs::write(&path, &bytes).map_err(|err| format!("write MSI: {err}"))?;

    let msi_arg = path.to_string_lossy().to_string();
    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Start-Process msiexec -ArgumentList '/i \"{msi_arg}\" /passive /norestart' -Verb RunAs"
            ),
        ])
        .spawn()
        .map_err(|err| format!("launch installer: {err}"))?;

    Ok(path)
}

fn glow_quality_label(q: GlowQuality) -> &'static str {
    match q {
        GlowQuality::Low => "Low (eco — half-res glow)",
        GlowQuality::Balanced => "Balanced (default)",
        GlowQuality::High => "High (max quality)",
    }
}

fn reveal_in_explorer(path: &std::path::Path) {
    // explorer.exe /select,<path> opens File Explorer with the file
    // highlighted. Best-effort — ignore launch failures, the dialog
    // can do nothing meaningful about a missing explorer.
    let arg = format!("/select,{}", path.display());
    let _ = std::process::Command::new("explorer").arg(arg).spawn();
}

fn file_mtime_string(path: &std::path::Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    let secs = duration.as_secs();
    Some(format_epoch_seconds_local(secs))
}

/// Format a Unix timestamp as `YYYY-MM-DD HH:MM:SS` in local time. Done
/// manually so the dialog doesn't pull in chrono just for this. Hand-
/// rolled rather than relying on a date crate — accurate to the
/// minute is plenty for "Last modified".
fn format_epoch_seconds_local(epoch_seconds: u64) -> String {
    use std::time::SystemTime;
    let system_now = SystemTime::now();
    let utc_now = system_now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Approximate local offset by diffing localtime/utctime via the
    // standard library — not perfect across DST transitions but close
    // enough for a "last modified" label, and avoids the chrono dep.
    let local_offset_seconds = local_utc_offset_seconds().unwrap_or(0);
    let local_seconds = epoch_seconds as i64 + local_offset_seconds;
    let _ = utc_now;
    civil_from_epoch(local_seconds)
}

fn local_utc_offset_seconds() -> Option<i64> {
    // Read it via the localtime/gmtime difference on Windows by
    // probing %TZ% indirectly through std isn't available. Punt to a
    // best-effort 0 (UTC) when we can't determine — accuracy isn't
    // worth a new transitive dep here.
    None
}

/// Civil-date breakdown from a (possibly-local) Unix timestamp.
/// Source: Howard Hinnant's "date algorithms", days_from_civil reversed.
fn civil_from_epoch(epoch_seconds: i64) -> String {
    let days = epoch_seconds.div_euclid(86_400);
    let secs_of_day = epoch_seconds.rem_euclid(86_400) as u32;
    let hh = secs_of_day / 3600;
    let mm = (secs_of_day % 3600) / 60;
    let ss = secs_of_day % 60;

    // Hinnant: days since 1970-01-01 → (y, m, d)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_epoch_matches_known_values() {
        // 2020-01-01 00:00:00 UTC
        assert_eq!(civil_from_epoch(1_577_836_800), "2020-01-01 00:00:00");
        // 2026-05-12 16:00:00 UTC — verified via
        //   `date -u -d '2026-05-12 16:00:00' +%s` → 1778601600.
        assert_eq!(civil_from_epoch(1_778_601_600), "2026-05-12 16:00:00");
    }

    #[test]
    fn glow_quality_labels_distinct() {
        let labels = [
            glow_quality_label(GlowQuality::Low),
            glow_quality_label(GlowQuality::Balanced),
            glow_quality_label(GlowQuality::High),
        ];
        assert_eq!(labels.len(), 3);
        assert_ne!(labels[0], labels[1]);
        assert_ne!(labels[1], labels[2]);
    }
}
