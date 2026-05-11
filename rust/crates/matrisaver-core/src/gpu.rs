//! GPU renderer scaffold: wgpu pipeline initialization and draw submission.
use crate::renderer::{GlyphAtlas, GlyphInstance};
use ab_glyph::{point, Font, FontArc, FontRef, Glyph};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GpuSelectionOptions {
    pub backend: Option<String>,
    pub adapter_name: Option<String>,
}

impl GpuSelectionOptions {
    pub fn from_env() -> Self {
        Self {
            backend: std::env::var("WGPU_BACKEND").ok(),
            adapter_name: std::env::var("WGPU_ADAPTER_NAME").ok(),
        }
    }

    pub fn with_overrides(mut self, backend: Option<String>, adapter_name: Option<String>) -> Self {
        if let Some(value) = backend {
            self.backend = Some(value);
        }
        if let Some(value) = adapter_name {
            self.adapter_name = Some(value);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterSnapshot {
    pub name: String,
    pub backend: String,
    pub device_type: String,
}

pub fn enumerate_adapters() -> Vec<AdapterSnapshot> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    adapters
        .into_iter()
        .map(|adapter| {
            let info = adapter.get_info();
            AdapterSnapshot {
                name: info.name,
                backend: format!("{:?}", info.backend),
                device_type: format!("{:?}", info.device_type),
            }
        })
        .collect()
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct QuadVertex {
    position: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ScreenVertex {
    position: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GlobalUniform {
    inv_size: [f32; 2],
    _pad: [f32; 2],
    glyph_tint: [f32; 4],
    style_params: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    texel_size: [f32; 2],
    threshold: f32,
    _pad0: f32,
    direction: [f32; 2],
    intensity: f32,
    _pad1: f32,
}

const QUAD_VERTICES: [QuadVertex; 6] = [
    QuadVertex {
        position: [-0.5, -0.5],
    },
    QuadVertex {
        position: [0.5, -0.5],
    },
    QuadVertex {
        position: [0.5, 0.5],
    },
    QuadVertex {
        position: [-0.5, -0.5],
    },
    QuadVertex {
        position: [0.5, 0.5],
    },
    QuadVertex {
        position: [-0.5, 0.5],
    },
];

const SCREEN_VERTICES: [ScreenVertex; 6] = [
    ScreenVertex {
        position: [-1.0, -1.0],
    },
    ScreenVertex {
        position: [1.0, -1.0],
    },
    ScreenVertex {
        position: [1.0, 1.0],
    },
    ScreenVertex {
        position: [-1.0, -1.0],
    },
    ScreenVertex {
        position: [1.0, 1.0],
    },
    ScreenVertex {
        position: [-1.0, 1.0],
    },
];

struct AtlasSurface<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct CellRect {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

struct RenderTargets {
    _scene_texture: wgpu::Texture,
    scene_view: wgpu::TextureView,
    _glow_a_texture: wgpu::Texture,
    glow_a_view: wgpu::TextureView,
    _glow_b_texture: wgpu::Texture,
    glow_b_view: wgpu::TextureView,
    _target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
}

pub struct GpuRendererScaffold {
    selected_adapter: AdapterSnapshot,
    device: wgpu::Device,
    queue: wgpu::Queue,
    glyph_pipeline: wgpu::RenderPipeline,
    prefilter_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    global_bind_group: wgpu::BindGroup,
    global_buffer: wgpu::Buffer,
    post_buffer: wgpu::Buffer,
    atlas_bind_group: wgpu::BindGroup,
    quad_vertex_buffer: wgpu::Buffer,
    screen_vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    surface_size: (u32, u32),
    downsample_factor: u8,
    glow_size: (u32, u32),
    post_sampler: wgpu::Sampler,
    render_targets: RenderTargets,
}

impl GpuRendererScaffold {
    pub fn initialize(
        width: u32,
        height: u32,
        atlas: &GlyphAtlas,
        selection: &GpuSelectionOptions,
    ) -> Result<Self, String> {
        pollster::block_on(Self::initialize_async(width, height, atlas, selection))
    }

    async fn initialize_async(
        width: u32,
        height: u32,
        atlas: &GlyphAtlas,
        selection: &GpuSelectionOptions,
    ) -> Result<Self, String> {
        let requested_backends = if let Some(raw) = selection.backend.as_deref() {
            let parsed = wgpu::Backends::from_comma_list(raw);
            if parsed.is_empty() {
                return Err(format!(
                        "Invalid WGPU backend selection '{raw}'. Use comma-separated names like 'vulkan,gl' or 'dx12'."
                    ));
            }
            parsed
        } else {
            wgpu::Backends::all()
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: requested_backends,
            ..Default::default()
        });
        let mut adapters = instance.enumerate_adapters(requested_backends).await;
        if adapters.is_empty() {
            return Err(format!(
                "No adapters available for backend selection '{}'.",
                selection.backend.as_deref().unwrap_or("all")
            ));
        }

        let adapter = if let Some(adapter_hint) = selection.adapter_name.as_deref() {
            let wanted = adapter_hint.to_lowercase();
            let available: Vec<String> = adapters
                .iter()
                .map(|adapter| {
                    let info = adapter.get_info();
                    format!("{}({:?})", info.name, info.backend)
                })
                .collect();
            adapters
                .drain(..)
                .find(|adapter| adapter.get_info().name.to_lowercase().contains(&wanted))
                .ok_or_else(|| {
                    format!(
                        "No adapter matched hint '{adapter_hint}'. Available adapters: {}",
                        available.join(", ")
                    )
                })?
        } else {
            let mut chosen = adapters.remove(0);
            let mut chosen_rank = Self::adapter_rank(chosen.get_info().device_type);
            for adapter in adapters {
                let rank = Self::adapter_rank(adapter.get_info().device_type);
                if rank < chosen_rank {
                    chosen = adapter;
                    chosen_rank = rank;
                }
            }
            chosen
        };
        let adapter_info = adapter.get_info();
        let selected_adapter = AdapterSnapshot {
            name: adapter_info.name,
            backend: format!("{:?}", adapter_info.backend),
            device_type: format!("{:?}", adapter_info.device_type),
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|error| format!("request_device failed: {error}"))?;

        Self::initialize_from_device(width, height, atlas, selected_adapter, device, queue)
    }

    pub fn initialize_with_shared_device(
        width: u32,
        height: u32,
        atlas: &GlyphAtlas,
        selected_adapter: AdapterSnapshot,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, String> {
        Self::initialize_from_device(width, height, atlas, selected_adapter, device, queue)
    }

    fn initialize_from_device(
        width: u32,
        height: u32,
        atlas: &GlyphAtlas,
        selected_adapter: AdapterSnapshot,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, String> {
        let glyph_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("matrisaver-scaffold-glyph-shader"),
                source: wgpu::ShaderSource::Wgsl(
                    "
                    struct Globals {
                        inv_size: vec2<f32>,
                        _pad: vec2<f32>,
                        glyph_tint: vec4<f32>,
                        style_params: vec4<f32>,
                    };

                    @group(0) @binding(0)
                    var<uniform> globals: Globals;

                    @group(1) @binding(0)
                    var atlas_tex: texture_2d<f32>;

                    @group(1) @binding(1)
                    var atlas_sampler: sampler;

                    struct VertexIn {
                        @location(0) local_pos: vec2<f32>,
                        @location(1) instance_pos_size: vec4<f32>,
                        @location(2) instance_uv: vec4<f32>,
                        @location(3) instance_params: vec4<f32>,
                    };

                    struct VertexOut {
                        @builtin(position) clip_position: vec4<f32>,
                        @location(0) uv: vec2<f32>,
                        @location(1) brightness: f32,
                        @location(2) head_boost: f32,
                        @location(3) grain: f32,
                        @location(4) style_tag: f32,
                    };

                    @vertex
                    fn vs_main(input: VertexIn) -> VertexOut {
                        let pixel = input.instance_pos_size.xy + input.local_pos * input.instance_pos_size.zw;
                        let clip = vec2<f32>(
                            pixel.x * globals.inv_size.x - 1.0,
                            1.0 - pixel.y * globals.inv_size.y
                        );

                        var output: VertexOut;
                        output.clip_position = vec4<f32>(clip, 0.0, 1.0);
                        let uv_t = input.local_pos + vec2<f32>(0.5, 0.5);
                        output.uv = mix(input.instance_uv.xy, input.instance_uv.zw, uv_t);
                        output.brightness = input.instance_params.x;
                        output.head_boost = input.instance_params.y;
                        output.grain = input.instance_params.z;
                        output.style_tag = input.instance_params.w;
                        return output;
                    }

                    @fragment
                    fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
                        let atlas_value = textureSample(atlas_tex, atlas_sampler, input.uv).r;
                        let glyph_mask = smoothstep(0.15, 0.78, atlas_value);
                        let edge_glint = smoothstep(0.75, 1.0, atlas_value) * input.head_boost;
                        let is_ghost = step(1.5, input.style_tag);
                        let is_volatile = step(0.5, input.style_tag) - is_ghost;
                        let pulse = (0.5 + 0.5 * sin((input.grain + input.brightness) * 18.0))
                            * is_volatile
                            * globals.style_params.w;
                        let raw_value = clamp(
                            input.brightness * glyph_mask + edge_glint * (0.45 + globals.style_params.y * 0.2),
                            0.0,
                            1.0,
                        );
                        let volatile_gamma = max(globals.style_params.x, 0.35);
                        let value_base = pow(raw_value, mix(1.0, 1.0 / volatile_gamma, is_volatile));
                        let value_pulsed = clamp(value_base * (1.0 + pulse), 0.0, 1.0);
                        let value_final = value_pulsed;
                        let head_mix = clamp(input.head_boost * (0.65 + globals.style_params.y * 0.35), 0.0, 1.0);
                        let base_color = globals.glyph_tint.rgb;
                        let head_color = mix(base_color, vec3<f32>(0.86, 1.0, 0.92), head_mix);
                        let ghost_color = mix(base_color * 0.28, vec3<f32>(0.08, 0.22, 0.14), 0.35);
                        let color = mix(head_color, ghost_color, is_ghost);
                        let alpha_scale = mix(1.0, globals.style_params.z, is_ghost);
                        let grain = 0.95 + input.grain * 0.08;
                        return vec4<f32>(color * value_final * grain * alpha_scale, 1.0);
                    }
                    "
                    .into(),
                ),
            });

        let post_filter_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("matrisaver-scaffold-post-filter-shader"),
                source: wgpu::ShaderSource::Wgsl(
                    "
                    struct PostUniform {
                        texel_size: vec2<f32>,
                        threshold: f32,
                        _pad0: f32,
                        direction: vec2<f32>,
                        intensity: f32,
                        _pad1: f32,
                    };

                    @group(0) @binding(0)
                    var src_tex: texture_2d<f32>;

                    @group(0) @binding(1)
                    var src_sampler: sampler;

                    @group(0) @binding(2)
                    var<uniform> post: PostUniform;

                    struct VertexIn {
                        @location(0) pos: vec2<f32>,
                    };

                    struct VertexOut {
                        @builtin(position) clip_position: vec4<f32>,
                        @location(0) uv: vec2<f32>,
                    };

                    @vertex
                    fn vs_screen(input: VertexIn) -> VertexOut {
                        var output: VertexOut;
                        output.clip_position = vec4<f32>(input.pos, 0.0, 1.0);
                        output.uv = input.pos * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
                        return output;
                    }

                    @fragment
                    fn fs_prefilter(input: VertexOut) -> @location(0) vec4<f32> {
                        let base = textureSample(src_tex, src_sampler, input.uv).rgb;
                        let right = textureSample(src_tex, src_sampler, input.uv + vec2<f32>(post.texel_size.x, 0.0)).rgb;
                        let down = textureSample(src_tex, src_sampler, input.uv + vec2<f32>(0.0, post.texel_size.y)).rgb;
                        let avg = (base + right + down) / 3.0;
                        let luminance = dot(avg, vec3<f32>(0.2126, 0.7152, 0.0722));
                        let boosted = max(luminance - post.threshold, 0.0);
                        return vec4<f32>(avg * boosted * post.intensity, 1.0);
                    }

                    @fragment
                    fn fs_blur(input: VertexOut) -> @location(0) vec4<f32> {
                        let center = textureSample(src_tex, src_sampler, input.uv).rgb * 0.4;
                        let tap_a = textureSample(src_tex, src_sampler, input.uv + post.direction * post.texel_size * 1.5).rgb * 0.3;
                        let tap_b = textureSample(src_tex, src_sampler, input.uv - post.direction * post.texel_size * 1.5).rgb * 0.3;
                        return vec4<f32>(center + tap_a + tap_b, 1.0);
                    }
                    "
                    .into(),
                ),
            });

        let post_composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matrisaver-scaffold-post-composite-shader"),
            source: wgpu::ShaderSource::Wgsl(
                "
                    @group(0) @binding(0)
                    var scene_tex: texture_2d<f32>;

                    @group(0) @binding(1)
                    var glow_tex: texture_2d<f32>;

                    @group(0) @binding(2)
                    var mix_sampler: sampler;

                    struct VertexIn {
                        @location(0) pos: vec2<f32>,
                    };

                    struct VertexOut {
                        @builtin(position) clip_position: vec4<f32>,
                        @location(0) uv: vec2<f32>,
                    };

                    @vertex
                    fn vs_screen(input: VertexIn) -> VertexOut {
                        var output: VertexOut;
                        output.clip_position = vec4<f32>(input.pos, 0.0, 1.0);
                        output.uv = input.pos * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
                        return output;
                    }

                    @fragment
                    fn fs_composite(input: VertexOut) -> @location(0) vec4<f32> {
                        let scene = textureSample(scene_tex, mix_sampler, input.uv).rgb;
                        let glow = textureSample(glow_tex, mix_sampler, input.uv).rgb;
                        let color = scene + glow * 0.9;
                        return vec4<f32>(color, 1.0);
                    }
                    "
                .into(),
            ),
        });

        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matrisaver-scaffold-global-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(
                        std::mem::size_of::<GlobalUniform>() as u64,
                    ),
                },
                count: None,
            }],
        });

        let global_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrisaver-scaffold-global-buffer"),
            contents: bytemuck::bytes_of(&GlobalUniform {
                inv_size: [2.0 / width.max(1) as f32, 2.0 / height.max(1) as f32],
                _pad: [0.0; 2],
                glyph_tint: [0.0, 1.0, 0.35, 1.0],
                style_params: [1.0, 1.0, 0.48, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-global-bind-group"),
            layout: &global_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: global_buffer.as_entire_binding(),
            }],
        });

        let post_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrisaver-scaffold-post-buffer"),
            contents: bytemuck::bytes_of(&PostUniform {
                texel_size: [1.0 / width.max(1) as f32, 1.0 / height.max(1) as f32],
                threshold: 0.45,
                _pad0: 0.0,
                direction: [1.0, 0.0],
                intensity: 1.0,
                _pad1: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matrisaver-scaffold-atlas-layout"),
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
            ],
        });

        let post_single_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("matrisaver-scaffold-post-single-layout"),
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
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                PostUniform,
                            >(
                            )
                                as u64),
                        },
                        count: None,
                    },
                ],
            });

        let post_mix_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matrisaver-scaffold-post-mix-layout"),
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

        let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("matrisaver-scaffold-post-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let (atlas_texture, atlas_view) = Self::create_atlas_texture(&device, &queue, atlas);
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("matrisaver-scaffold-atlas-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-atlas-bind-group"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrisaver-scaffold-quad-vertices"),
            contents: bytemuck::cast_slice(&QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let screen_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrisaver-scaffold-screen-vertices"),
            contents: bytemuck::cast_slice(&SCREEN_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("matrisaver-scaffold-instance-buffer"),
            size: (instance_capacity as u64) * std::mem::size_of::<GlyphInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let glow_factor = 2;
        let render_targets =
            Self::create_render_targets(&device, width.max(1), height.max(1), glow_factor);

        let glyph_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matrisaver-scaffold-glyph-layout"),
            bind_group_layouts: &[&global_layout, &atlas_layout],
            immediate_size: 0,
        });

        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("matrisaver-scaffold-glyph-pipeline"),
            layout: Some(&glyph_layout),
            vertex: wgpu::VertexState {
                module: &glyph_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 0,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 2,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 32,
                                shader_location: 3,
                            },
                        ],
                    },
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &glyph_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matrisaver-scaffold-post-layout"),
            bind_group_layouts: &[&post_single_layout],
            immediate_size: 0,
        });

        let prefilter_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("matrisaver-scaffold-prefilter-pipeline"),
            layout: Some(&post_layout),
            vertex: wgpu::VertexState {
                module: &post_filter_shader,
                entry_point: Some("vs_screen"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ScreenVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_filter_shader,
                entry_point: Some("fs_prefilter"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
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

        let blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("matrisaver-scaffold-blur-pipeline"),
            layout: Some(&post_layout),
            vertex: wgpu::VertexState {
                module: &post_filter_shader,
                entry_point: Some("vs_screen"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ScreenVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_filter_shader,
                entry_point: Some("fs_blur"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
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

        let composite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matrisaver-scaffold-composite-layout"),
            bind_group_layouts: &[&post_mix_layout],
            immediate_size: 0,
        });

        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("matrisaver-scaffold-composite-pipeline"),
            layout: Some(&composite_layout),
            vertex: wgpu::VertexState {
                module: &post_composite_shader,
                entry_point: Some("vs_screen"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ScreenVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &post_composite_shader,
                entry_point: Some("fs_composite"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
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

        let _keep_alive = atlas_texture;

        Ok(Self {
            selected_adapter,
            device,
            queue,
            glyph_pipeline,
            prefilter_pipeline,
            blur_pipeline,
            composite_pipeline,
            global_bind_group,
            global_buffer,
            post_buffer,
            atlas_bind_group,
            quad_vertex_buffer,
            screen_vertex_buffer,
            instance_buffer,
            instance_capacity,
            surface_size: (width.max(1), height.max(1)),
            downsample_factor: glow_factor,
            glow_size: (
                (width.max(1) / glow_factor as u32).max(1),
                (height.max(1) / glow_factor as u32).max(1),
            ),
            post_sampler,
            render_targets,
        })
    }

    fn adapter_rank(device_type: wgpu::DeviceType) -> u8 {
        match device_type {
            wgpu::DeviceType::DiscreteGpu => 0,
            wgpu::DeviceType::IntegratedGpu => 1,
            wgpu::DeviceType::VirtualGpu => 2,
            wgpu::DeviceType::Other => 3,
            wgpu::DeviceType::Cpu => 4,
        }
    }

    pub fn selected_adapter(&self) -> &AdapterSnapshot {
        &self.selected_adapter
    }

    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.render_targets.target_view
    }

    fn create_render_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        downsample_factor: u8,
    ) -> RenderTargets {
        let factor = downsample_factor.max(1) as u32;
        let glow_width = (width / factor).max(1);
        let glow_height = (height / factor).max(1);

        let scene_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("matrisaver-scaffold-scene"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let scene_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let glow_a_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("matrisaver-scaffold-glow-a"),
            size: wgpu::Extent3d {
                width: glow_width,
                height: glow_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let glow_a_view = glow_a_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let glow_b_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("matrisaver-scaffold-glow-b"),
            size: wgpu::Extent3d {
                width: glow_width,
                height: glow_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let glow_b_view = glow_b_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let target_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("matrisaver-scaffold-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

        RenderTargets {
            _scene_texture: scene_texture,
            scene_view,
            _glow_a_texture: glow_a_texture,
            glow_a_view,
            _glow_b_texture: glow_b_texture,
            glow_b_view,
            _target_texture: target_texture,
            target_view,
        }
    }

    fn try_load_embedded_font() -> Option<FontArc> {
        const CJK_FONT_BYTES: &[u8] =
            include_bytes!("../../../../assets/fonts/NotoSansCJK-Regular.ttc");
        FontArc::try_from_slice(CJK_FONT_BYTES).ok().or_else(|| {
            FontRef::try_from_slice_and_index(CJK_FONT_BYTES, 0)
                .ok()
                .map(FontArc::new)
        })
    }

    fn draw_placeholder_cell(surface: AtlasSurface<'_>, rect: CellRect, glyph_phase: f32) {
        let cell_w = rect.x1.saturating_sub(rect.x0).max(1);
        let cell_h = rect.y1.saturating_sub(rect.y0).max(1);
        for y in rect.y0..rect.y1.min(surface.height) {
            for x in rect.x0..rect.x1.min(surface.width) {
                let nx = (x.saturating_sub(rect.x0)) as f32 / cell_w as f32;
                let ny = (y.saturating_sub(rect.y0)) as f32 / cell_h as f32;
                let stroke_v = (nx - 0.5).abs() < 0.08;
                let stroke_h = (ny - 0.5).abs() < 0.08;
                let slash = ((nx + ny + glyph_phase) * 8.0).fract() < 0.2;
                let mut value = if stroke_v || stroke_h || slash {
                    220
                } else {
                    20
                };
                if (nx - 0.5).hypot(ny - 0.5) > 0.48 {
                    value = 0;
                }
                surface.pixels[(y * surface.width + x) as usize] = value;
            }
        }
    }

    fn draw_font_cell(
        surface: AtlasSurface<'_>,
        rect: CellRect,
        glyph_char: char,
        font: &FontArc,
        glyph_size: f32,
    ) -> bool {
        let cell_w = rect.x1.saturating_sub(rect.x0).max(1) as f32;
        let cell_h = rect.y1.saturating_sub(rect.y0).max(1) as f32;
        let scale_value = (glyph_size * 0.98).max(4.0);
        let scale = ab_glyph::PxScale {
            x: scale_value,
            y: scale_value,
        };
        let glyph_id = font.glyph_id(glyph_char);
        let probe = Glyph {
            id: glyph_id,
            scale,
            position: point(0.0, 0.0),
        };
        let Some(probe_outline) = font.outline_glyph(probe) else {
            return false;
        };
        let probe_bounds = probe_outline.px_bounds();
        if probe_bounds.width() <= 0.0 || probe_bounds.height() <= 0.0 {
            return false;
        }
        let offset_x = rect.x0 as f32 + (cell_w - probe_bounds.width()) * 0.5 - probe_bounds.min.x;
        let offset_y = rect.y0 as f32 + (cell_h - probe_bounds.height()) * 0.5 - probe_bounds.min.y;
        let glyph = Glyph {
            id: glyph_id,
            scale,
            position: point(offset_x, offset_y),
        };
        let Some(outline) = font.outline_glyph(glyph) else {
            return false;
        };
        let AtlasSurface {
            pixels,
            width,
            height,
        } = surface;
        outline.draw(|x, y, coverage| {
            let px = rect.x0.saturating_add(x);
            let py = rect.y0.saturating_add(y);
            if px >= width || py >= height {
                return;
            }
            let idx = (py * width + px) as usize;
            let value = (coverage * 255.0).round().clamp(0.0, 255.0) as u8;
            pixels[idx] = pixels[idx].max(value);
        });
        true
    }

    fn create_atlas_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &GlyphAtlas,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let width = atlas.texture_size.0.max(1) as u32;
        let height = atlas.texture_size.1.max(1) as u32;
        let mut pixels = vec![0u8; (width * height) as usize];
        let font = Self::try_load_embedded_font();
        let glyph_size = atlas.glyph_size.max(1) as f32;

        for (glyph_index, glyph) in atlas.glyphs.iter().enumerate() {
            let x0 = (glyph.u0 * width as f32).floor() as u32;
            let y0 = (glyph.v0 * height as f32).floor() as u32;
            let x1 = (glyph.u1 * width as f32).ceil() as u32;
            let y1 = (glyph.v1 * height as f32).ceil() as u32;
            let glyph_phase = ((glyph_index as f32) * 0.173).fract();

            let rect = CellRect { x0, y0, x1, y1 };
            let rendered = font.as_ref().is_some_and(|font_ref| {
                Self::draw_font_cell(
                    AtlasSurface {
                        pixels: &mut pixels,
                        width,
                        height,
                    },
                    rect,
                    glyph.glyph,
                    font_ref,
                    glyph_size,
                )
            });

            if !rendered {
                Self::draw_placeholder_cell(
                    AtlasSurface {
                        pixels: &mut pixels,
                        width,
                        height,
                    },
                    rect,
                    glyph_phase,
                );
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("matrisaver-scaffold-atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn set_surface_size(&mut self, width: u32, height: u32) {
        self.surface_size = (width.max(1), height.max(1));
    }

    fn ensure_instance_capacity(&mut self, instance_count: u32) {
        if instance_count <= self.instance_capacity {
            return;
        }
        let mut next_capacity = self.instance_capacity.max(1);
        while next_capacity < instance_count {
            next_capacity = next_capacity.saturating_mul(2);
        }
        self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("matrisaver-scaffold-instance-buffer"),
            size: (next_capacity as u64) * std::mem::size_of::<GlyphInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_capacity = next_capacity;
    }

    fn ensure_target(&mut self, downsample_factor: u8) {
        let next_factor = downsample_factor.max(1);
        let factor = next_factor as u32;
        let next_glow_size = (
            (self.surface_size.0 / factor).max(1),
            (self.surface_size.1 / factor).max(1),
        );
        if self.downsample_factor == next_factor && self.glow_size == next_glow_size {
            return;
        }
        let render_targets = Self::create_render_targets(
            &self.device,
            self.surface_size.0,
            self.surface_size.1,
            next_factor,
        );
        self.downsample_factor = next_factor;
        self.glow_size = next_glow_size;
        self.render_targets = render_targets;
    }

    fn draw_post_pass(
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        screen_vertex_buffer: &wgpu::Buffer,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("matrisaver-scaffold-post-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
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
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, screen_vertex_buffer.slice(..));
        pass.draw(0..6, 0..1);
    }

    pub fn draw_instanced_pass(
        &mut self,
        instances: &[GlyphInstance],
        downsample_factor: u8,
        glyph_tint: [f32; 3],
        style_params: [f32; 4],
    ) {
        if instances.is_empty() {
            return;
        }
        self.ensure_target(downsample_factor);
        self.ensure_instance_capacity(instances.len() as u32);
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
        self.queue.write_buffer(
            &self.global_buffer,
            0,
            bytemuck::bytes_of(&GlobalUniform {
                inv_size: [
                    2.0 / self.surface_size.0.max(1) as f32,
                    2.0 / self.surface_size.1.max(1) as f32,
                ],
                _pad: [0.0; 2],
                glyph_tint: [glyph_tint[0], glyph_tint[1], glyph_tint[2], 1.0],
                style_params,
            }),
        );

        let source_texel = [
            1.0 / self.surface_size.0.max(1) as f32,
            1.0 / self.surface_size.1.max(1) as f32,
        ];
        let glow_texel = [
            1.0 / self.glow_size.0.max(1) as f32,
            1.0 / self.glow_size.1.max(1) as f32,
        ];

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("matrisaver-scaffold-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("matrisaver-scaffold-glyph-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_targets.scene_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
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
            pass.set_pipeline(&self.glyph_pipeline);
            pass.set_bind_group(0, &self.global_bind_group, &[]);
            pass.set_bind_group(1, &self.atlas_bind_group, &[]);
            pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            pass.draw(0..6, 0..instances.len() as u32);
        }

        self.queue.write_buffer(
            &self.post_buffer,
            0,
            bytemuck::bytes_of(&PostUniform {
                texel_size: source_texel,
                threshold: 0.45,
                _pad0: 0.0,
                direction: [0.0, 0.0],
                intensity: 1.2,
                _pad1: 0.0,
            }),
        );
        let prefilter_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-prefilter-bind-group"),
            layout: &self.prefilter_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.render_targets.scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.post_buffer.as_entire_binding(),
                },
            ],
        });
        Self::draw_post_pass(
            &mut encoder,
            &self.render_targets.glow_a_view,
            &self.prefilter_pipeline,
            &prefilter_bind_group,
            &self.screen_vertex_buffer,
        );

        self.queue.write_buffer(
            &self.post_buffer,
            0,
            bytemuck::bytes_of(&PostUniform {
                texel_size: glow_texel,
                threshold: 0.0,
                _pad0: 0.0,
                direction: [1.0, 0.0],
                intensity: 1.0,
                _pad1: 0.0,
            }),
        );
        let blur_horizontal_bind_group =
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("matrisaver-scaffold-blur-h-bind-group"),
                layout: &self.blur_pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &self.render_targets.glow_a_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.post_buffer.as_entire_binding(),
                    },
                ],
            });
        Self::draw_post_pass(
            &mut encoder,
            &self.render_targets.glow_b_view,
            &self.blur_pipeline,
            &blur_horizontal_bind_group,
            &self.screen_vertex_buffer,
        );

        self.queue.write_buffer(
            &self.post_buffer,
            0,
            bytemuck::bytes_of(&PostUniform {
                texel_size: glow_texel,
                threshold: 0.0,
                _pad0: 0.0,
                direction: [0.0, 1.0],
                intensity: 1.0,
                _pad1: 0.0,
            }),
        );
        let blur_vertical_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-blur-v-bind-group"),
            layout: &self.blur_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.render_targets.glow_b_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.post_buffer.as_entire_binding(),
                },
            ],
        });
        Self::draw_post_pass(
            &mut encoder,
            &self.render_targets.glow_a_view,
            &self.blur_pipeline,
            &blur_vertical_bind_group,
            &self.screen_vertex_buffer,
        );

        let composite_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-composite-bind-group"),
            layout: &self.composite_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.render_targets.scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.render_targets.glow_a_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                },
            ],
        });

        Self::draw_post_pass(
            &mut encoder,
            &self.render_targets.target_view,
            &self.composite_pipeline,
            &composite_bind_group,
            &self.screen_vertex_buffer,
        );

        self.queue.submit([encoder.finish()]);
    }
}
