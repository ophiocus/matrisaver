//! Linux playback host: winit windows, wgpu surfaces, one shared scene.
//!
//! Shape of a frame:
//!
//! 1. `CoreRuntime::tick` advances the cascade and renders it to the
//!    offscreen scene texture, exactly as on Windows. Scene size is the
//!    union bounding box of every participating output.
//! 2. Each per-output window blits its own UV sub-rectangle of that one
//!    scene ([`matrisaver_core::present::SceneBlit`]), or wraps it onto
//!    3D geometry when the variant demands the Reloaded compositor.
//!
//! One scene and N windows (rather than N scenes) is what keeps the rain
//! continuous across a bezel, and it is the only arrangement Wayland
//! permits: a surface belongs to exactly one output, so a spanning
//! window — which is what the Windows `.scr` creates — is not
//! expressible there.
//!
//! GPU bring-up walks an explicit ladder and says which rung it reached,
//! because "the screensaver is black on my machine" is otherwise
//! unanswerable.

use std::sync::Arc;
use std::time::Instant;

use matrisaver_core::config::Pipeline;
use matrisaver_core::gpu::{AdapterSnapshot, GpuSelectionOptions};
use matrisaver_core::present::{Present3d, SceneBlit, UvRect};
use matrisaver_core::CoreRuntime;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowId};

use crate::cli::{exit_code, Cli, Mode};
use crate::layout::{self, OutputRect, SceneLayout};
use crate::session::{self, Session, NO_SESSION_HELP};

/// Ignore input for a moment after launch. Same reasoning as the Windows
/// host: the key release that launched the screensaver would otherwise
/// dismiss it before a single frame landed.
const INPUT_GRACE_MS: u128 = 500;

/// Conservative max 2D texture dimension used when planning the scene.
/// The real limit is read back from the device once it exists; this is
/// the value used for the pre-device layout plan, and 8192 is the floor
/// across the GL tier wgpu will fall back to.
const PLANNING_MAX_TEXTURE_DIM: u32 = 8192;

pub fn run(cli: Cli) -> i32 {
    let session = session::detect();
    eprintln!("SESSION host=linux type={}", session.key());

    match cli.mode {
        Mode::ListAdapters => list_adapters(&cli),
        Mode::ListMonitors => list_monitors(session),
        Mode::Run | Mode::Window => present(cli, session),
        // Handled by main before reaching here.
        Mode::Benchmark { .. } | Mode::Help => exit_code::OK,
    }
}

fn list_adapters(cli: &Cli) -> i32 {
    let mut runtime = CoreRuntime::new(cli.settings.clone());
    runtime.set_gpu_selection(cli.gpu_selection.clone());
    for adapter in runtime.adapter_snapshots() {
        println!(
            "ADAPTER host=linux name={} backend={} device_type={}",
            adapter.name, adapter.backend, adapter.device_type
        );
    }
    exit_code::OK
}

fn list_monitors(session: Session) -> i32 {
    if !session.can_present() {
        eprintln!("{NO_SESSION_HELP}");
        return exit_code::NO_SESSION;
    }
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("Failed to open the event loop: {error}");
            return exit_code::NO_SESSION;
        }
    };
    // winit only exposes monitors through ActiveEventLoop, i.e. from
    // inside a running app, so listing them means running an app that
    // prints and immediately exits.
    let mut lister = MonitorLister;
    if let Err(error) = event_loop.run_app(&mut lister) {
        eprintln!("Event loop failed: {error}");
        return exit_code::RUNTIME;
    }
    exit_code::OK
}

/// Prints the compositor's outputs on first resume, then exits.
struct MonitorLister;

impl ApplicationHandler for MonitorLister {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
        let primary = event_loop.primary_monitor();
        for output in collect_outputs(&monitors, primary.as_ref()) {
            println!(
                "MONITOR host=linux name={} x={} y={} width={} height={} primary={}",
                output.name, output.x, output.y, output.width, output.height, output.primary
            );
        }
        event_loop.exit();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }
}

fn present(cli: Cli, session: Session) -> i32 {
    if !session.can_present() {
        eprintln!("{NO_SESSION_HELP}");
        return exit_code::NO_SESSION;
    }

    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("Failed to open the event loop: {error}");
            eprintln!("{NO_SESSION_HELP}");
            return exit_code::NO_SESSION;
        }
    };
    // Poll: the runtime is animated, so there is always a next frame.
    // Surface present mode (Fifo where available) does the pacing.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(cli, session);
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("Event loop failed: {error}");
        return exit_code::RUNTIME;
    }
    app.exit_code
}

