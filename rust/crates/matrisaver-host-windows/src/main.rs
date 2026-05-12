// update_check pulls in reqwest + serde, both declared under
// [target.'cfg(windows)'.dependencies] in Cargo.toml — so the module
// can only exist on Windows. The CI matrix builds this binary on
// ubuntu-latest and macos-latest too, where dropping the cfg gate
// would break the build with "cannot find module reqwest".
#[cfg(target_os = "windows")]
mod update_check;
#[cfg(target_os = "windows")]
use update_check::UpdateCheckResult;

// Settings dialog for /c mode — pulls in eframe + rfd, same cfg-gate
// reasoning as update_check above. On Linux/macOS /c is a no-op stub.
#[cfg(target_os = "windows")]
mod config_dialog;

use matrisaver_core::config::{GlowQuality, Pipeline};
use matrisaver_core::gpu::GpuSelectionOptions;
use matrisaver_core::storage;
use matrisaver_core::CoreRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrMode {
    Screensaver,
    Configure,
    Preview { parent_hwnd: Option<u64> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostAction {
    RunScreensaver,
    OpenConfig,
    RunPreview { parent_hwnd: u64 },
}

fn parse_scr_mode(args: &[String]) -> ScrMode {
    for (index, arg) in args.iter().enumerate() {
        if arg.eq_ignore_ascii_case("/c") || arg.eq_ignore_ascii_case("-c") {
            return ScrMode::Configure;
        }
        if arg.eq_ignore_ascii_case("/s") || arg.eq_ignore_ascii_case("-s") {
            return ScrMode::Screensaver;
        }
        if arg.eq_ignore_ascii_case("/p") || arg.eq_ignore_ascii_case("-p") {
            let hwnd = args.get(index + 1).and_then(|value| parse_hwnd(value));
            return ScrMode::Preview { parent_hwnd: hwnd };
        }
        if let Some(raw) = arg.strip_prefix("/p:").or_else(|| arg.strip_prefix("-p:")) {
            return ScrMode::Preview {
                parent_hwnd: parse_hwnd(raw),
            };
        }
    }

    ScrMode::Screensaver
}

fn parse_hwnd(raw: &str) -> Option<u64> {
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    raw.parse::<u64>().ok()
}

fn build_host_action(mode: ScrMode) -> Result<HostAction, &'static str> {
    match mode {
        ScrMode::Screensaver => Ok(HostAction::RunScreensaver),
        ScrMode::Configure => Ok(HostAction::OpenConfig),
        ScrMode::Preview {
            parent_hwnd: Some(hwnd),
        } => Ok(HostAction::RunPreview { parent_hwnd: hwnd }),
        ScrMode::Preview { parent_hwnd: None } => Err("Preview mode requires a valid parent HWND."),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let list_adapters = has_flag(&args, "--list-adapters");
    let enable_gpu_scaffold = has_flag(&args, "--gpu-scaffold");
    let gpu_selection = parse_gpu_selection(&args);
    let benchmark_frames = parse_benchmark_frames(&args);
    let lifecycle_frames = parse_lifecycle_frames(&args);
    let lifecycle_trace_file =
        parse_option_value(&args, "--lifecycle-trace-file").map(str::to_owned);
    let mode = parse_scr_mode(&args);
    let action = match build_host_action(mode) {
        Ok(action) => action,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let mut runtime_settings = storage::load_settings_or_default(None);
    if let Some(glow_quality) =
        parse_option_value(&args, "--glow-quality").and_then(parse_glow_quality)
    {
        runtime_settings.glow_quality = glow_quality;
    }
    let mut runtime = CoreRuntime::new(runtime_settings);
    runtime.set_gpu_selection(gpu_selection.clone());
    let width = parse_option_value(&args, "--width")
        .and_then(parse_u32)
        .unwrap_or(1920)
        .max(1);
    let height = parse_option_value(&args, "--height")
        .and_then(parse_u32)
        .unwrap_or(1080)
        .max(1);
    runtime.set_surface_size(width, height);
    if list_adapters {
        for adapter in runtime.adapter_snapshots() {
            println!(
                "ADAPTER host=windows name={} backend={} device_type={}",
                adapter.name, adapter.backend, adapter.device_type
            );
        }
    }

    match action {
        HostAction::OpenConfig => {
            run_config_mode(&args);
        }
        HostAction::RunPreview { .. } | HostAction::RunScreensaver => {
            if let Some(frames) = benchmark_frames {
                if enable_gpu_scaffold {
                    if let Err(error) = runtime.enable_gpu_scaffold() {
                        eprintln!("Failed to enable GPU scaffold: {error}");
                    }
                }
                let mode_key = match action {
                    HostAction::RunPreview { .. } => "preview",
                    _ => "screensaver",
                };
                run_runtime_benchmark("windows", mode_key, &mut runtime, frames, width, height);
            } else if let Err(error) = run_windows_lifecycle(
                action,
                runtime,
                width,
                height,
                gpu_selection,
                lifecycle_frames,
                lifecycle_trace_file,
            ) {
                eprintln!("Windows host lifecycle error: {error}");
                std::process::exit(3);
            }
        }
    }
}

fn run_windows_lifecycle(
    action: HostAction,
    runtime: CoreRuntime,
    width: u32,
    height: u32,
    gpu_selection: GpuSelectionOptions,
    lifecycle_frames: Option<u32>,
    lifecycle_trace_file: Option<String>,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        windows_host::run(
            action,
            runtime,
            width,
            height,
            gpu_selection,
            lifecycle_frames,
            lifecycle_trace_file,
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (
            action,
            runtime,
            width,
            height,
            gpu_selection,
            lifecycle_frames,
            lifecycle_trace_file,
        );
        Err(
            "Real .scr lifecycle requires building/running this host on Windows. Use --benchmark-frames for non-Windows benchmarking."
                .to_owned(),
        )
    }
}

/// Decide between the new egui settings dialog and the legacy stdout
/// CLI handler. Display Properties' "Settings…" button passes only the
/// `/c` token, so we open the dialog in that case. Any CLI override
/// flag (or the explicit `--headless-config` opt-out) forces the
/// headless path — preserves scripted/CI workflows that pass
/// `--variant`, `--char-size`, etc. on the command line.
fn run_config_mode(args: &[String]) {
    let has_cli_overrides = parse_option_value(args, "--variant").is_some()
        || parse_option_value(args, "--pipeline").is_some()
        || parse_option_value(args, "--glow-quality").is_some()
        || parse_option_value(args, "--char-size").is_some()
        || parse_option_value(args, "--update-check-repo").is_some()
        || has_flag(args, "--overlay")
        || has_flag(args, "--no-overlay")
        || has_flag(args, "--performance")
        || has_flag(args, "--no-performance")
        || has_flag(args, "--multi-monitor")
        || has_flag(args, "--single-monitor")
        || has_flag(args, "--skip-update-check")
        || has_flag(args, "--headless-config");

    #[cfg(target_os = "windows")]
    if !has_cli_overrides {
        if let Err(err) = config_dialog::open() {
            eprintln!("Settings dialog failed: {err}");
            std::process::exit(4);
        }
        return;
    }

    run_config_mode_headless(args);
}

/// Pre-egui CLI handler. Kept for scripting, CI tests, and non-Windows
/// builds. Display Properties never reaches this path now.
fn run_config_mode_headless(args: &[String]) {
    let mut settings = storage::load_settings_or_default(None);

    if let Some(variant) = parse_option_value(args, "--variant") {
        settings.variant = variant.to_owned();
    }
    if let Some(pipeline) = parse_option_value(args, "--pipeline").and_then(parse_pipeline) {
        settings.pipeline = pipeline;
    }
    if let Some(glow) = parse_option_value(args, "--glow-quality").and_then(parse_glow_quality) {
        settings.glow_quality = glow;
    }
    if has_flag(args, "--overlay") {
        settings.overlay_enabled = true;
    }
    if has_flag(args, "--no-overlay") {
        settings.overlay_enabled = false;
    }
    if has_flag(args, "--performance") {
        settings.performance_mode = true;
    }
    if has_flag(args, "--no-performance") {
        settings.performance_mode = false;
    }
    if has_flag(args, "--multi-monitor") {
        settings.multi_monitor = true;
    }
    if has_flag(args, "--single-monitor") {
        settings.multi_monitor = false;
    }
    if let Some(char_size) = parse_option_value(args, "--char-size").and_then(parse_u16) {
        settings.char_size = char_size;
    }

    let sanitized = settings.sanitize();
    match storage::save_settings(&sanitized, None) {
        Ok(()) => {
            println!(
                "Saved settings variant={} pipeline={} glow_quality={} overlay={} performance={} multi_monitor={} char_size={}",
                sanitized.variant,
                sanitized.pipeline.key(),
                glow_quality_key(sanitized.glow_quality),
                sanitized.overlay_enabled,
                sanitized.performance_mode,
                sanitized.multi_monitor,
                sanitized.char_size
            );
        }
        Err(error) => {
            eprintln!("Failed to save settings: {error}");
        }
    }

    // Update check — non-fatal, runs only in /c mode. Windows-only:
    // the surrounding /c mode and its update-check transport (reqwest
    // + SChannel) don't exist on the Linux/macOS host stubs.
    #[cfg(target_os = "windows")]
    if !has_flag(args, "--skip-update-check") {
        let repo_override = parse_option_value(args, "--update-check-repo");
        match update_check::check(repo_override) {
            UpdateCheckResult::UpToDate { current } => {
                println!("UPDATE status=up-to-date current={current}");
            }
            UpdateCheckResult::Available {
                current,
                latest,
                msi_url,
                changelog_url,
            } => {
                print!(
                    "UPDATE status=available current={current} latest={latest} msi_url={msi_url}"
                );
                if let Some(url) = changelog_url {
                    print!(" changelog_url={url}");
                }
                println!();
            }
            UpdateCheckResult::Failed(err) => {
                println!("UPDATE status=failed reason={}", err.replace(' ', "_"));
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = args;
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn parse_option_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(&format!("{key}=")) {
            return Some(value);
        }
        if arg == key {
            return args.get(index + 1).map(String::as_str);
        }
    }
    None
}

fn parse_pipeline(raw: &str) -> Option<Pipeline> {
    match raw {
        "opengl" => Some(Pipeline::OpenGl),
        "cpu" => Some(Pipeline::Cpu),
        "cpu_glow" => Some(Pipeline::CpuGlow),
        _ => None,
    }
}

fn parse_glow_quality(raw: &str) -> Option<GlowQuality> {
    match raw {
        "low" => Some(GlowQuality::Low),
        "balanced" => Some(GlowQuality::Balanced),
        "high" => Some(GlowQuality::High),
        _ => None,
    }
}

fn parse_gpu_selection(args: &[String]) -> GpuSelectionOptions {
    let explicit_backend = parse_option_value(args, "--wgpu-backend")
        .or_else(|| parse_option_value(args, "--backend"))
        .map(str::to_owned);
    let explicit_adapter = parse_option_value(args, "--wgpu-adapter-name")
        .or_else(|| parse_option_value(args, "--adapter-name"))
        .map(str::to_owned);
    GpuSelectionOptions::from_env().with_overrides(explicit_backend, explicit_adapter)
}

fn parse_benchmark_frames(args: &[String]) -> Option<u32> {
    parse_option_value(args, "--benchmark-frames")
        .and_then(parse_u32)
        .map(|value| value.max(1))
}

fn parse_lifecycle_frames(args: &[String]) -> Option<u32> {
    parse_option_value(args, "--lifecycle-frames")
        .and_then(parse_u32)
        .map(|value| value.max(1))
}

fn perf_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn glow_quality_key(value: GlowQuality) -> &'static str {
    match value {
        GlowQuality::Low => "low",
        GlowQuality::Balanced => "balanced",
        GlowQuality::High => "high",
    }
}

fn parse_u16(raw: &str) -> Option<u16> {
    raw.parse::<u16>().ok()
}

fn parse_u32(raw: &str) -> Option<u32> {
    raw.parse::<u32>().ok()
}

fn run_runtime_benchmark(
    host: &str,
    mode: &str,
    runtime: &mut CoreRuntime,
    frames: u32,
    width: u32,
    height: u32,
) {
    for _ in 0..frames {
        runtime.tick_profiled(1.0 / 60.0);
    }

    if let Some(summary) = runtime.performance_summary() {
        let adapter = runtime.selected_adapter_snapshot();
        let backend = adapter.map_or("none", |value| value.backend.as_str());
        let adapter_name = adapter.map_or("none", |value| value.name.as_str());
        let device_type = adapter.map_or("none", |value| value.device_type.as_str());
        println!(
            "PERF host={} mode={} glow_quality={} selected_backend={} selected_adapter={} selected_device_type={} width={} height={} frames={} avg_total_ms={:.4} p95_total_ms={:.4} avg_update_ms={:.4} avg_draw_ms={:.4} avg_post_ms={:.4} avg_fps={:.2}",
            host,
            mode,
            glow_quality_key(runtime.settings().glow_quality),
            perf_token(backend),
            perf_token(adapter_name),
            perf_token(device_type),
            width,
            height,
            summary.frame_count,
            summary.avg_total_ms,
            summary.p95_total_ms,
            summary.avg_update_ms,
            summary.avg_draw_ms,
            summary.avg_post_process_ms,
            summary.avg_fps
        );
    }
}

#[cfg(target_os = "windows")]
mod windows_host {
    use super::HostAction;
    use matrisaver_core::gpu::GpuSelectionOptions;
    use matrisaver_core::CoreRuntime;
    use raw_window_handle::{
        RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
    };
    use std::fs::OpenOptions;
    use std::io::{BufWriter, Write};
    use std::iter;
    use std::num::NonZeroIsize;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{BeginPaint, EndPaint, UpdateWindow, PAINTSTRUCT};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
        GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, LoadCursorW,
        PostQuitMessage, RegisterClassW, SetTimer, SetWindowLongPtrW, ShowWindow, TranslateMessage,
        CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, IDC_ARROW, MSG, SM_CXSCREEN,
        SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        SW_SHOW, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
        WM_MOUSEMOVE, WM_NCDESTROY, WM_PAINT, WM_RBUTTONDOWN, WM_SIZE, WM_TIMER, WNDCLASSW,
        WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
    };

    struct WindowPresenter {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_config: wgpu::SurfaceConfiguration,
        selected_adapter: matrisaver_core::gpu::AdapterSnapshot,
        present_pipeline: wgpu::RenderPipeline,
        present_bind_group_layout: wgpu::BindGroupLayout,
        present_sampler: wgpu::Sampler,
    }

    impl WindowPresenter {
        fn new(
            hwnd: HWND,
            width: u32,
            height: u32,
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

            let hwnd = NonZeroIsize::new(hwnd as isize)
                .ok_or_else(|| "Cannot create wgpu surface for null HWND".to_owned())?;
            let mut window_handle = Win32WindowHandle::new(hwnd);
            window_handle.hinstance =
                NonZeroIsize::new(unsafe { GetModuleHandleW(std::ptr::null()) as isize });
            let surface_target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                raw_window_handle: RawWindowHandle::Win32(window_handle),
            };
            let surface = unsafe { instance.create_surface_unsafe(surface_target) }
                .map_err(|error| format!("Failed to create wgpu surface: {error}"))?;

            let mut adapters = pollster::block_on(instance.enumerate_adapters(requested_backends));
            adapters.retain(|adapter| adapter.is_surface_supported(&surface));
            if adapters.is_empty() {
                return Err("No surface-compatible adapters available for this window.".to_owned());
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
                            "No surface adapter matched hint '{adapter_hint}'. Available adapters: {}",
                            available.join(", ")
                        )
                    })?
            } else {
                let mut chosen = adapters.remove(0);
                let mut chosen_rank = adapter_rank(chosen.get_info().device_type);
                for adapter in adapters {
                    let rank = adapter_rank(adapter.get_info().device_type);
                    if rank < chosen_rank {
                        chosen = adapter;
                        chosen_rank = rank;
                    }
                }
                chosen
            };
            let adapter_info = adapter.get_info();
            let selected_adapter = matrisaver_core::gpu::AdapterSnapshot {
                name: adapter_info.name.clone(),
                backend: format!("{:?}", adapter_info.backend),
                device_type: format!("{:?}", adapter_info.device_type),
            };
            eprintln!(
                "SURFACE host=windows backend={:?} adapter={} device_type={:?}",
                adapter_info.backend, adapter_info.name, adapter_info.device_type
            );

            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                    .map_err(|error| format!("request_device failed: {error}"))?;

            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|candidate| {
                    matches!(
                        candidate,
                        wgpu::TextureFormat::Bgra8UnormSrgb
                            | wgpu::TextureFormat::Bgra8Unorm
                            | wgpu::TextureFormat::Rgba8UnormSrgb
                            | wgpu::TextureFormat::Rgba8Unorm
                    )
                })
                .or_else(|| caps.formats.first().copied())
                .ok_or_else(|| "Surface reported no supported formats".to_owned())?;
            let present_mode = caps
                .present_modes
                .iter()
                .copied()
                .find(|value| *value == wgpu::PresentMode::Fifo)
                .or_else(|| caps.present_modes.first().copied())
                .ok_or_else(|| "Surface reported no supported present modes".to_owned())?;
            let alpha_mode = caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto);

            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: width.max(1),
                height: height.max(1),
                present_mode,
                desired_maximum_frame_latency: 2,
                alpha_mode,
                view_formats: vec![],
            };
            surface.configure(&device, &surface_config);

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("matrisaver-window-present-shader"),
                source: wgpu::ShaderSource::Wgsl(
                    "
                    @group(0) @binding(0)
                    var scene_tex: texture_2d<f32>;

                    @group(0) @binding(1)
                    var scene_sampler: sampler;

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
                        out.uv = pos * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
                        return out;
                    }

                    @fragment
                    fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
                        let color = textureSample(scene_tex, scene_sampler, in.uv).rgb;
                        return vec4<f32>(color, 1.0);
                    }
                    "
                    .into(),
                ),
            });

            let present_bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("matrisaver-window-present-bind-group-layout"),
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
            let present_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("matrisaver-window-present-sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("matrisaver-window-present-pipeline-layout"),
                bind_group_layouts: &[&present_bind_group_layout],
                immediate_size: 0,
            });
            let present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("matrisaver-window-present-pipeline"),
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

            Ok(Self {
                surface,
                device,
                queue,
                surface_config,
                selected_adapter,
                present_pipeline,
                present_bind_group_layout,
                present_sampler,
            })
        }

        fn selected_adapter_snapshot(&self) -> matrisaver_core::gpu::AdapterSnapshot {
            self.selected_adapter.clone()
        }

        fn shared_device(&self) -> wgpu::Device {
            self.device.clone()
        }

        fn shared_queue(&self) -> wgpu::Queue {
            self.queue.clone()
        }

        fn resize(&mut self, width: u32, height: u32) {
            self.surface_config.width = width.max(1);
            self.surface_config.height = height.max(1);
            self.surface.configure(&self.device, &self.surface_config);
        }

        fn render(&mut self, runtime: &CoreRuntime) -> Result<(), String> {
            let frame = match self.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                    self.surface.configure(&self.device, &self.surface_config);
                    return Ok(());
                }
                Err(wgpu::SurfaceError::Timeout) => {
                    return Ok(());
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    return Err("wgpu surface out of memory".to_owned());
                }
                Err(wgpu::SurfaceError::Other) => {
                    return Ok(());
                }
            };

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let scene_view = runtime.gpu_scaffold_output_view().ok_or_else(|| {
                "Core scaffold output was not initialized for lifecycle mode".to_owned()
            })?;
            let present_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("matrisaver-window-present-bind-group"),
                layout: &self.present_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(scene_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.present_sampler),
                    },
                ],
            });

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("matrisaver-window-encoder"),
                });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("matrisaver-window-render-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
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
                pass.set_pipeline(&self.present_pipeline);
                pass.set_bind_group(0, &present_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }

            self.queue.submit([encoder.finish()]);
            frame.present();
            Ok(())
        }
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HostWindowMode {
        Screensaver,
        Preview,
    }

    // Standard .scr convention: tolerate a few pixels of cursor jitter
    // and ignore input for a brief grace period after launch. Without
    // these, optical-mouse jitter or a residual key release from
    // launching the screensaver dismisses it before any frames render.
    const JITTER_THRESHOLD_PX: i32 = 4;
    const INPUT_GRACE_MS: u128 = 500;

    struct HostState {
        runtime: CoreRuntime,
        mode: HostWindowMode,
        presenter: Option<WindowPresenter>,
        last_cursor: Option<(i32, i32)>,
        launch_instant: std::time::Instant,
        lifecycle_frames_left: Option<u32>,
        lifecycle_trace: Option<BufWriter<std::fs::File>>,
    }

    pub fn run(
        action: HostAction,
        runtime: CoreRuntime,
        width: u32,
        height: u32,
        gpu_selection: GpuSelectionOptions,
        lifecycle_frames: Option<u32>,
        lifecycle_trace_file: Option<String>,
    ) -> Result<(), String> {
        let window_mode = match action {
            HostAction::RunScreensaver => HostWindowMode::Screensaver,
            HostAction::RunPreview { .. } => HostWindowMode::Preview,
            HostAction::OpenConfig => return Ok(()),
        };

        let class_name = to_wide("MatriSaverScrHostWindow");
        let window_name = to_wide("MatriSaver");
        let mut trace_writer = if let Some(path) = lifecycle_trace_file.as_deref() {
            if let Some(parent) = Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        format!(
                            "Failed to create lifecycle trace directory '{}': {error}",
                            parent.display()
                        )
                    })?;
                }
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|error| {
                    format!("Failed to open lifecycle trace file '{path}': {error}")
                })?;
            let mut writer = BufWriter::new(file);
            writeln!(writer).map_err(|error| {
                format!("Failed to initialize lifecycle trace file '{path}': {error}")
            })?;
            writeln!(
                writer,
                "LIFECYCLE host=windows mode={:?} width={} height={} backend={} adapter_hint={}",
                window_mode,
                width,
                height,
                gpu_selection.backend.as_deref().unwrap_or("all"),
                gpu_selection.adapter_name.as_deref().unwrap_or("auto")
            )
            .map_err(|error| {
                format!("Failed to initialize lifecycle trace file '{path}': {error}")
            })?;
            writer.flush().map_err(|error| {
                format!("Failed to flush lifecycle trace file '{path}': {error}")
            })?;
            Some(writer)
        } else {
            None
        };

        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            if instance.is_null() {
                return Err("GetModuleHandleW failed".to_owned());
            }

            let wnd = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class_name.as_ptr(),
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                ..std::mem::zeroed()
            };
            if RegisterClassW(&wnd) == 0 {
                return Err("RegisterClassW failed".to_owned());
            }

            let (style, ex_style, parent, x, y, w, h) = match action {
                HostAction::RunPreview { parent_hwnd } => {
                    let parent = parent_hwnd as usize as HWND;
                    let mut rect: RECT = std::mem::zeroed();
                    let mut w = width as i32;
                    let mut h = height as i32;
                    if GetClientRect(parent, &mut rect) != 0 {
                        w = (rect.right - rect.left).max(1);
                        h = (rect.bottom - rect.top).max(1);
                    }
                    (
                        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                        0,
                        parent,
                        0,
                        0,
                        w,
                        h,
                    )
                }
                HostAction::RunScreensaver => {
                    let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
                    let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
                    let w = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1);
                    let h = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1);
                    (
                        WS_POPUP | WS_VISIBLE,
                        WS_EX_TOPMOST,
                        std::ptr::null_mut(),
                        x,
                        y,
                        w,
                        h,
                    )
                }
                HostAction::OpenConfig => unreachable!(),
            };

            let state = Box::new(HostState {
                runtime,
                mode: window_mode,
                presenter: None,
                last_cursor: None,
                launch_instant: std::time::Instant::now(),
                lifecycle_frames_left: lifecycle_frames,
                lifecycle_trace: trace_writer.take(),
            });
            let state_ptr = Box::into_raw(state);
            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                window_name.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                parent,
                std::ptr::null_mut(),
                instance,
                state_ptr.cast(),
            );
            if hwnd.is_null() {
                drop(Box::from_raw(state_ptr));
                return Err("CreateWindowExW failed".to_owned());
            }

            let initial_width = (w as u32).max(1);
            let initial_height = (h as u32).max(1);
            (*state_ptr)
                .runtime
                .set_surface_size(initial_width, initial_height);
            if matches!(action, HostAction::RunScreensaver) {
                let primary_w = GetSystemMetrics(SM_CXSCREEN).max(1) as u32;
                let primary_h = GetSystemMetrics(SM_CYSCREEN).max(1) as u32;
                let primary_x = (0 - x).max(0) as u32;
                let primary_y = (0 - y).max(0) as u32;
                (*state_ptr)
                    .runtime
                    .set_overlay_reference_rect(primary_x, primary_y, primary_w, primary_h);
            } else {
                (*state_ptr).runtime.clear_overlay_reference_rect();
            }
            let presenter =
                match WindowPresenter::new(hwnd, initial_width, initial_height, &gpu_selection) {
                    Ok(presenter) => presenter,
                    Err(error) => {
                        DestroyWindow(hwnd);
                        return Err(error);
                    }
                };
            if let Err(error) = (*state_ptr).runtime.enable_gpu_scaffold_with_shared_device(
                presenter.selected_adapter_snapshot(),
                presenter.shared_device(),
                presenter.shared_queue(),
            ) {
                DestroyWindow(hwnd);
                return Err(error);
            }
            (*state_ptr).presenter = Some(presenter);

            if SetTimer(hwnd, 1, 16, None) == 0 {
                DestroyWindow(hwnd);
                return Err("SetTimer failed".to_owned());
            }

            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        Ok(())
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        _wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => {
                let create = &*(lparam as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                return 0;
            }
            WM_SIZE => {
                if let Some(state) = state_mut(hwnd) {
                    let width = (lparam as u32) & 0xFFFF;
                    let height = ((lparam as u32) >> 16) & 0xFFFF;
                    let width = width.max(1);
                    let height = height.max(1);
                    state.runtime.set_surface_size(width, height);
                    if let Some(presenter) = &mut state.presenter {
                        presenter.resize(width, height);
                    }
                }
                return 0;
            }
            WM_TIMER => {
                if let Some(state) = state_mut(hwnd) {
                    state.runtime.tick(1.0 / 60.0);
                    if let Some(presenter) = &mut state.presenter {
                        if let Err(error) = presenter.render(&state.runtime) {
                            eprintln!("Window present error: {error}");
                            DestroyWindow(hwnd);
                        }
                    }
                    if let Some(trace) = &mut state.lifecycle_trace {
                        if let Err(error) =
                            writeln!(trace, "{}", state.runtime.lifecycle_trace_line())
                        {
                            eprintln!("Lifecycle trace write error: {error}");
                            state.lifecycle_trace = None;
                        } else if let Err(error) = trace.flush() {
                            eprintln!("Lifecycle trace flush error: {error}");
                            state.lifecycle_trace = None;
                        }
                    }
                    if let Some(frames_left) = &mut state.lifecycle_frames_left {
                        *frames_left = frames_left.saturating_sub(1);
                        if *frames_left == 0 {
                            DestroyWindow(hwnd);
                        }
                    }
                }
                return 0;
            }
            WM_MOUSEMOVE => {
                if let Some(state) = state_mut(hwnd) {
                    if state.mode == HostWindowMode::Screensaver
                        && state.launch_instant.elapsed().as_millis() >= INPUT_GRACE_MS
                    {
                        let mut point: POINT = std::mem::zeroed();
                        if GetCursorPos(&mut point) != 0 {
                            if let Some((last_x, last_y)) = state.last_cursor {
                                if (last_x - point.x).abs() > JITTER_THRESHOLD_PX
                                    || (last_y - point.y).abs() > JITTER_THRESHOLD_PX
                                {
                                    DestroyWindow(hwnd);
                                }
                            }
                            state.last_cursor = Some((point.x, point.y));
                        }
                    }
                }
                return 0;
            }
            WM_KEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    if state.mode == HostWindowMode::Screensaver
                        && state.launch_instant.elapsed().as_millis() >= INPUT_GRACE_MS
                    {
                        DestroyWindow(hwnd);
                    }
                }
                return 0;
            }
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = std::mem::zeroed();
                BeginPaint(hwnd, &mut paint);
                EndPaint(hwnd, &paint);
                return 0;
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                return 0;
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return 0;
            }
            WM_NCDESTROY => {
                let state_ptr = take_state_ptr(hwnd);
                if !state_ptr.is_null() {
                    drop(Box::from_raw(state_ptr));
                }
                return 0;
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, 0, lparam)
    }

    unsafe fn state_mut(hwnd: HWND) -> Option<&'static mut HostState> {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HostState;
        ptr.as_mut()
    }

    unsafe fn take_state_ptr(hwnd: HWND) -> *mut HostState {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HostState;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        ptr
    }

    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(iter::once(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_host_action, parse_benchmark_frames, parse_glow_quality, parse_gpu_selection,
        parse_hwnd, parse_lifecycle_frames, parse_option_value, parse_pipeline, parse_scr_mode,
        parse_u32, HostAction, ScrMode,
    };
    use matrisaver_core::config::{GlowQuality, Pipeline};

    fn parse(values: &[&str]) -> ScrMode {
        let args: Vec<String> = values.iter().map(|value| value.to_string()).collect();
        parse_scr_mode(&args)
    }

    #[test]
    fn defaults_to_screensaver_mode() {
        assert_eq!(parse(&["matrisaver.scr"]), ScrMode::Screensaver);
    }

    #[test]
    fn parses_config_mode() {
        assert_eq!(parse(&["matrisaver.scr", "/c"]), ScrMode::Configure);
    }

    #[test]
    fn parses_preview_mode_with_following_hwnd() {
        assert_eq!(
            parse(&["matrisaver.scr", "/p", "4242"]),
            ScrMode::Preview {
                parent_hwnd: Some(4242)
            }
        );
    }

    #[test]
    fn parses_preview_mode_with_inline_hwnd() {
        assert_eq!(
            parse(&["matrisaver.scr", "/p:1337"]),
            ScrMode::Preview {
                parent_hwnd: Some(1337)
            }
        );
    }

    #[test]
    fn parses_preview_mode_with_hex_hwnd() {
        assert_eq!(
            parse(&["matrisaver.scr", "/p", "0xBEEF"]),
            ScrMode::Preview {
                parent_hwnd: Some(0xBEEF)
            }
        );
    }

    #[test]
    fn parses_inline_option_values() {
        let args = vec![
            "matrisaver.scr".to_string(),
            "/c".to_string(),
            "--variant=reloaded".to_string(),
        ];
        assert_eq!(parse_option_value(&args, "--variant"), Some("reloaded"));
    }

    #[test]
    fn parses_pipeline_values() {
        assert_eq!(parse_pipeline("opengl"), Some(Pipeline::OpenGl));
        assert_eq!(parse_pipeline("cpu"), Some(Pipeline::Cpu));
        assert_eq!(parse_pipeline("cpu_glow"), Some(Pipeline::CpuGlow));
        assert_eq!(parse_pipeline("unknown"), None);
    }

    #[test]
    fn parses_glow_quality_values() {
        assert_eq!(parse_glow_quality("low"), Some(GlowQuality::Low));
        assert_eq!(parse_glow_quality("balanced"), Some(GlowQuality::Balanced));
        assert_eq!(parse_glow_quality("high"), Some(GlowQuality::High));
        assert_eq!(parse_glow_quality("x"), None);
    }

    #[test]
    fn parses_u32_values() {
        assert_eq!(parse_u32("120"), Some(120));
        assert_eq!(parse_u32("-1"), None);
    }

    #[test]
    fn parse_hwnd_supports_decimal_and_hex() {
        assert_eq!(parse_hwnd("4242"), Some(4242));
        assert_eq!(parse_hwnd("0x10"), Some(16));
        assert_eq!(parse_hwnd("invalid"), None);
    }

    #[test]
    fn preview_requires_valid_hwnd() {
        let result = build_host_action(ScrMode::Preview { parent_hwnd: None });
        assert!(result.is_err());
    }

    #[test]
    fn preview_with_hwnd_builds_preview_action() {
        let result = build_host_action(ScrMode::Preview {
            parent_hwnd: Some(7),
        });
        assert_eq!(result, Ok(HostAction::RunPreview { parent_hwnd: 7 }));
    }

    #[test]
    fn parses_explicit_gpu_selection_flags() {
        let args = vec![
            "matrisaver.scr".to_string(),
            "--wgpu-backend=dx12".to_string(),
            "--wgpu-adapter-name".to_string(),
            "rtx".to_string(),
        ];
        let selection = parse_gpu_selection(&args);
        assert_eq!(selection.backend.as_deref(), Some("dx12"));
        assert_eq!(selection.adapter_name.as_deref(), Some("rtx"));
    }

    #[test]
    fn parses_benchmark_frames_inline_or_separate() {
        let args = vec![
            "matrisaver.scr".to_string(),
            "--benchmark-frames=240".to_string(),
        ];
        assert_eq!(parse_benchmark_frames(&args), Some(240));

        let args = vec![
            "matrisaver.scr".to_string(),
            "--benchmark-frames".to_string(),
            "120".to_string(),
        ];
        assert_eq!(parse_benchmark_frames(&args), Some(120));
    }

    #[test]
    fn parses_lifecycle_frames_inline_or_separate() {
        let args = vec![
            "matrisaver.scr".to_string(),
            "--lifecycle-frames=240".to_string(),
        ];
        assert_eq!(parse_lifecycle_frames(&args), Some(240));

        let args = vec![
            "matrisaver.scr".to_string(),
            "--lifecycle-frames".to_string(),
            "120".to_string(),
        ];
        assert_eq!(parse_lifecycle_frames(&args), Some(120));
    }
}
