//! Monitor layout: one scene, N windows.
//!
//! Core renders a single scene at a single surface size. The Windows
//! `.scr` exploits that directly — one window spanning the virtual
//! screen, one surface, blit whole. A Wayland surface belongs to exactly
//! one output and cannot span, so the Linux host instead renders the
//! scene at the **union bounding box** of every participating output and
//! hands each per-output window the sub-rectangle it covers. The rain is
//! continuous across monitors because there is literally one scene; the
//! columns do not restart at a bezel.
//!
//! Everything in this module is pure arithmetic over output rectangles,
//! so the awkward cases (negative origins from a monitor placed left of
//! the primary, stacked outputs, mixed resolutions, a union too wide for
//! the GPU) are unit-tested without a compositor.

/// One output as the windowing system reports it, in desktop
/// coordinates. Origins may be negative: a monitor placed to the left of
/// the primary on X11 reports a negative `x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputRect {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

impl OutputRect {
    pub fn area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// An output plus where it sits inside the scene texture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedOutput {
    pub output: OutputRect,
    /// This output's top-left corner in scene pixels (always >= 0; the
    /// union origin has been subtracted out).
    pub scene_x: i64,
    pub scene_y: i64,
}

/// A rectangle in scene space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneRect {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneLayout {
    pub scene_width: u32,
    pub scene_height: u32,
    pub outputs: Vec<PlannedOutput>,
    /// The primary output in scene space. Pinned as the overlay
    /// reference rect so silhouettes compose against one screen instead
    /// of being stretched across the whole span — mirroring what the
    /// Windows host does with `SM_CXSCREEN`.
    pub primary: SceneRect,
    /// Set when the requested layout had to be demoted (see
    /// [`plan`]). Hosts log it as a `FALLBACK` line.
    pub demotion: Option<String>,
}

/// Why `plan` can refuse a layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// The windowing system reported no usable outputs.
    NoOutputs,
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOutputs => write!(
                formatter,
                "no usable outputs reported (every monitor had a zero-size rect)"
            ),
        }
    }
}

/// Plan the scene and the windows that show it.
///
/// `multi_monitor` false collapses to the primary output alone. A union
/// wider or taller than `max_dimension` (the GPU's max 2D texture size)
/// is demoted to primary-only rather than failing: four side-by-side 4K
/// panels are 15360 px across, which fits the usual 16384 limit, but a
/// fifth does not, and a black screensaver is a worse answer than a
/// single-monitor one.
pub fn plan(
    outputs: &[OutputRect],
    multi_monitor: bool,
    max_dimension: u32,
) -> Result<SceneLayout, LayoutError> {
    let usable: Vec<OutputRect> = outputs
        .iter()
        .filter(|output| output.width > 0 && output.height > 0)
        .cloned()
        .collect();
    if usable.is_empty() {
        return Err(LayoutError::NoOutputs);
    }

    let primary_index = pick_primary(&usable);

    if !multi_monitor {
        return Ok(single_output_layout(usable[primary_index].clone(), None));
    }

    let (scene_width, scene_height, min_x, min_y) = union_bounds(&usable);

    if scene_width > max_dimension || scene_height > max_dimension {
        let reason = format!(
            "spanned scene {scene_width}x{scene_height} exceeds max texture dimension \
             {max_dimension}; falling back to the primary output only"
        );
        return Ok(single_output_layout(
            usable[primary_index].clone(),
            Some(reason),
        ));
    }

    let planned: Vec<PlannedOutput> = usable
        .iter()
        .map(|output| PlannedOutput {
            scene_x: i64::from(output.x) - i64::from(min_x),
            scene_y: i64::from(output.y) - i64::from(min_y),
            output: output.clone(),
        })
        .collect();

    let primary_planned = &planned[primary_index];
    Ok(SceneLayout {
        scene_width,
        scene_height,
        primary: SceneRect {
            x: primary_planned.scene_x,
            y: primary_planned.scene_y,
            width: primary_planned.output.width,
            height: primary_planned.output.height,
        },
        outputs: planned,
        demotion: None,
    })
}

fn single_output_layout(output: OutputRect, demotion: Option<String>) -> SceneLayout {
    let scene_width = output.width;
    let scene_height = output.height;
    SceneLayout {
        scene_width,
        scene_height,
        primary: SceneRect {
            x: 0,
            y: 0,
            width: scene_width,
            height: scene_height,
        },
        outputs: vec![PlannedOutput {
            output,
            scene_x: 0,
            scene_y: 0,
        }],
        demotion,
    }
}

/// Union bounding box, returned as `(width, height, min_x, min_y)`.
fn union_bounds(outputs: &[OutputRect]) -> (u32, u32, i32, i32) {
    let min_x = outputs.iter().map(|output| output.x).min().unwrap_or(0);
    let min_y = outputs.iter().map(|output| output.y).min().unwrap_or(0);
    let max_x = outputs
        .iter()
        .map(|output| i64::from(output.x) + i64::from(output.width))
        .max()
        .unwrap_or(0);
    let max_y = outputs
        .iter()
        .map(|output| i64::from(output.y) + i64::from(output.height))
        .max()
        .unwrap_or(0);
    let width = (max_x - i64::from(min_x)).clamp(1, i64::from(u32::MAX)) as u32;
    let height = (max_y - i64::from(min_y)).clamp(1, i64::from(u32::MAX)) as u32;
    (width, height, min_x, min_y)
}