/// Translate winit monitors into layout inputs.
///
/// winit does not expose "is primary" per monitor, so the primary handle
/// is compared by name and position. Wayland commonly reports no primary
/// at all, which `layout::plan` handles by taking the largest output.
fn collect_outputs(monitors: &[MonitorHandle], primary: Option<&MonitorHandle>) -> Vec<OutputRect> {
    monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            let position = monitor.position();
            let size = monitor.size();
            let is_primary = primary.is_some_and(|candidate| {
                candidate.name() == monitor.name() && candidate.position() == position
            });
            OutputRect {
                name: monitor.name().unwrap_or_else(|| format!("output-{index}")),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                primary: is_primary,
            }
        })
        .collect()
}

/// One window and everything needed to present into it.
struct HostWindow {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    blit: SceneBlit,
    present_3d: Present3d,
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    snapshot: AdapterSnapshot,
    format: wgpu::TextureFormat,
    present_mode: wgpu::PresentMode,
    alpha_mode: wgpu::CompositeAlphaMode,
}

struct App {
    cli: Cli,
    session: Session,
    runtime: CoreRuntime,
    gpu: Option<Gpu>,
    windows: Vec<HostWindow>,
    layout: Option<SceneLayout>,
    launch_instant: Instant,
    last_frame: Instant,
    start_instant: Instant,
    frames_rendered: u32,
    lifecycle_trace: Option<std::io::BufWriter<std::fs::File>>,
    logged_present_choice: bool,
    exit_code: i32,
}

