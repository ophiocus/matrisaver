//! Reloaded-era 3D present pass.
//!
//! The 2D code cascade renders offscreen exactly as it does for the
//! flat present path — full lifecycle, glyph swap, bloom, overlays,
//! all of it. This module takes that offscreen texture and, instead
//! of blitting it flat to the surface, wraps it onto a slowly
//! rotating cylinder of vertical "code strips" seen from the inside.
//!
//! Reference: THE MATRIX RELOADED (2003) — Trinity-falls dream and
//! Neo's code-vision cutscenes. The matrix code isn't a flat rain in
//! those scenes; it drapes over 3D forms and passes through space.
//! This pipeline is the geometric substrate for that look. What runs
//! *on* the strips is still the 2D cascade tuned by the variant's
//! other knobs (colour, speed, glow, ghost swap, etc.).

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const STRIP_COUNT: u32 = 32;
// Tighter radius so cylinder curvature is legible at the frustum
// edges — with the camera at origin and the wall at 1.8 units, the
// strips at ±FOV/2 are ~30% narrower than the head-on strip, which
// the eye reads as "wrapping in 3D" instead of "flat vertical rain."
const STRIP_RADIUS: f32 = 1.8;
const STRIP_HEIGHT: f32 = 3.6;
const FOV_Y_RAD: f32 = 1.4; // ~80°, wider so more of the cylinder shows
const NEAR: f32 = 0.05;
// FAR needs to comfortably cover the tube (camera at Z=+6 looking
// down -Z toward strips at Z=-6 puts far walls at view distance 12)
// plus the disc + shatter shards. 20 gives everything headroom
// without meaningfully affecting depth precision on modern GPUs.
const FAR: f32 = 20.0;
// A hair faster than the first draft so the "flying through strips"
// motion is unmistakable within a couple of screen-refresh seconds.
const CAMERA_ROT_RAD_PER_SEC: f32 = 0.35;
// Flat-plane arrangement width. Arc-length of the cylinder wall
// (2π·R) — laid flat, the strips form a continuous plane exactly as
// wide as the cylinder is around. Cameras see it as a flat wall of
// code at Z=-R in flat mode.
const FLAT_WIDTH: f32 = std::f32::consts::TAU * STRIP_RADIUS;

// Tube pose: same wrap-around ring but the axis points down -Z
// instead of up +Y. Camera at origin looks down the barrel; the
// strips stretch away toward the vanishing point.
const TUBE_RADIUS: f32 = 1.2;
const TUBE_LEN: f32 = 12.0;

// Pose indices used by the shader to pick which vertex-position
// attribute to sample. Kept in a `pose_pair: vec2<u32>` uniform;
// morph_t blends between the two selected poses each frame.
const POSE_FLAT: u32 = 0;
const POSE_CYL: u32 = 1;
const POSE_TUBE: u32 = 2;
const POSE_SHATTER: u32 = 3;

// Clock-face disc appended to the tube geometry. Reproduces the
// Matrix Reloaded inception-scene move: as the camera advances,
// code assembles into a recognisable circular shape ahead of the
// viewer. `position_tube` places the disc partway down the tube
// (not at the far end — pushing it to the far plane collapses it
// against the fog floor and it reads as a dark occluder instead
// of a floating code-face). Other-pose positions collapse to origin
// so the disc is only visible while the tube pose is in the blend.
const DISC_RIM_SEGMENTS: u32 = 48;
const DISC_RADIUS: f32 = 1.55;
const DISC_Z: f32 = -3.2;

// Shatter pose: 4 flat "shards" of code floating at various angles
// through 3D space. Matches the dream sequence's "code from multiple
// simultaneous angles" fragments. Strips are distributed among the
// shards deterministically (8 strips per shard × 4 shards = 32).
const SHATTER_PANES: u32 = 4;
const STRIPS_PER_PANE: u32 = STRIP_COUNT / SHATTER_PANES;
const SHATTER_PANE_HALFW: f32 = 1.4; // pane half-width in local space
const SHATTER_PANE_HALFH: f32 = 1.1;

