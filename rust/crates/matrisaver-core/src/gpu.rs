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
    /// `.0` = animation_seconds (drives the head-glyph specular sheen).
    /// `.1` is still spare padding.
    time_pad: [f32; 2],
    glyph_tint: [f32; 4],
    style_params: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    texel_size: [f32; 2],
    threshold: f32,
    intensity: f32,
    /// 0.0 = plain box downsample, 1.0 = first-downsample (Karis
    /// luma-weighted average + soft-knee HDR threshold), 2.0 = upsample.
    /// The separable-blur `direction` field is gone — mip-chain bloom
    /// doesn't need it.
    mode: f32,
    // Three scalar pads (not a `vec3<f32>`) because in WGSL's uniform
    // address space `vec3<f32>` aligns to 16 and would force the struct
    // to 48 bytes, mismatching this 32-byte Rust layout. Scalar f32
    // fields keep the struct at a clean 32 bytes in both worlds.
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

// Compile-time layout assertions for the GPU-bound uniform structs.
// WGSL's uniform address space has stricter alignment rules than C
// (`vec3<f32>` aligns to 16; nested arrays have stride 16; padding
// after `f32` followed by `vec2/vec3/vec4` is not free). A silent
// drift between the Rust layout and the WGSL-declared layout produces
// a wgpu validation error at pipeline creation, not at compile time,
// so by then the regression has already shipped to `/s`.
//
// These const-asserts make the layout part of the type system. If
// anyone adds a field or changes a type, the build fails immediately
// with a message pointing at the size expectation that was violated.
// Cross-check the expectations with the WGSL `struct` declarations in
// the shader sources — same number of bytes, same field order.
const _: () = assert!(
    std::mem::size_of::<GlobalUniform>() == 48,
    "GlobalUniform must be 48 bytes — Rust struct layout drifted from the WGSL declaration"
);
const _: () = assert!(
    std::mem::size_of::<PostUniform>() == 32,
    "PostUniform must be 32 bytes — Rust struct layout drifted from the WGSL declaration"
);

/// Number of bloom mip levels. Mip 0 is `surface / downsample_factor`,
/// each subsequent mip halves both dimensions. 5 levels at base 1/2
/// gives sizes 1/2, 1/4, 1/8, 1/16, 1/32 — wide enough for the soft
/// phosphor halo the 1999 grade wants.
const BLOOM_MIP_COUNT: u32 = 5;

/// HDR storage format for scene, persistence, and bloom textures.
/// Rgba16Float gives ~5 stops of headroom above display range so
/// super-bright head glyphs have room to be "actually bright" before
/// tone mapping crushes them back to 0..1 for the display surface.
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

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

// ─────────────────────────────────────────────────────────────────────
// WGSL shader sources
// ─────────────────────────────────────────────────────────────────────
//
// Extracted to module-level constants so they're (a) usable by the
// pipeline-creation paths AND (b) parse-testable via naga without a
// GPU. The `wgsl_shaders_parse_cleanly` test runs `naga::front::wgsl::parse_str`
// against each one, catching syntax errors at `cargo test` time rather
// than at first `/s` launch. See the matching test below.

const GLYPH_SHADER_WGSL: &str = "
    struct Globals {
        inv_size: vec2<f32>,
        time_pad: vec2<f32>,
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

        // Specular sheen on head glyphs — a tight diagonal
        // highlight band raking across the glyph, time-driven,
        // gated by head_mix so only leading heads chrome up.
        // (uv.x + uv.y) is the diagonal coordinate; pow() tightens
        // the band; the cool-white tint reads as liquid-mirror
        // rather than merely a brighter green.
        let sheen_phase = (input.uv.x + input.uv.y) * 3.0 - globals.time_pad.x * 2.2;
        let sheen_band = pow(max(sin(sheen_phase) * 0.5 + 0.5, 0.0), 6.0);
        let sheen = sheen_band * head_mix * 0.6;
        let head_sheened = head_color + vec3<f32>(0.55, 0.72, 0.62) * sheen;

        // HDR head boost — head glyphs emit super-bright
        // luminance into the Rgba16Float scene buffer so
        // the mip-chain bloom has real headroom to grow
        // a wide soft halo. head_mix gates this so only
        // leading heads crank up; trails stay near 1.0
        // and don't blow out the bloom.
        let head_hdr_scale = 1.0 + head_mix * 3.0;
        let head_emissive = head_sheened * head_hdr_scale;

        let color = mix(head_emissive, ghost_color, is_ghost);
        let alpha_scale = mix(1.0, globals.style_params.z, is_ghost);
        let grain = 0.95 + input.grain * 0.08;
        return vec4<f32>(color * value_final * grain * alpha_scale, 1.0);
    }