impl App {
    fn new(cli: Cli, session: Session) -> Self {
        let mut runtime = CoreRuntime::new(cli.settings.clone());
        runtime.set_gpu_selection(cli.gpu_selection.clone());
        let now = Instant::now();
        Self {
            cli,
            session,
            runtime,
            gpu: None,
            windows: Vec::new(),
            layout: None,
            launch_instant: now,
            last_frame: now,
            start_instant: now,
            frames_rendered: 0,
            lifecycle_trace: None,
            logged_present_choice: false,
            exit_code: exit_code::OK,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, code: i32, message: &str) {
        eprintln!("{message}");
        self.exit_code = code;
        event_loop.exit();
    }

    fn open_lifecycle_trace(&mut self) {
        let Some(path) = self.cli.lifecycle_trace.clone() else {
            return;
        };
        use std::io::Write;
        let parent = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_path_buf());
        if let Some(parent) = parent {
            if !parent.as_os_str().is_empty() {
                if let Err(error) = std::fs::create_dir_all(&parent) {
                    eprintln!(
                        "Failed to create lifecycle trace directory '{}': {error}",
                        parent.display()
                    );
                    return;
                }
            }
        }
        let file = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) => {
                eprintln!("Failed to open lifecycle trace file '{path}': {error}");
                return;
            }
        };
        let mut writer = std::io::BufWriter::new(file);
        let scene = self
            .layout
            .as_ref()
            .map(|layout| (layout.scene_width, layout.scene_height))
            .unwrap_or((0, 0));
        let _ = writeln!(writer);
        let _ = writeln!(
            writer,
            "LIFECYCLE host=linux session={} mode={:?} width={} height={} backend={} adapter_hint={}",
            self.session.key(),
            self.cli.mode,
            scene.0,
            scene.1,
            self.cli.gpu_selection.backend.as_deref().unwrap_or("all"),
            self.cli
                .gpu_selection
                .adapter_name
                .as_deref()
                .unwrap_or("auto")
        );
        let _ = writer.flush();
        self.lifecycle_trace = Some(writer);
    }

    fn write_lifecycle_line(&mut self) {
        use std::io::Write;
        let line = self.runtime.lifecycle_trace_line();
        if let Some(writer) = &mut self.lifecycle_trace {
            if writeln!(writer, "{line}").is_err() || writer.flush().is_err() {
                eprintln!("Lifecycle trace write failed; dropping trace.");
                self.lifecycle_trace = None;
            }
        }
    }

    /// Build the window set, bring up the GPU, and wire the core
    /// scaffold to the shared device.
    fn bring_up(&mut self, event_loop: &ActiveEventLoop) -> Result<(), (i32, String)> {
        let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
        let primary = event_loop.primary_monitor();
        let outputs = collect_outputs(&monitors, primary.as_ref());

        let windowed = matches!(self.cli.mode, Mode::Window);
        // Window mode is always one window at the requested size; the
        // layout machinery is for fullscreen playback.
        let plan = if windowed {
            layout::plan(
                &[OutputRect {
                    name: "window".to_owned(),
                    x: 0,
                    y: 0,
                    width: self.cli.width,
                    height: self.cli.height,
                    primary: true,
                }],
                false,
                PLANNING_MAX_TEXTURE_DIM,
            )
        } else {
            layout::plan(
                &outputs,
                self.cli.settings.multi_monitor,
                PLANNING_MAX_TEXTURE_DIM,
            )
        };
        let plan = plan.map_err(|error| {
            (
                exit_code::RUNTIME,
                format!("Cannot plan a scene: {error}\n\n{NO_SESSION_HELP}"),
            )
        })?;

        if let Some(reason) = &plan.demotion {
            eprintln!("FALLBACK host=linux stage=layout reason={reason}");
        }
        eprintln!(
            "SCENE host=linux width={} height={} outputs={} multi_monitor={}",
            plan.scene_width,
            plan.scene_height,
            plan.outputs.len(),
            self.cli.settings.multi_monitor
        );

        // Create the windows first: adapter choice must be validated
        // against every surface that will be presented to.
        let mut created: Vec<(Arc<Window>, UvRect)> = Vec::new();
        for planned in &plan.outputs {
            let mut attributes = Window::default_attributes()
                .with_title("MatriSaver")
                .with_decorations(windowed);
            if windowed {
                attributes = attributes.with_inner_size(winit::dpi::PhysicalSize::new(
                    self.cli.width,
                    self.cli.height,
                ));
            } else {
                let monitor = monitors
                    .iter()
                    .find(|monitor| monitor.name().as_deref() == Some(planned.output.name.as_str()))
                    .cloned();
                attributes = attributes.with_fullscreen(Some(Fullscreen::Borderless(monitor)));
            }
            let window = event_loop.create_window(attributes).map_err(|error| {
                (
                    exit_code::RUNTIME,
                    format!(
                        "Failed to create a window on output '{}': {error}",
                        planned.output.name
                    ),
                )
            })?;
            if !windowed && !self.cli.show_cursor {
                window.set_cursor_visible(false);
            }
            let uv_rect = UvRect::from_sub_rect(
                planned.scene_x,
                planned.scene_y,
                planned.output.width,
                planned.output.height,
                plan.scene_width,
                plan.scene_height,
            );
            created.push((Arc::new(window), uv_rect));
        }

        let (gpu, surfaces) = bring_up_gpu(
            &created
                .iter()
                .map(|(window, _)| window.clone())
                .collect::<Vec<_>>(),
            &self.cli.gpu_selection,
        )
        .map_err(|error| (exit_code::NO_GPU, error))?;

        self.runtime
            .set_surface_size(plan.scene_width, plan.scene_height);
        self.runtime.set_overlay_reference_rect(
            plan.primary.x.max(0) as u32,
            plan.primary.y.max(0) as u32,
            plan.primary.width,
            plan.primary.height,
        );
        self.runtime
            .enable_gpu_scaffold_with_shared_device(
                gpu.snapshot.clone(),
                gpu.device.clone(),
                gpu.queue.clone(),
            )
            .map_err(|error| {
                (
                    exit_code::NO_GPU,
                    format!("Failed to initialize the core GPU scaffold: {error}"),
                )
            })?;

        for ((window, uv_rect), surface) in created.into_iter().zip(surfaces) {
            let size = window.inner_size();
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: gpu.format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: gpu.present_mode,
                desired_maximum_frame_latency: 2,
                alpha_mode: gpu.alpha_mode,
                view_formats: vec![],
            };
            surface.configure(&gpu.device, &surface_config);
            let blit = SceneBlit::with_uv_rect(&gpu.device, gpu.format, uv_rect);
            let present_3d = Present3d::new(
                &gpu.device,
                gpu.format,
                (surface_config.width, surface_config.height),
            );
            self.windows.push(HostWindow {
                window,
                surface,
                surface_config,
                blit,
                present_3d,
            });
        }

        self.layout = Some(plan);
        self.gpu = Some(gpu);
        self.open_lifecycle_trace();
        Ok(())
    }

    fn render_all(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        let Some(scene_view) = self.runtime.gpu_scaffold_output_view() else {
            self.fail(
                event_loop,
                exit_code::RUNTIME,
                "Core scaffold output was not initialized for playback.",
            );
            return;
        };

        let use_3d = matches!(
            self.runtime.runtime_config().pipeline,
            Pipeline::CodeStrips3D
        );
        if !self.logged_present_choice {
            self.logged_present_choice = true;
            eprintln!(
                "PRESENT_CHOICE variant={} pipeline={:?} use_3d={} windows={}",
                self.runtime.settings().variant,
                self.runtime.runtime_config().pipeline,
                use_3d,
                self.windows.len()
            );
        }

        let elapsed = self.start_instant.elapsed().as_secs_f32();
        let mut failure: Option<String> = None;

        for host_window in &mut self.windows {
            let frame = match host_window.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                    host_window
                        .surface
                        .configure(&gpu.device, &host_window.surface_config);
                    continue;
                }
                Err(wgpu::SurfaceError::Timeout) | Err(wgpu::SurfaceError::Other) => continue,
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    failure = Some("wgpu surface out of memory".to_owned());
                    break;
                }
            };
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("matrisaver-linux-encoder"),
                });
            if use_3d {
                host_window.present_3d.record_pass(
                    &gpu.queue,
                    &gpu.device,
                    &mut encoder,
                    &view,
                    scene_view,
                    elapsed,
                );
            } else {
                host_window
                    .blit
                    .record_pass(&gpu.device, &mut encoder, &view, scene_view);
            }
            gpu.queue.submit([encoder.finish()]);
            frame.present();
        }

        if let Some(message) = failure {
            self.fail(event_loop, exit_code::RUNTIME, &message);
        }
    }

    fn should_ignore_input(&self) -> bool {
        self.launch_instant.elapsed().as_millis() < INPUT_GRACE_MS
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        if let Err((code, message)) = self.bring_up(event_loop) {
            self.fail(event_loop, code, &message);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.runtime
                    .request_exit(matrisaver_core::ExitReason::HostRequest);
                event_loop.exit();
            }
            // Keyboard and clicks dismiss playback; pointer *motion*
            // deliberately does not. Same rule the Windows host settled
            // on in v0.3.6 — a knocked desk or an optical-mouse twitch
            // should not kill the screensaver.
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !self.should_ignore_input() {
                    self.runtime
                        .request_exit(matrisaver_core::ExitReason::UserInput);
                    event_loop.exit();
                }
            }
            WindowEvent::MouseInput { state, .. } => {
                if state == ElementState::Pressed && !self.should_ignore_input() {
                    self.runtime
                        .request_exit(matrisaver_core::ExitReason::UserInput);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                let Some(gpu) = &self.gpu else {
                    return;
                };
                let width = size.width.max(1);
                let height = size.height.max(1);
                let windowed = matches!(self.cli.mode, Mode::Window);
                for host_window in &mut self.windows {
                    if host_window.window.id() != window_id {
                        continue;
                    }
                    host_window.surface_config.width = width;
                    host_window.surface_config.height = height;
                    host_window
                        .surface
                        .configure(&gpu.device, &host_window.surface_config);
                    host_window.present_3d.resize(&gpu.device, width, height);
                    // In window mode the scene *is* this window, so the
                    // scene follows the resize. In fullscreen playback
                    // the scene is the output union and must not be
                    // redefined by one output's resize event; the UV
                    // rect keeps pointing at the same slice.
                    if windowed {
                        self.runtime.set_surface_size(width, height);
                        self.runtime.set_overlay_reference_rect(0, 0, width, height);
                        host_window.blit.set_uv_rect(&gpu.queue, UvRect::FULL);
                    }
                }
            }
            WindowEvent::RedrawRequested => {}
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_none() {
            return;
        }
        let now = Instant::now();
        let delta = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        // Clamp: a compositor that stalls the process (VT switch, a
        // suspend/resume cycle) would otherwise hand the simulation a
        // multi-second step and teleport every column.
        self.runtime.tick(delta.clamp(1.0 / 1000.0, 1.0 / 15.0));
        self.render_all(event_loop);
        if self.lifecycle_trace.is_some() {
            self.write_lifecycle_line();
        }

        self.frames_rendered = self.frames_rendered.saturating_add(1);
        if let Some(limit) = self.cli.lifecycle_frames {
            if self.frames_rendered >= limit {
                self.runtime
                    .request_exit(matrisaver_core::ExitReason::HostRequest);
                event_loop.exit();
            }
        }
    }
}