// Full pose cycle: flat → cyl → tube → shatter → flat. Each pose
// gets a hold, then a ramp to the next.
const HOLD_SECS: f32 = 4.0;
const RAMP_SECS: f32 = 2.0;
const CYCLE_SECS: f32 = 4.0 * HOLD_SECS + 4.0 * RAMP_SECS;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct StripVertex {
    /// Flat-plane position (2D-imitation pose at Z=-R).
    position_flat: [f32; 3],
    /// Cylinder-wall position (axis along +Y, wraps around camera).
    position_cyl: [f32; 3],
    /// Tube position (axis along -Z, camera looks down the barrel).
    position_tube: [f32; 3],
    /// Shatter pose: strip lives on one of N tilted panes floating
    /// through 3D space.
    position_shatter: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    /// (t_secs, morph_t [0..1], 0, 0) — vec4 for std140 alignment.
    /// morph_t interpolates between the two poses named by pose_pair.
    time_morph: [f32; 4],
    /// (pose_a, pose_b, 0, 0). Each is POSE_FLAT/POSE_CYL/POSE_TUBE.
    pose_pair: [u32; 4],
}

pub struct Present3d {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    sampler: wgpu::Sampler,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    aspect: f32,
}

impl Present3d {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        initial_size: (u32, u32),
    ) -> Self {
        let (vertices, indices) = build_strip_geometry();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrisaver-present-3d-vertex-buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrisaver-present-3d-index-buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("matrisaver-present-3d-uniform-buffer"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("matrisaver-present-3d-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matrisaver-present-3d-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matrisaver-present-3d-shader"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matrisaver-present-3d-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("matrisaver-present-3d-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<StripVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // location 0: flat-plane position (POSE_FLAT=0)
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        // location 1: cylinder-wall position (POSE_CYL=1)
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        // location 2: tube position (POSE_TUBE=2)
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 24,
                            shader_location: 2,
                        },
                        // location 3: shatter position (POSE_SHATTER=3)
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 36,
                            shader_location: 3,
                        },
                        // location 4: uv
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 48,
                            shader_location: 4,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // strips are seen from inside — no back-culling
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let (depth_view, depth_size) = create_depth(device, initial_size);
        let aspect = aspect_for(initial_size);

        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            sampler,
            depth_view,
            depth_size,
            aspect,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let size = (width.max(1), height.max(1));
        if size != self.depth_size {
            let (view, actual) = create_depth(device, size);
            self.depth_view = view;
            self.depth_size = actual;
        }
        self.aspect = aspect_for(size);
    }

    pub fn record_pass(
        &self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        scene_view: &wgpu::TextureView,
        elapsed_secs: f32,
    ) {
        let frame = compute_pose_frame(elapsed_secs);
        let view_proj = compute_view_proj(elapsed_secs, self.aspect, &frame);
        let uniform = SceneUniform {
            view_proj,
            time_morph: [elapsed_secs, frame.t, 0.0, 0.0],
            pose_pair: [frame.pose_a, frame.pose_b, 0, 0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-present-3d-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("matrisaver-present-3d-pass"),
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
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn build_strip_geometry() -> (Vec<StripVertex>, Vec<u32>) {
    let mut vertices =
        Vec::with_capacity(STRIP_COUNT as usize * 4 + DISC_RIM_SEGMENTS as usize + 1);
    let mut indices = Vec::with_capacity(STRIP_COUNT as usize * 6 + DISC_RIM_SEGMENTS as usize * 3);
    let two_pi = std::f32::consts::TAU;

    let y0 = -STRIP_HEIGHT * 0.5;
    let y1 = STRIP_HEIGHT * 0.5;
    let tube_z_far = -TUBE_LEN * 0.5;
    let tube_z_near = TUBE_LEN * 0.5;

    // Precompute shatter-pane transforms — deterministic so the poses
    // are consistent every run. Each pane is (center, rotation_matrix).
    let panes = shatter_pane_transforms();

    for i in 0..STRIP_COUNT {
        let t0 = i as f32 / STRIP_COUNT as f32;
        let t1 = (i + 1) as f32 / STRIP_COUNT as f32;

        // Cylinder pose.
        let theta0 = t0 * two_pi;
        let theta1 = t1 * two_pi;
        let (s0, c0) = theta0.sin_cos();
        let (s1, c1) = theta1.sin_cos();
        let cx0 = STRIP_RADIUS * c0;
        let cz0 = STRIP_RADIUS * s0;
        let cx1 = STRIP_RADIUS * c1;
        let cz1 = STRIP_RADIUS * s1;

        // Flat pose.
        let fx0 = (t0 - 0.5) * FLAT_WIDTH;
        let fx1 = (t1 - 0.5) * FLAT_WIDTH;
        let fz = -STRIP_RADIUS;

        // Tube pose.
        let tx0 = TUBE_RADIUS * c0;
        let ty0 = TUBE_RADIUS * s0;
        let tx1 = TUBE_RADIUS * c1;
        let ty1 = TUBE_RADIUS * s1;

        // Shatter pose: strip belongs to pane (i / STRIPS_PER_PANE).
        // Within its pane, it occupies a horizontal slice from
        // local_frac_0 to local_frac_1 across the pane's face.
        let pane_index = (i / STRIPS_PER_PANE) as usize;
        let strip_in_pane = i % STRIPS_PER_PANE;
        let local_frac_0 = strip_in_pane as f32 / STRIPS_PER_PANE as f32;
        let local_frac_1 = (strip_in_pane + 1) as f32 / STRIPS_PER_PANE as f32;
        let lx0 = (local_frac_0 - 0.5) * 2.0 * SHATTER_PANE_HALFW;
        let lx1 = (local_frac_1 - 0.5) * 2.0 * SHATTER_PANE_HALFW;
        let ly0 = -SHATTER_PANE_HALFH;
        let ly1 = SHATTER_PANE_HALFH;
        let (center, rot) = panes[pane_index];
        let sh_bl = add3(center, mat3_mul_vec3(rot, [lx0, ly0, 0.0]));
        let sh_br = add3(center, mat3_mul_vec3(rot, [lx1, ly0, 0.0]));
        let sh_tr = add3(center, mat3_mul_vec3(rot, [lx1, ly1, 0.0]));
        let sh_tl = add3(center, mat3_mul_vec3(rot, [lx0, ly1, 0.0]));

        let base = vertices.len() as u32;
        vertices.push(StripVertex {
            position_flat: [fx0, y0, fz],
            position_cyl: [cx0, y0, cz0],
            position_tube: [tx0, ty0, tube_z_far],
            position_shatter: sh_bl,
            uv: [t0, 1.0],
        });
        vertices.push(StripVertex {
            position_flat: [fx1, y0, fz],
            position_cyl: [cx1, y0, cz1],
            position_tube: [tx1, ty1, tube_z_far],
            position_shatter: sh_br,
            uv: [t1, 1.0],
        });
        vertices.push(StripVertex {
            position_flat: [fx1, y1, fz],
            position_cyl: [cx1, y1, cz1],
            position_tube: [tx1, ty1, tube_z_near],
            position_shatter: sh_tr,
            uv: [t1, 0.0],
        });
        vertices.push(StripVertex {
            position_flat: [fx0, y1, fz],
            position_cyl: [cx0, y1, cz0],
            position_tube: [tx0, ty0, tube_z_near],
            position_shatter: sh_tl,
            uv: [t0, 0.0],
        });
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    // Clock-face disc, floating partway down the tube. Non-tube
    // poses collapse it to the origin so it's a zero-area
    // (invisible) fan. The morph in/out of the tube pose grows the
    // disc from a point at origin into a full disc — a literal
    // iris-open effect that reads as "code assembling into a shape".
    // Sitting at DISC_Z (not the far plane) keeps it in a bright fog
    // region so it reads as a floating luminous face rather than a
    // dark occluder against the vanishing point.
    let disc_center_idx = vertices.len() as u32;
    vertices.push(StripVertex {
        position_flat: [0.0, 0.0, 0.0],
        position_cyl: [0.0, 0.0, 0.0],
        position_tube: [0.0, 0.0, DISC_Z],
        position_shatter: [0.0, 0.0, 0.0],
        uv: [0.5, 0.5],
    });
    for j in 0..DISC_RIM_SEGMENTS {
        let a = (j as f32 / DISC_RIM_SEGMENTS as f32) * two_pi;
        let (sa, ca) = a.sin_cos();
        vertices.push(StripVertex {
            position_flat: [0.0, 0.0, 0.0],
            position_cyl: [0.0, 0.0, 0.0],
            position_tube: [DISC_RADIUS * ca, DISC_RADIUS * sa, DISC_Z],
            position_shatter: [0.0, 0.0, 0.0],
            uv: [0.5 + 0.5 * ca, 0.5 - 0.5 * sa],
        });
    }
    for j in 0..DISC_RIM_SEGMENTS {
        let rim_a = disc_center_idx + 1 + j;
        let rim_b = disc_center_idx + 1 + ((j + 1) % DISC_RIM_SEGMENTS);
        indices.extend_from_slice(&[disc_center_idx, rim_a, rim_b]);
    }

    (vertices, indices)
}

/// Deterministic transforms for the shatter-pose panes. Each pane
/// gets a translation + a rotation matrix around its own local
/// origin. Chosen by hand to give the "fractured mirror of code"
/// look — panes at different depths, some tilted horizontally,
/// one tilted overhead, one leaning back.
fn shatter_pane_transforms() -> [([f32; 3], [[f32; 3]; 3]); SHATTER_PANES as usize] {
    [
        // Pane 0: dead ahead, slight downward tilt.
        ([0.0, -0.1, -2.6], mat3_rot_x(-0.15)),
        // Pane 1: tilted left, closer.
        (
            [-1.6, 0.35, -1.9],
            mat3_mul(mat3_rot_y(0.6), mat3_rot_z(-0.1)),
        ),
        // Pane 2: tilted right, mid-depth.
        (
            [1.55, -0.25, -1.7],
            mat3_mul(mat3_rot_y(-0.55), mat3_rot_z(0.18)),
        ),
        // Pane 3: overhead, angled down toward viewer.
        ([0.0, 0.95, -2.15], mat3_rot_x(-0.5)),
    ]
}

fn mat3_rot_x(a: f32) -> [[f32; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]]
}
fn mat3_rot_y(a: f32) -> [[f32; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]]
}
fn mat3_rot_z(a: f32) -> [[f32; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
}
fn mat3_mul(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut out = [[0.0f32; 3]; 3];
    for col in 0..3 {
        for row in 0..3 {
            let mut s = 0.0;
            for k in 0..3 {
                s += a[k][row] * b[col][k];
            }
            out[col][row] = s;
        }
    }
    out
}
fn mat3_mul_vec3(m: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn create_depth(device: &wgpu::Device, size: (u32, u32)) -> (wgpu::TextureView, (u32, u32)) {
    let clamped = (size.0.max(1), size.1.max(1));
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("matrisaver-present-3d-depth"),
        size: wgpu::Extent3d {
            width: clamped.0,
            height: clamped.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    (tex.create_view(&Default::default()), clamped)
}

fn aspect_for(size: (u32, u32)) -> f32 {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    w / h
}

/// Where we are in the pose cycle right now.
///
/// The cycle is: FLAT → CYL → TUBE → SHATTER → FLAT, with a HOLD at
/// each pose and a RAMP between neighbours. Returned `(pose_a, pose_b, t)`:
///   - during a HOLD: pose_a == pose_b, t = 0
///   - during a RAMP: pose_a, pose_b are the two adjacent poses, t goes
///     0 → 1 across the ramp
struct PoseFrame {
    pose_a: u32,
    pose_b: u32,
    t: f32,
}

fn compute_pose_frame(t_secs: f32) -> PoseFrame {
    let t = t_secs.rem_euclid(CYCLE_SECS);
    // Segments in order: flat → cyl → tube → shatter → flat.
    let segs: [(u32, u32, f32); 8] = [
        (POSE_FLAT, POSE_FLAT, HOLD_SECS),       // hold flat
        (POSE_FLAT, POSE_CYL, RAMP_SECS),        // ramp flat → cyl
        (POSE_CYL, POSE_CYL, HOLD_SECS),         // hold cyl
        (POSE_CYL, POSE_TUBE, RAMP_SECS),        // ramp cyl → tube
        (POSE_TUBE, POSE_TUBE, HOLD_SECS),       // hold tube (with clock)
        (POSE_TUBE, POSE_SHATTER, RAMP_SECS),    // ramp tube → shatter
        (POSE_SHATTER, POSE_SHATTER, HOLD_SECS), // hold shatter
        (POSE_SHATTER, POSE_FLAT, RAMP_SECS),    // ramp shatter → flat
    ];
    let mut acc = 0.0;
    for (a, b, dur) in segs {
        if t < acc + dur {
            let local = (t - acc) / dur;
            let is_hold = a == b;
            return PoseFrame {
                pose_a: a,
                pose_b: b,
                t: if is_hold { 0.0 } else { smoothstep01(local) },
            };
        }
        acc += dur;
    }
    // Fallback (shouldn't hit).
    PoseFrame {
        pose_a: POSE_FLAT,
        pose_b: POSE_FLAT,
        t: 0.0,
    }
}

fn smoothstep01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Per-pose camera setup. Returns (eye, target).
fn pose_camera(pose: u32, t: f32) -> ([f32; 3], [f32; 3]) {
    match pose {
        // POSE_FLAT: camera at origin, looking straight ahead at the
        // flat wall of code. Tiny +Z nudge for a well-defined look-at.
        0 => ([0.0, 0.0, 0.001], [0.0, 0.0, -1.0]),
        // POSE_CYL: off-axis orbit + tilt inside the cylinder.
        1 => {
            let angle = t * CAMERA_ROT_RAD_PER_SEC;
            let (sin_a, cos_a) = angle.sin_cos();
            let orbit_r: f32 = 0.9;
            let eye = [orbit_r * cos_a, 0.15 * (t * 0.7).sin(), orbit_r * sin_a];
            let target = [0.0, 0.35, 0.0];
            (eye, target)
        }
        // POSE_TUBE: camera drifts along the tube axis, looking
        // straight down the barrel. Drift cycles so the fly-through
        // never actually reaches the far end.
        2 => {
            let drift = ((t * 0.35).sin() * 0.5 + 0.5) * (TUBE_LEN * 0.4);
            let eye = [0.0, 0.0, (TUBE_LEN * 0.5) - drift];
            let target = [0.0, 0.0, -1.0];
            (eye, target)
        }
        // POSE_SHATTER: camera does a slow lissajous-like drift so
        // the shards visibly parallax against each other. Look-at
        // stays around the group center for a stable framing.
        _ => {
            let eye = [
                0.5 * (t * 0.20).sin(),
                0.3 * (t * 0.27).cos(),
                0.4 * (t * 0.16).sin() + 0.2,
            ];
            let target = [0.0, 0.05, -1.5];
            (eye, target)
        }
    }
}

/// Column-major 4×4 view*proj matrix. Blends between per-pose
/// camera setups using the current pose_frame:
///   POSE_FLAT: fixed at origin, looking straight down −Z (2D view)
///   POSE_CYL:  off-axis orbit inside cylinder, tilted up ~12°
///   POSE_TUBE: camera drifts slowly along tube axis toward the far
///              end, looking straight down −Z into the tunnel
/// Right-handed look-at, camera-space -Z = view direction.
fn compute_view_proj(t: f32, aspect: f32, frame: &PoseFrame) -> [[f32; 4]; 4] {
    let (eye_a, target_a) = pose_camera(frame.pose_a, t);
    let (eye_b, target_b) = pose_camera(frame.pose_b, t);
    let eye = mix3(eye_a, eye_b, frame.t);
    let target = mix3(target_a, target_b, frame.t);
    let up = [0.0, 1.0, 0.0];

    let view = look_at_rh(eye, target, up);

    // Perspective projection, right-handed, depth 0..1 (wgpu convention).
    let f = 1.0 / (FOV_Y_RAD * 0.5).tan();
    let range_inv = 1.0 / (NEAR - FAR);
    let proj: [[f32; 4]; 4] = [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, FAR * range_inv, -1.0],
        [0.0, 0.0, FAR * NEAR * range_inv, 0.0],
    ];

    mat4_mul(proj, view)
}

fn look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let fwd = normalize(sub(target, eye));
    let right = normalize(cross(fwd, up));
    let up = cross(right, fwd);
    // Column-major view matrix. Camera space: +X right, +Y up, -Z fwd.
    [
        [right[0], up[0], -fwd[0], 0.0],
        [right[1], up[1], -fwd[1], 0.0],
        [right[2], up[2], -fwd[2], 0.0],
        [-dot(right, eye), -dot(up, eye), dot(fwd, eye), 1.0],
    ]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k][row] * b[col][k];
            }
            out[col][row] = s;
        }
    }
    out
}