";

// Mip-chain bloom shader (replaces the old prefilter + separable
// blur). Three fragment entry points:
//   - fs_first_downsample: Karis luma-weighted 13-tap average
//     plus soft-knee HDR threshold. Run once, reads full-size
//     persist_curr, writes bloom mip 0. The Karis weighting
//     kills HDR fireflies that would otherwise survive into
//     every subsequent mip and bloom unnaturally.
//   - fs_downsample: plain 13-tap downsample (Call of Duty:
//     Advanced Warfare technique, per LearnOpenGL's
//     physically-based bloom article). Run on mips 1..N.
//   - fs_upsample: 9-tap tent filter. Run on mips N-1..0 with
//     additive blend at the pipeline level so each level adds
//     to the one below for smooth wide falloff.
const BLOOM_SHADER_WGSL: &str = "
    struct PostUniform {
        texel_size: vec2<f32>,
        threshold: f32,
        intensity: f32,
        mode: f32,
        _pad0: f32,
        _pad1: f32,
        _pad2: f32,
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

    // Karis average: weight each tap by 1/(1+luma) to
    // pull the average toward dim taps, which prevents
    // a single super-bright HDR pixel from dominating
    // the box and surviving into deeper mips as a
    // sparkling 'firefly'.
    fn karis_weight(c: vec3<f32>) -> f32 {
        let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        return 1.0 / (1.0 + luma);
    }

    // 13-tap downsample (CoD:AW). Samples a 4x4 region as
    // four 2x2 sub-averages plus a center cluster, then
    // weights them so the result has minimal aliasing
    // when feeding the next mip down.
    fn downsample_13_tap(uv: vec2<f32>, texel: vec2<f32>, karis: bool) -> vec3<f32> {
        let a = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>(-2.0, -2.0)).rgb;
        let b = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>( 0.0, -2.0)).rgb;
        let c = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>( 2.0, -2.0)).rgb;
        let d = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>(-2.0,  0.0)).rgb;
        let e = textureSample(src_tex, src_sampler, uv).rgb;
        let f = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>( 2.0,  0.0)).rgb;
        let g = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>(-2.0,  2.0)).rgb;
        let h = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>( 0.0,  2.0)).rgb;
        let i = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>( 2.0,  2.0)).rgb;
        let j = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>(-1.0, -1.0)).rgb;
        let k = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>( 1.0, -1.0)).rgb;
        let l = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>(-1.0,  1.0)).rgb;
        let m = textureSample(src_tex, src_sampler, uv + texel * vec2<f32>( 1.0,  1.0)).rgb;

        if (karis) {
            // First mip — partition into 5 groups and apply
            // per-group Karis weighting before combining.
            let g0 = (a + b + d + e) * 0.25;
            let g1 = (b + c + e + f) * 0.25;
            let g2 = (d + e + g + h) * 0.25;
            let g3 = (e + f + h + i) * 0.25;
            let g4 = (j + k + l + m) * 0.25;
            let w0 = karis_weight(g0);
            let w1 = karis_weight(g1);
            let w2 = karis_weight(g2);
            let w3 = karis_weight(g3);
            let w4 = karis_weight(g4);
            // Group 4 (the center cluster) weighted 0.5,
            // outer corners 0.125 each — matches the CoD
            // recipe's energy distribution.
            let sum = g0 * w0 * 0.125 + g1 * w1 * 0.125
                   + g2 * w2 * 0.125 + g3 * w3 * 0.125
                   + g4 * w4 * 0.5;
            let wsum = w0 * 0.125 + w1 * 0.125
                     + w2 * 0.125 + w3 * 0.125 + w4 * 0.5;
            return sum / max(wsum, 0.0001);
        }

        // Deeper mips — straight weighted average.
        return e * 0.125
             + (a + c + g + i) * 0.03125
             + (b + d + f + h) * 0.0625
             + (j + k + l + m) * 0.125;
    }

    // Soft-knee HDR threshold (Unreal-style). Linear
    // threshold has a hard cutoff that flickers; the
    // knee fades pixels in over a half-stop window.
    fn soft_threshold(color: vec3<f32>, threshold: f32) -> vec3<f32> {
        let knee = threshold * 0.5;
        let luma = max(color.r, max(color.g, color.b));
        let soft = clamp(luma - threshold + knee, 0.0, 2.0 * knee);
        let soft_factor = soft * soft / max(4.0 * knee * knee, 0.0001);
        let contrib = max(luma - threshold, soft_factor) / max(luma, 0.0001);
        return color * contrib;
    }

    @fragment
    fn fs_first_downsample(input: VertexOut) -> @location(0) vec4<f32> {
        let sampled = downsample_13_tap(input.uv, post.texel_size, true);
        let bloomed = soft_threshold(sampled, post.threshold) * post.intensity;
        return vec4<f32>(bloomed, 1.0);
    }

    @fragment
    fn fs_downsample(input: VertexOut) -> @location(0) vec4<f32> {
        let sampled = downsample_13_tap(input.uv, post.texel_size, false);
        return vec4<f32>(sampled, 1.0);
    }

    // 9-tap tent filter upsample. Each call samples a
    // small 3x3 neighborhood from the smaller mip above
    // and writes additively into the larger mip below
    // (additive blend is on the pipeline). Compounding
    // these calls produces the wide soft falloff that
    // reads as phosphor wash rather than a Gaussian
    // halo.
    @fragment
    fn fs_upsample(input: VertexOut) -> @location(0) vec4<f32> {
        let t = post.texel_size;
        let a = textureSample(src_tex, src_sampler, input.uv + vec2<f32>(-t.x, -t.y)).rgb;
        let b = textureSample(src_tex, src_sampler, input.uv + vec2<f32>( 0.0, -t.y)).rgb;
        let c = textureSample(src_tex, src_sampler, input.uv + vec2<f32>( t.x, -t.y)).rgb;
        let d = textureSample(src_tex, src_sampler, input.uv + vec2<f32>(-t.x,  0.0)).rgb;
        let e = textureSample(src_tex, src_sampler, input.uv).rgb;
        let f = textureSample(src_tex, src_sampler, input.uv + vec2<f32>( t.x,  0.0)).rgb;
        let g = textureSample(src_tex, src_sampler, input.uv + vec2<f32>(-t.x,  t.y)).rgb;
        let h = textureSample(src_tex, src_sampler, input.uv + vec2<f32>( 0.0,  t.y)).rgb;
        let i = textureSample(src_tex, src_sampler, input.uv + vec2<f32>( t.x,  t.y)).rgb;
        let tent = e * 0.25
                 + (b + d + f + h) * 0.125
                 + (a + c + g + i) * 0.0625;
        return vec4<f32>(tent * post.intensity, 1.0);
    }