/// Create surfaces and pick a device, walking an explicit ladder.
///
/// Rungs, in order:
///
/// 1. The backend set the user asked for (`--wgpu-backend` / `WGPU_BACKEND`).
///    An explicit request is never silently overridden — if it fails,
///    that is the answer.
/// 2. Vulkan. On a modern Mesa stack this is the fast path.
/// 3. GL. Covers old drivers, llvmpipe software rendering, and the
///    remote/virtualised cases (including WSLg) where Vulkan may be
///    absent or refuse the surface.
///
/// Every demotion prints `FALLBACK host=linux stage=... reason=...`, and
/// total failure lists what *was* visible so the message is actionable.
fn bring_up_gpu(
    windows: &[Arc<Window>],
    selection: &GpuSelectionOptions,
) -> Result<(Gpu, Vec<wgpu::Surface<'static>>), String> {
    if windows.is_empty() {
        return Err("No windows to present into.".to_owned());
    }

    let ladder: Vec<(&str, wgpu::Backends)> = if let Some(raw) = selection.backend.as_deref() {
        let parsed = wgpu::Backends::from_comma_list(raw);
        if parsed.is_empty() {
            return Err(format!(
                "Invalid backend selection '{raw}'. Use comma-separated names like 'vulkan,gl'."
            ));
        }
        vec![("explicit", parsed)]
    } else {
        vec![
            ("vulkan", wgpu::Backends::VULKAN),
            ("gl", wgpu::Backends::GL),
        ]
    };

    let mut attempts: Vec<String> = Vec::new();

    for (rung, backends) in &ladder {
        match try_backend(windows, selection, *backends) {
            Ok(result) => {
                eprintln!(
                    "SURFACE host=linux rung={} backend={} adapter={} device_type={}",
                    rung,
                    result.0.snapshot.backend,
                    result.0.snapshot.name,
                    result.0.snapshot.device_type
                );
                return Ok(result);
            }
            Err(reason) => {
                eprintln!("FALLBACK host=linux stage=gpu rung={rung} reason={reason}");
                attempts.push(format!("{rung}: {reason}"));
            }
        }
    }

    Err(format!(
        "No usable GPU backend. Tried:\n  {}\n\nTry an explicit backend \
         (`--wgpu-backend gl`), check that a Vulkan or GL driver is installed \
         (mesa-vulkan-drivers / libgl1-mesa-dri), and run `matrisaver \
         --list-adapters` to see what wgpu can enumerate.",
        attempts.join("\n  ")
    ))
}

