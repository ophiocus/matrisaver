//! Flat present pass with a UV window.
//!
//! Lifted out of the Windows host (where it was an inline fullscreen-
//! triangle blit) so every host shares one copy, and extended with a UV
//! rectangle so a single rendered scene can be split across several
//! surfaces.
//!
//! Why the UV window exists: core renders **one** scene at **one**
//! surface size. The Windows `.scr` can create a single window spanning
//! the whole virtual screen, so it blits the scene whole
//! ([`UvRect::FULL`]). A Wayland surface, by contrast, belongs to
//! exactly one output — spanning is not expressible — so the Linux host
//! renders the scene at the union bounding box of every output and gives
//! each per-output window the sub-rectangle it covers. The rain stays
//! continuous across monitors because it is literally one scene.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// The slice of the scene texture a surface should show, in normalized
/// texture coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
}

impl UvRect {
    /// The whole scene — what a single-window host wants.
    pub const FULL: Self = Self {
        offset: [0.0, 0.0],
        scale: [1.0, 1.0],
    };

    /// The sub-rectangle `(x, y, width, height)` of a `scene_width` x
    /// `scene_height` scene, in pixels.
    ///
    /// Degenerate inputs (zero scene extent, zero sub-rect extent) fall
    /// back to [`UvRect::FULL`] rather than emitting NaNs into the
    /// shader: a monitor that reports a zero-size rect should show
    /// *something*, not a black window.
    pub fn from_sub_rect(
        x: i64,
        y: i64,
        width: u32,
        height: u32,
        scene_width: u32,
        scene_height: u32,
    ) -> Self {
        if scene_width == 0 || scene_height == 0 || width == 0 || height == 0 {
            return Self::FULL;
        }
        let scene_w = scene_width as f32;
        let scene_h = scene_height as f32;
        Self {
            offset: [x as f32 / scene_w, y as f32 / scene_h],
            scale: [width as f32 / scene_w, height as f32 / scene_h],
        }
    }
}

impl Default for UvRect {
    fn default() -> Self {
        Self::FULL
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BlitUniform {
    uv_offset: [f32; 2],
    uv_scale: [f32; 2],
}

/// Flat scene-to-surface present pass.
///
/// One instance per surface: each owns its uniform buffer, and the
/// pipeline is built against that surface's format.
pub struct SceneBlit {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    uv_rect: UvRect,
}

impl SceneBlit {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self::with_uv_rect(device, format, UvRect::FULL)
    }

    pub fn with_uv_rect(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        uv_rect: UvRect,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matrisaver-scene-blit-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matrisaver-scene-blit-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("matrisaver-scene-blit-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrisaver-scene-blit-uniform"),
            contents: bytemuck::bytes_of(&BlitUniform {
                uv_offset: uv_rect.offset,
                uv_scale: uv_rect.scale,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matrisaver-scene-blit-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("matrisaver-scene-blit-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform,
            uv_rect,
        }
    }

    pub fn uv_rect(&self) -> UvRect {
        self.uv_rect
    }

    /// Repoint this surface at a different slice of the scene. Cheap —
    /// a 16-byte uniform write, no pipeline rebuild — so hosts call it
    /// freely when outputs move or the scene is resized.
    pub fn set_uv_rect(&mut self, queue: &wgpu::Queue, uv_rect: UvRect) {
        if uv_rect == self.uv_rect {
            return;
        }
        self.uv_rect = uv_rect;
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&BlitUniform {
                uv_offset: uv_rect.offset,
                uv_scale: uv_rect.scale,
            }),
        );
    }

    /// Record the blit into `encoder`, sampling `scene_view` onto
    /// `target_view`.
    pub fn record_pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        scene_view: &wgpu::TextureView,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scene-blit-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("matrisaver-scene-blit-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.01,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

// Fullscreen-triangle blit. The oversized triangle covers the clip
// volume with three vertices (no index buffer, no vertex buffer); the
// UV window is applied in the vertex stage so the fragment stage stays
// a single texture sample.
const SHADER: &str = "
struct BlitUniform {
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,
};

@group(0) @binding(0)
var scene_tex: texture_2d<f32>;

@group(0) @binding(1)
var scene_sampler: sampler;

@group(0) @binding(2)
var<uniform> blit: BlitUniform;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0)
    );
    let pos = positions[idx];
    var out: VsOut;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    let base_uv = pos * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    out.uv = base_uv * blit.uv_scale + blit.uv_offset;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(scene_tex, scene_sampler, in.uv).rgb;
    return vec4<f32>(color, 1.0);
}
";

#[cfg(test)]
mod tests {
    use super::{UvRect, SHADER};

    #[test]
    fn full_rect_covers_whole_scene() {
        assert_eq!(UvRect::FULL.offset, [0.0, 0.0]);
        assert_eq!(UvRect::FULL.scale, [1.0, 1.0]);
        assert_eq!(UvRect::default(), UvRect::FULL);
    }

    #[test]
    fn sub_rect_maps_right_half_of_scene() {
        let rect = UvRect::from_sub_rect(1920, 0, 1920, 1080, 3840, 1080);
        assert_eq!(rect.offset, [0.5, 0.0]);
        assert_eq!(rect.scale, [0.5, 1.0]);
    }

    #[test]
    fn sub_rect_maps_stacked_output() {
        let rect = UvRect::from_sub_rect(0, 1080, 1920, 1080, 1920, 2160);
        assert_eq!(rect.offset, [0.0, 0.5]);
        assert_eq!(rect.scale, [1.0, 0.5]);
    }

    #[test]
    fn degenerate_extents_fall_back_to_full() {
        assert_eq!(
            UvRect::from_sub_rect(0, 0, 1920, 1080, 0, 1080),
            UvRect::FULL
        );
        assert_eq!(
            UvRect::from_sub_rect(0, 0, 0, 1080, 1920, 1080),
            UvRect::FULL
        );
    }

    #[test]
    fn shader_parses_as_wgsl() {
        naga::front::wgsl::parse_str(SHADER).expect("scene blit shader must parse");
    }
}