";

const POST_COMPOSITE_SHADER_WGSL: &str = "
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

    // ACES filmic tone map — Krzysztof Narkowicz's
    // approximation. Preserves the institutional green
    // character of the 1999 grade in highlights better
    // than Reinhard, which would desaturate toward
    // white. Folds the open-ended HDR range into the
    // display's 0..1 with a filmic shoulder.
    fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
        let a = 2.51;
        let b = 0.03;
        let c = 2.43;
        let d = 0.59;
        let e = 0.14;
        return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
    }

    @fragment
    fn fs_composite(input: VertexOut) -> @location(0) vec4<f32> {
        let scene = textureSample(scene_tex, mix_sampler, input.uv).rgb;
        let glow = textureSample(glow_tex, mix_sampler, input.uv).rgb;
        // Combine HDR scene + HDR bloom in linear space,
        // then tone-map once at the end. Bloom intensity
        // is part of the bloom upsample now (post.intensity);
        // here we apply a final compositing weight.
        let hdr = scene + glow * 0.9;
        var color = aces_tonemap(hdr);
        // Crush blacks — 1999 Matrix chiaroscuro look. Pull
        // the toe to true black so the institutional green
        // sits on a void rather than a muddy near-black,
        // then a tiny gain to compensate for the floor lift.
        color = max(color - vec3<f32>(0.018, 0.018, 0.018), vec3<f32>(0.0));
        color = color * 1.05;
        return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    }