type BringUp = (Gpu, Vec<wgpu::Surface<'static>>);

fn try_backend(
    windows: &[Arc<Window>],
    selection: &GpuSelectionOptions,
    backends: wgpu::Backends,
) -> Result<BringUp, String> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        ..Default::default()
    });

    let mut surfaces: Vec<wgpu::Surface<'static>> = Vec::with_capacity(windows.len());
    for window in windows {
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| format!("surface creation failed: {error}"))?;
        surfaces.push(surface);
    }

    let mut adapters = pollster::block_on(instance.enumerate_adapters(backends));
    if adapters.is_empty() {
        return Err("no adapters enumerated".to_owned());
    }
    // Every surface must work on the chosen adapter: the windows share
    // one device so they can share one scene texture.
    adapters.retain(|adapter| {
        surfaces
            .iter()
            .all(|surface| adapter.is_surface_supported(surface))
    });
    if adapters.is_empty() {
        return Err("no adapter supported every output surface".to_owned());
    }

    let adapter = if let Some(hint) = selection.adapter_name.as_deref() {
        let wanted = hint.to_lowercase();
        let available: Vec<String> = adapters
            .iter()
            .map(|adapter| {
                let info = adapter.get_info();
                format!("{}({:?})", info.name, info.backend)
            })
            .collect();
        adapters
            .into_iter()
            .find(|adapter| adapter.get_info().name.to_lowercase().contains(&wanted))
            .ok_or_else(|| {
                format!(
                    "no adapter matched hint '{hint}'; available: {}",
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

    let info = adapter.get_info();
    let snapshot = AdapterSnapshot {
        name: info.name.clone(),
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
    };

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("matrisaver-linux-device"),
        ..Default::default()
    }))
    .map_err(|error| format!("request_device failed: {error}"))?;

    let caps = surfaces[0].get_capabilities(&adapter);
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
        .ok_or_else(|| "surface reported no supported formats".to_owned())?;
    let present_mode = caps
        .present_modes
        .iter()
        .copied()
        .find(|value| *value == wgpu::PresentMode::Fifo)
        .or_else(|| caps.present_modes.first().copied())
        .ok_or_else(|| "surface reported no supported present modes".to_owned())?;
    let alpha_mode = caps
        .alpha_modes
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);

    Ok((
        Gpu {
            device,
            queue,
            snapshot,
            format,
            present_mode,
            alpha_mode,
        },
        surfaces,
    ))
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