const WGSL: &str = r#"
struct SceneUniform {
    view_proj: mat4x4<f32>,
    time_morph: vec4<f32>,   // (t, morph_t, 0, 0)
    pose_pair: vec4<u32>,    // (pose_a, pose_b, 0, 0)
}

@group(0) @binding(0) var<uniform> scene: SceneUniform;
@group(0) @binding(1) var scene_tex: texture_2d<f32>;
@group(0) @binding(2) var scene_sampler: sampler;

struct VsIn {
    @location(0) position_flat:    vec3<f32>,
    @location(1) position_cyl:     vec3<f32>,
    @location(2) position_tube:    vec3<f32>,
    @location(3) position_shatter: vec3<f32>,
    @location(4) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) view_dist: f32,
    @location(2) three_d_weight: f32,  // 0 in flat pose, 1 in any 3D pose
};

fn pick_pose(in: VsIn, idx: u32) -> vec3<f32> {
    // POSE_FLAT=0, POSE_CYL=1, POSE_TUBE=2, POSE_SHATTER=3
    if (idx == 0u) { return in.position_flat; }
    if (idx == 1u) { return in.position_cyl; }
    if (idx == 2u) { return in.position_tube; }
    return in.position_shatter;
}

fn is_flat(idx: u32) -> f32 {
    if (idx == 0u) { return 0.0; }
    return 1.0;
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let morph_t = scene.time_morph.y;
    let pose_a = scene.pose_pair.x;
    let pose_b = scene.pose_pair.y;
    let pos_a = pick_pose(in, pose_a);
    let pos_b = pick_pose(in, pose_b);
    let world_pos = mix(pos_a, pos_b, morph_t);
    out.position = scene.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = in.uv;
    out.view_dist = length(world_pos);
    // Fog is a 3D-mode-only effect; blend between "no fog" (in the
    // flat pose) and "distance fog" (in any 3D pose).
    let a = is_flat(pose_a);
    let b = is_flat(pose_b);
    out.three_d_weight = mix(a, b, morph_t);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(scene_tex, scene_sampler, in.uv).rgb;
    let fog_3d = clamp(1.15 - in.view_dist * 0.09, 0.55, 1.0);
    let fog = mix(1.0, fog_3d, in.three_d_weight);
    return vec4<f32>(color * fog, 1.0);
}
"#;