";

// Phosphor-persistence pass. Reads the freshly-drawn scene plus
// the previous frame's persisted buffer; emits max(scene,
// prev * decay). max() (not additive accumulation) is the
// phosphor model — bright glyphs persist and decay, dark
// regions don't runaway-brighten. Bind layout is identical to
// the composite pass (two textures + sampler), so it reuses
// composite_layout. Decay is a shader constant — no uniform,
// which sidesteps any post_buffer write-ordering question.
const POST_PERSISTENCE_SHADER_WGSL: &str = "
    @group(0) @binding(0)
    var scene_tex: texture_2d<f32>;

    @group(0) @binding(1)
    var prev_tex: texture_2d<f32>;

    @group(0) @binding(2)
    var persist_sampler: sampler;

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
    fn fs_persist(input: VertexOut) -> @location(0) vec4<f32> {
        let decay = 0.86;
        let scene = textureSample(scene_tex, persist_sampler, input.uv).rgb;
        let prev = textureSample(prev_tex, persist_sampler, input.uv).rgb;
        let persisted = max(scene, prev * decay);
        return vec4<f32>(persisted, 1.0);
    }
";

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
    // Mip-chain bloom: BLOOM_MIP_COUNT progressively-halved HDR
    // textures. The downsample chain writes 0..N (each from the
    // previous level), then the upsample chain blends additively from
    // N-1..0. Final bloom result lives in `bloom_views[0]`.
    _bloom_textures: Vec<wgpu::Texture>,
    bloom_views: Vec<wgpu::TextureView>,
    bloom_sizes: Vec<(u32, u32)>,
    // Phosphor-persistence ping-pong pair. Full-size HDR; one is read
    // as "previous frame", the other written as "this frame", swapped
    // via GpuRendererScaffold::persist_toggle. Recreated on resize,
    // which harmlessly resets the trail.
    _persist_a_texture: wgpu::Texture,
    persist_a_view: wgpu::TextureView,
    _persist_b_texture: wgpu::Texture,
    persist_b_view: wgpu::TextureView,
    _target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
}