/// The output the compositor flagged primary; failing that, the largest;
/// failing that, the first. Wayland in particular often flags nothing.
fn pick_primary(outputs: &[OutputRect]) -> usize {
    if let Some(index) = outputs.iter().position(|output| output.primary) {
        return index;
    }
    outputs
        .iter()
        .enumerate()
        .max_by_key(|(_, output)| output.area())
        .map(|(index, _)| index)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{plan, LayoutError, OutputRect, SceneRect};

    const MAX_DIM: u32 = 16384;

    fn output(name: &str, x: i32, y: i32, width: u32, height: u32, primary: bool) -> OutputRect {
        OutputRect {
            name: name.to_owned(),
            x,
            y,
            width,
            height,
            primary,
        }
    }

    #[test]
    fn single_output_spans_the_whole_scene() {
        let outputs = vec![output("DP-1", 0, 0, 1920, 1080, true)];
        let layout = plan(&outputs, true, MAX_DIM).expect("layout");
        assert_eq!((layout.scene_width, layout.scene_height), (1920, 1080));
        assert_eq!(layout.outputs.len(), 1);
        assert_eq!(
            (layout.outputs[0].scene_x, layout.outputs[0].scene_y),
            (0, 0)
        );
        assert_eq!(
            layout.primary,
            SceneRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080
            }
        );
        assert!(layout.demotion.is_none());
    }

    #[test]
    fn side_by_side_outputs_share_one_wide_scene() {
        let outputs = vec![
            output("DP-1", 0, 0, 1920, 1080, true),
            output("HDMI-1", 1920, 0, 1920, 1080, false),
        ];
        let layout = plan(&outputs, true, MAX_DIM).expect("layout");
        assert_eq!((layout.scene_width, layout.scene_height), (3840, 1080));
        assert_eq!(
            (layout.outputs[0].scene_x, layout.outputs[0].scene_y),
            (0, 0)
        );
        assert_eq!(
            (layout.outputs[1].scene_x, layout.outputs[1].scene_y),
            (1920, 0)
        );
    }

    #[test]
    fn monitor_left_of_primary_shifts_the_origin() {
        // X11 reports a negative x for a monitor placed left of the
        // primary. Scene coordinates must come out non-negative.
        let outputs = vec![
            output("DP-1", 0, 0, 1920, 1080, true),
            output("DP-2", -2560, 0, 2560, 1440, false),
        ];
        let layout = plan(&outputs, true, MAX_DIM).expect("layout");
        assert_eq!((layout.scene_width, layout.scene_height), (4480, 1440));
        assert_eq!(
            (layout.outputs[0].scene_x, layout.outputs[0].scene_y),
            (2560, 0)
        );
        assert_eq!(
            (layout.outputs[1].scene_x, layout.outputs[1].scene_y),
            (0, 0)
        );
        assert_eq!(layout.primary.x, 2560);
    }

    #[test]
    fn stacked_outputs_share_one_tall_scene() {
        let outputs = vec![
            output("DP-1", 0, 0, 1920, 1080, true),
            output("DP-2", 0, 1080, 1920, 1080, false),
        ];
        let layout = plan(&outputs, true, MAX_DIM).expect("layout");
        assert_eq!((layout.scene_width, layout.scene_height), (1920, 2160));
        assert_eq!(
            (layout.outputs[1].scene_x, layout.outputs[1].scene_y),
            (0, 1080)
        );
    }

    #[test]
    fn single_monitor_setting_collapses_to_primary() {
        let outputs = vec![
            output("DP-1", 1920, 0, 1920, 1080, true),
            output("HDMI-1", 0, 0, 3840, 2160, false),
        ];
        let layout = plan(&outputs, false, MAX_DIM).expect("layout");
        assert_eq!((layout.scene_width, layout.scene_height), (1920, 1080));
        assert_eq!(layout.outputs.len(), 1);
        assert_eq!(layout.outputs[0].output.name, "DP-1");
        assert!(layout.demotion.is_none());
    }

    #[test]
    fn oversized_span_demotes_to_primary_with_a_reason() {
        let outputs = vec![
            output("DP-1", 0, 0, 3840, 2160, true),
            output("DP-2", 3840, 0, 3840, 2160, false),
            output("DP-3", 7680, 0, 3840, 2160, false),
        ];
        // 11520 across fits the real limit, so force a small one.
        let layout = plan(&outputs, true, 8192).expect("layout");
        assert_eq!((layout.scene_width, layout.scene_height), (3840, 2160));
        assert_eq!(layout.outputs.len(), 1);
        let reason = layout.demotion.expect("demotion reason");
        assert!(reason.contains("11520x2160"), "reason was: {reason}");
        assert!(reason.contains("8192"), "reason was: {reason}");
    }

    #[test]
    fn largest_output_wins_when_nothing_is_flagged_primary() {
        // Wayland commonly flags no primary at all.
        let outputs = vec![
            output("WL-1", 0, 0, 1280, 720, false),
            output("WL-2", 1280, 0, 2560, 1440, false),
        ];
        let layout = plan(&outputs, true, MAX_DIM).expect("layout");
        assert_eq!(layout.primary.x, 1280);
        assert_eq!(layout.primary.width, 2560);
    }

    #[test]
    fn zero_size_outputs_are_ignored() {
        let outputs = vec![
            output("GHOST", 0, 0, 0, 0, true),
            output("DP-1", 0, 0, 1920, 1080, false),
        ];
        let layout = plan(&outputs, true, MAX_DIM).expect("layout");
        assert_eq!(layout.outputs.len(), 1);
        assert_eq!(layout.outputs[0].output.name, "DP-1");
    }

    #[test]
    fn no_usable_outputs_is_an_error() {
        let outputs = vec![output("GHOST", 0, 0, 0, 0, true)];
        assert_eq!(plan(&outputs, true, MAX_DIM), Err(LayoutError::NoOutputs));
        assert_eq!(plan(&[], true, MAX_DIM), Err(LayoutError::NoOutputs));
    }
}
