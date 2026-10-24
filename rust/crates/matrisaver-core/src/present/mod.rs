//! Present passes: how a rendered scene reaches a host surface.
//!
//! Core renders the cascade to a single offscreen texture
//! (`CoreRuntime::gpu_scaffold_output_view`). Getting that texture onto
//! a window is the last mile, and it is *not* host-specific work — the
//! choice of present path is driven by the active variant's pipeline,
//! which is core's business. Both passes live here so every host shares
//! one implementation:
//!
//! * [`blit::SceneBlit`] — flat present. Draws a UV sub-rectangle of the
//!   scene, so a host can show the whole scene on one window (the
//!   Windows `.scr` spanning the virtual screen) or give each of several
//!   windows its own slice of one continuous scene (the Linux host's
//!   per-output windows, where Wayland forbids one surface spanning
//!   outputs).
//! * [`three_d::Present3d`] — the Reloaded-era 3D compositor, which
//!   wraps the scene onto animated geometry instead of blitting flat.
//!
//! Selection rule, unchanged from v0.3.7: `Pipeline::CodeStrips3D` takes
//! the 3D path, everything else blits flat.

pub mod blit;
pub mod three_d;

pub use blit::{SceneBlit, UvRect};
pub use three_d::Present3d;