pub struct GpuRendererScaffold {
    selected_adapter: AdapterSnapshot,
    device: wgpu::Device,
    queue: wgpu::Queue,
    glyph_pipeline: wgpu::RenderPipeline,
    /// First-level downsample: Karis luma weighting + soft-knee HDR
    /// threshold. Reads full-size persist_curr, writes bloom mip 0.
    bloom_first_pipeline: wgpu::RenderPipeline,
    /// Plain box downsample. Reads bloom mip i-1, writes bloom mip i.
    bloom_downsample_pipeline: wgpu::RenderPipeline,
    /// Tent-filter upsample with additive blend. Reads bloom mip i+1,
    /// blends additively into bloom mip i.
    bloom_upsample_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    persistence_pipeline: wgpu::RenderPipeline,
    /// Ping-pong selector for the two persist buffers. `false` → A is
    /// this frame's write target (B is last frame's read source);
    /// flipped at the end of every `draw_instanced_pass`.
    persist_toggle: bool,
    global_bind_group: wgpu::BindGroup,
    global_buffer: wgpu::Buffer,
    /// One small (32-byte) PostUniform buffer per bloom pass —
    /// `2 * BLOOM_MIP_COUNT - 1` entries: 1 first-downsample, then
    /// `BLOOM_MIP_COUNT - 1` plain downsamples, then `BLOOM_MIP_COUNT - 1`
    /// upsamples. Separate buffers are required because `queue.write_buffer`
    /// calls between render passes in the same submission all flush
    /// before any pass executes — only the *last* write would be
    /// observable if we reused a single buffer. Separate destinations
    /// sidestep that wgpu semantic entirely.
    bloom_post_buffers: Vec<wgpu::Buffer>,
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
            source: wgpu::ShaderSource::Wgsl(GLYPH_SHADER_WGSL.into()),
        });

        let bloom_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matrisaver-scaffold-bloom-shader"),
            source: wgpu::ShaderSource::Wgsl(BLOOM_SHADER_WGSL.into()),
        });

        let post_composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matrisaver-scaffold-post-composite-shader"),
            source: wgpu::ShaderSource::Wgsl(POST_COMPOSITE_SHADER_WGSL.into()),
        });

        let post_persistence_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matrisaver-scaffold-post-persistence-shader"),
            source: wgpu::ShaderSource::Wgsl(POST_PERSISTENCE_SHADER_WGSL.into()),
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
                time_pad: [0.0; 2],
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

        // One PostUniform buffer per bloom pass. 2*BLOOM_MIP_COUNT-1
        // because we have 1 first-downsample, (BLOOM_MIP_COUNT-1)
        // plain downsamples, and (BLOOM_MIP_COUNT-1) upsamples.
        let bloom_pass_count = (2 * BLOOM_MIP_COUNT - 1) as usize;
        let mut bloom_post_buffers: Vec<wgpu::Buffer> = Vec::with_capacity(bloom_pass_count);
        for _ in 0..bloom_pass_count {
            bloom_post_buffers.push(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("matrisaver-scaffold-bloom-post-buffer"),
                size: std::mem::size_of::<PostUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

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
                    format: HDR_FORMAT,
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

        // Helper closure to construct a bloom pipeline. Same vertex
        // state, same layout, varying only by entry point and blend
        // state (REPLACE for downsamples, additive for upsample).
        let make_bloom_pipeline =
            |label: &str, entry: &str, blend: wgpu::BlendState| -> wgpu::RenderPipeline {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&post_layout),
                    vertex: wgpu::VertexState {
                        module: &bloom_shader,
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
                        module: &bloom_shader,
                        entry_point: Some(entry),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: HDR_FORMAT,
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
            };

        let bloom_first_pipeline = make_bloom_pipeline(
            "matrisaver-scaffold-bloom-first-pipeline",
            "fs_first_downsample",
            wgpu::BlendState::REPLACE,
        );
        let bloom_downsample_pipeline = make_bloom_pipeline(
            "matrisaver-scaffold-bloom-downsample-pipeline",
            "fs_downsample",
            wgpu::BlendState::REPLACE,
        );
        // Additive blend (src + dst) on the upsample so each level's
        // contribution stacks into the larger mip below it.
        let bloom_upsample_pipeline = make_bloom_pipeline(
            "matrisaver-scaffold-bloom-upsample-pipeline",
            "fs_upsample",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        );

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

        // Persistence pipeline — same bind-group layout as composite
        // (two textures + sampler), different shader.
        let persistence_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("matrisaver-scaffold-persistence-pipeline"),
            layout: Some(&composite_layout),
            vertex: wgpu::VertexState {
                module: &post_persistence_shader,
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
                module: &post_persistence_shader,
                entry_point: Some("fs_persist"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
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
            bloom_first_pipeline,
            bloom_downsample_pipeline,
            bloom_upsample_pipeline,
            composite_pipeline,
            persistence_pipeline,
            persist_toggle: false,
            global_bind_group,
            global_buffer,
            bloom_post_buffers,
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
        let mip0_width = (width / factor).max(1);
        let mip0_height = (height / factor).max(1);

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
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let scene_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Bloom mip chain — BLOOM_MIP_COUNT progressively halved HDR
        // textures starting at (mip0_width, mip0_height).
        let mut bloom_textures: Vec<wgpu::Texture> = Vec::with_capacity(BLOOM_MIP_COUNT as usize);
        let mut bloom_views: Vec<wgpu::TextureView> = Vec::with_capacity(BLOOM_MIP_COUNT as usize);
        let mut bloom_sizes: Vec<(u32, u32)> = Vec::with_capacity(BLOOM_MIP_COUNT as usize);
        for level in 0..BLOOM_MIP_COUNT {
            let level_width = (mip0_width >> level).max(1);
            let level_height = (mip0_height >> level).max(1);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("matrisaver-scaffold-bloom-mip"),
                size: wgpu::Extent3d {
                    width: level_width,
                    height: level_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            bloom_textures.push(texture);
            bloom_views.push(view);
            bloom_sizes.push((level_width, level_height));
        }

        let persist_a_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("matrisaver-scaffold-persist-a"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let persist_a_view = persist_a_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let persist_b_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("matrisaver-scaffold-persist-b"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let persist_b_view = persist_b_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Display surface (LDR — the composite tone-maps to 0..1 before
        // writing here).
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
            _bloom_textures: bloom_textures,
            bloom_views,
            bloom_sizes,
            _persist_a_texture: persist_a_texture,
            persist_a_view,
            _persist_b_texture: persist_b_texture,
            persist_b_view,
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
        load_op: wgpu::LoadOp<wgpu::Color>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("matrisaver-scaffold-post-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: load_op,
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
        time: f32,
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
                time_pad: [time, 0.0],
                glyph_tint: [glyph_tint[0], glyph_tint[1], glyph_tint[2], 1.0],
                style_params,
            }),
        );

        let source_texel = [
            1.0 / self.surface_size.0.max(1) as f32,
            1.0 / self.surface_size.1.max(1) as f32,
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

        // Phosphor-persistence pass. `curr` is this frame's write
        // target; `prev` is last frame's persisted buffer (read).
        // Everything downstream (prefilter, composite) then reads
        // `curr` instead of the raw scene, so the glow and the final
        // image both carry the decayed trail.
        let persist_toggle = self.persist_toggle;
        let (persist_curr_view, persist_prev_view) = if persist_toggle {
            (
                &self.render_targets.persist_b_view,
                &self.render_targets.persist_a_view,
            )
        } else {
            (
                &self.render_targets.persist_a_view,
                &self.render_targets.persist_b_view,
            )
        };
        let persistence_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-persistence-bind-group"),
            layout: &self.persistence_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.render_targets.scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(persist_prev_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                },
            ],
        });
        Self::draw_post_pass(
            &mut encoder,
            persist_curr_view,
            &self.persistence_pipeline,
            &persistence_bind_group,
            &self.screen_vertex_buffer,
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
        );

        // -------- Mip-chain bloom --------
        //
        // Each bloom pass owns its own uniform buffer in
        // `self.bloom_post_buffers`. Writing N values to N distinct
        // buffers sidesteps wgpu's "all queue writes flush before any
        // submit" semantic that would otherwise cause every pass to
        // see only the last write's values.
        let bloom_mip_count = BLOOM_MIP_COUNT as usize;
        let downsample_count = bloom_mip_count - 1; // mips 1..N
        let upsample_count = bloom_mip_count - 1; // mips N-2..0

        // Index helpers into bloom_post_buffers / per-pass bind groups.
        let first_idx: usize = 0;
        let downsample_idx = |i: usize| 1 + i; // i in 0..downsample_count
        let upsample_idx = |i: usize| 1 + downsample_count + i; // i in 0..upsample_count

        // Stage 1: first downsample (Karis + soft-knee HDR threshold).
        // Reads full-size persist_curr; texel = full-surface texel.
        self.queue.write_buffer(
            &self.bloom_post_buffers[first_idx],
            0,
            bytemuck::bytes_of(&PostUniform {
                texel_size: source_texel,
                // HDR threshold: heads emit ~4.0 luminance, trails near
                // 1.0. Threshold 1.0 captures the genuine over-bright
                // tips and ignores the trails so bloom is anchored on
                // heads rather than smeared over everything.
                threshold: 1.0,
                intensity: 1.0,
                mode: 1.0,
                _pad0: 0.0,
                _pad1: 0.0,
                _pad2: 0.0,
            }),
        );
        let bg_first = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-bloom-first-bind-group"),
            layout: &self.bloom_first_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(persist_curr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.bloom_post_buffers[first_idx].as_entire_binding(),
                },
            ],
        });
        Self::draw_post_pass(
            &mut encoder,
            &self.render_targets.bloom_views[0],
            &self.bloom_first_pipeline,
            &bg_first,
            &self.screen_vertex_buffer,
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
        );

        // Stage 2: plain downsamples for mips 1..N.
        // (Bind groups must outlive the encoder.submit, so we collect
        // them into a Vec rather than letting them drop mid-loop.)
        let mut down_bind_groups: Vec<wgpu::BindGroup> = Vec::with_capacity(downsample_count);
        for i in 0..downsample_count {
            let src_level = i; // we read mip[i], write mip[i+1]
            let (src_w, src_h) = self.render_targets.bloom_sizes[src_level];
            let texel = [1.0 / src_w.max(1) as f32, 1.0 / src_h.max(1) as f32];
            self.queue.write_buffer(
                &self.bloom_post_buffers[downsample_idx(i)],
                0,
                bytemuck::bytes_of(&PostUniform {
                    texel_size: texel,
                    threshold: 0.0,
                    intensity: 1.0,
                    mode: 0.0,
                    _pad0: 0.0,
                    _pad1: 0.0,
                    _pad2: 0.0,
                }),
            );
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("matrisaver-scaffold-bloom-down-bind-group"),
                layout: &self.bloom_downsample_pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &self.render_targets.bloom_views[src_level],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.bloom_post_buffers[downsample_idx(i)].as_entire_binding(),
                    },
                ],
            });
            down_bind_groups.push(bg);
        }
        for (i, bind_group) in down_bind_groups.iter().enumerate() {
            let dst_level = i + 1;
            Self::draw_post_pass(
                &mut encoder,
                &self.render_targets.bloom_views[dst_level],
                &self.bloom_downsample_pipeline,
                bind_group,
                &self.screen_vertex_buffer,
                wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
            );
        }

        // Stage 3: upsamples — for each pair (src=i+1, dst=i) from
        // deepest to shallowest, tent-filter src and additively blend
        // onto dst. LoadOp::Load preserves dst's downsample content;
        // additive blend (pipeline state) sums new + existing.
        let mut up_bind_groups: Vec<wgpu::BindGroup> = Vec::with_capacity(upsample_count);
        for i in 0..upsample_count {
            // We're going to process pairs in reverse: src=N-1->dst=N-2,
            // src=N-2->dst=N-3, ..., src=1->dst=0. Loop index i runs
            // 0..upsample_count, corresponding to dst = upsample_count-1-i.
            let dst_level = upsample_count - 1 - i;
            let src_level = dst_level + 1;
            let (src_w, src_h) = self.render_targets.bloom_sizes[src_level];
            let texel = [1.0 / src_w.max(1) as f32, 1.0 / src_h.max(1) as f32];
            self.queue.write_buffer(
                &self.bloom_post_buffers[upsample_idx(i)],
                0,
                bytemuck::bytes_of(&PostUniform {
                    texel_size: texel,
                    threshold: 0.0,
                    // Per-step intensity — gentle, additive across
                    // levels. The cumulative effect is what builds the
                    // wide soft halo.
                    intensity: 0.85,
                    mode: 2.0,
                    _pad0: 0.0,
                    _pad1: 0.0,
                    _pad2: 0.0,
                }),
            );
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("matrisaver-scaffold-bloom-up-bind-group"),
                layout: &self.bloom_upsample_pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &self.render_targets.bloom_views[src_level],
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.bloom_post_buffers[upsample_idx(i)].as_entire_binding(),
                    },
                ],
            });
            up_bind_groups.push(bg);
        }
        for (i, bind_group) in up_bind_groups.iter().enumerate() {
            let dst_level = upsample_count - 1 - i;
            Self::draw_post_pass(
                &mut encoder,
                &self.render_targets.bloom_views[dst_level],
                &self.bloom_upsample_pipeline,
                bind_group,
                &self.screen_vertex_buffer,
                wgpu::LoadOp::Load,
            );
        }

        // -------- Composite (HDR scene + HDR bloom → tone-mapped LDR) --------
        let composite_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matrisaver-scaffold-composite-bind-group"),
            layout: &self.composite_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(persist_curr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self.render_targets.bloom_views[0],
                    ),
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
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
        );

        self.queue.submit([encoder.finish()]);

        // Ping-pong: this frame's `curr` becomes next frame's `prev`.
        self.persist_toggle = !persist_toggle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Validates each embedded shader string parses as WGSL. naga is the
    // same WGSL frontend wgpu uses internally, so a parse failure here
    // would also be a `create_shader_module` failure at runtime — but
    // catching it at `cargo test` time means we hit it locally (and on
    // ci.yml's gates) instead of at first `/s` launch.
    //
    // Lineage: a stray `"` inside a WGSL comment earlier in this file's
    // history caused a "prefix `green` is unknown" parse error that only
    // showed up at GPU init. This test class would have caught that
    // before commit, even on a machine with no GPU.
    fn assert_wgsl_parses(label: &str, source: &str) {
        if let Err(error) = naga::front::wgsl::parse_str(source) {
            panic!(
                "WGSL parse failed for {label}: {}",
                error.emit_to_string(source)
            );
        }
    }

    #[test]
    fn glyph_shader_parses() {
        assert_wgsl_parses("GLYPH_SHADER_WGSL", GLYPH_SHADER_WGSL);
    }

    #[test]
    fn bloom_shader_parses() {
        assert_wgsl_parses("BLOOM_SHADER_WGSL", BLOOM_SHADER_WGSL);
    }

    #[test]
    fn post_composite_shader_parses() {
        assert_wgsl_parses("POST_COMPOSITE_SHADER_WGSL", POST_COMPOSITE_SHADER_WGSL);
    }

    #[test]
    fn post_persistence_shader_parses() {
        assert_wgsl_parses("POST_PERSISTENCE_SHADER_WGSL", POST_PERSISTENCE_SHADER_WGSL);
    }

    // Cross-check that the Rust-side uniform struct sizes match what
    // the WGSL `struct` declarations imply. The const-asserts at the
    // top of this file lock the Rust side; this test locks the WGSL
    // side. If someone adds a field to one and forgets the other, the
    // size assertion below trips before pipeline creation does.
    #[test]
    fn uniform_struct_sizes_match_wgsl_declarations() {
        // GlobalUniform: vec2 + vec2 + vec4 + vec4 = 8+8+16+16 = 48 bytes.
        assert_eq!(std::mem::size_of::<GlobalUniform>(), 48);

        // PostUniform: vec2 + f32*5 + vec3-of-pad-as-3-scalars
        //            = 8 + 4 + 4 + 4 + 4 + 4 + 4 = 32 bytes.
        // Critical: the three trailing `_padN: f32` fields are *not*
        // `_pad: vec3<f32>` — vec3 would force 16-byte alignment and
        // bloat the struct to 48 bytes, mismatching wgpu's
        // min_binding_size. See the comment on PostUniform itself.
        assert_eq!(std::mem::size_of::<PostUniform>(), 32);
    }
}
