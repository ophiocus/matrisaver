# Linux Host

Related docs:

- Docs index: [`../README.md`](../README.md)
- Architecture: [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)
- Thin adapter boundary: [`../architecture/thin-adapters/THIN_ADAPTERS.md`](../architecture/thin-adapters/THIN_ADAPTERS.md)
- Debian packaging: [`../release/LINUX_PACKAGING.md`](../release/LINUX_PACKAGING.md)
- Roadmap (Milestone 3): [`../planning/ROADMAP.md`](../planning/ROADMAP.md)

`matrisaver-host-linux` is the Linux playback host. Before Milestone 3 it
was a headless benchmark stub that ticked the runtime and printed a
`PERF` line; it now creates real windows and surfaces.

## Modes

| Invocation | What it does |
| --- | --- |
| `matrisaver` / `--run` | Fullscreen playback on every participating output. The default. |
| `--window` | One ordinary window. Development, screenshots, and watching the renderer while reading a terminal. |
| `--benchmark-frames N` | Headless: advance N frames, print `PERF`, exit. The only mode that needs no compositor. |
| `--list-adapters` | Adapters wgpu can enumerate. |
| `--list-monitors` | Outputs the compositor reports. |
| `--help` | Full flag list. |

Listing modes win over playback, so `--list-monitors --run` answers the
question instead of taking over the screen.

**The default changed in Milestone 3.** A bare invocation used to run the
benchmark; it now plays. The benchmark is behind its explicit flag.
`/usr/bin/matrisaver` with no arguments is what the `.desktop` entry and
every idle-daemon example invoke, so playback is the useful default.

## One scene, N windows

Core renders **one** scene at **one** surface size and hands the host a
texture view. The Windows `.scr` exploits that directly: one window
spanning the virtual screen, one surface, blit whole.

A Wayland surface belongs to exactly one output and cannot span, so that
arrangement is not expressible there. Instead the Linux host:

1. Collects every output rect from winit (origins may be negative — a
   monitor left of the primary reports `x < 0`).
2. Computes the **union bounding box** and renders the scene at that
   size (`layout::plan`).
3. Creates one borderless-fullscreen window per output, each presenting
   its own UV sub-rectangle of that single scene
   (`matrisaver_core::present::SceneBlit` with a `UvRect`).

The rain is therefore continuous across a bezel: columns do not restart,
because there is literally one cascade. The same code path serves X11,
where a spanning window *would* have worked.

`--single-monitor` (or `multi_monitor: false` in settings) collapses to
the primary output with a full UV rect. The overlay reference rect is
pinned to the primary output in scene space, mirroring what the Windows
host does with `SM_CXSCREEN`, so silhouettes compose against one screen
instead of stretching across the span.

Primary-output selection: winit does not report "is primary" per
monitor, so the primary handle is matched by name and position. Wayland
commonly reports no primary at all, in which case the largest output
wins.

> **Known discrepancy with the Windows host.** `Settings.multi_monitor`
> is honoured here but is *not* read by the Windows host, which always
> spans `SM_CXVIRTUALSCREEN` regardless of the setting. The settings
> dialog shows the checkbox either way. Not changed as part of Milestone
> 3 — it would alter shipped Windows behaviour — but the asymmetry is
> real.

## GPU and compositor fallback

Session detection runs first and is reported:

```
SESSION host=linux type=wayland
```

`WAYLAND_DISPLAY` wins over `DISPLAY` (under XWayland both are set, and
the native path avoids a round trip through the X server), then
`XDG_SESSION_TYPE` as a last resort. `MATRISAVER_FORCE_SESSION=wayland
|x11|headless` overrides detection, which is how the X11 path gets
exercised on a Wayland desktop without logging out.

No session plus a playback request is a hard, named failure: exit code
**4** with a message naming the likely causes (TTY/SSH, a systemd
*system* unit instead of a user unit, a user unit that started before the
compositor published its environment) and pointing at
`--benchmark-frames`.

GPU bring-up then walks an explicit ladder:

1. The backend the user asked for (`--wgpu-backend`, `WGPU_BACKEND`). An
   explicit request is never silently overridden — if it fails, that is
   the answer.
2. Vulkan.
3. GL.

Every demotion prints one line, and the rung that succeeded is recorded:

```
FALLBACK host=linux stage=gpu rung=vulkan reason=no adapters enumerated
SURFACE  host=linux rung=gl backend=Gl adapter=llvmpipe (...) device_type=Cpu
```

Total failure exits **3** and lists what was tried. An oversized span
(union wider or taller than the planning texture limit) demotes to
primary-only with `FALLBACK host=linux stage=layout`, because a
single-monitor screensaver beats a black one.

Adapter choice requires that *every* output surface is supported, since
the windows share one device so they can share one scene texture.

### Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Clean exit, including playback dismissed by the user |
| 1 | Runtime failure after startup |
| 2 | Bad usage: unknown flag or unparseable value |
| 3 | No usable GPU adapter on any backend |
| 4 | Playback requested with no display session |

### Diagnostic lines

`SESSION`, `SCENE`, `SURFACE`, `PRESENT_CHOICE` and `FALLBACK` go to
stderr, one line each, and are the first thing to ask for in a bug
report. `--lifecycle-trace PATH` appends core's per-frame `LIFECYCLE`
records; `--lifecycle-frames N` exits after N rendered frames, which is
what makes playback testable in CI-ish conditions.

## Input

Keyboard and mouse *clicks* dismiss playback; pointer **motion**
deliberately does not — the same rule the Windows host settled on in
v0.3.6, so a knocked desk or an optical-mouse twitch does not kill the
screensaver. A 500 ms input grace period after launch matches
`INPUT_GRACE_MS` on Windows and stops the keystroke that started the
screensaver from immediately ending it.

Shake-to-menu is **not** implemented here: it opens the egui settings
dialog, which is Windows-only. There is no `/c` equivalent on Linux;
settings are edited via `settings.json` or the CLI flags.

## Dependencies and why

`winit` is pulled in with `default-features = false` and the `x11`,
`wayland`, `wayland-dlopen` and `rwh_06` features.

`wayland-dlopen` matters: linking `libwayland-client` at build time would
require `libwayland-dev` plus `pkg-config` on every build host, CI runner
and bare WSL rootfs. dlopen resolves it at runtime from
`libwayland-client.so.0` instead, so the crate builds on a rootfs with
nothing but `build-essential`. X11 goes through `x11-dl`, which is
dlopen-only already.

The playback dependencies are declared under
`[target.'cfg(target_os = "linux")'.dependencies]`, and `linux_host` is
`#[cfg(target_os = "linux")]`, mirroring how `matrisaver-host-windows`
gates its Win32 stack. The crate is a workspace member on all three CI
runners, and only the Linux leg needs a window system. Off Linux the
crate carries `#![cfg_attr(not(target_os = "linux"), allow(dead_code))]`,
because the playback-only items are legitimately unread there and
`clippy -D warnings` would otherwise fail the macOS and Windows legs.

## Building and running from WSL

WSLg provides a Wayland compositor and `/dev/dxg`, so playback can be
exercised from WSL without a desktop Linux install:

```bash
cd rust
cargo build -p matrisaver-host-linux
./target/debug/matrisaver-host-linux --list-monitors
./target/debug/matrisaver-host-linux --window --width 960 --height 540 \
    --lifecycle-frames 120 --lifecycle-trace /tmp/lc.log
```

Keeping `CARGO_TARGET_DIR` on the Linux side (e.g.
`~/.cache/matrisaver-target`) rather than on the `/mnt/...` DrvFs mount
is worth a large constant factor on build times.

## Verified, and not

Verified on WSLg (Ubuntu 24.04, Wayland, mesa 25.2 llvmpipe via Vulkan),
2026-10-23:

- Window creation, surface configuration, scene planning, per-output UV
  blit, and the 3D compositor path for `reloaded`.
- All five variants render and exit 0.
- Exit codes 0 / 2 / 3 / 4 on their respective paths.
- Overlay pack resolution from an installed-style prefix with no
  `assets/` near the binary.
- Lifecycle trace line counts match `--lifecycle-frames`.

Not verified, and honest about it:

- **Multi-monitor at runtime.** WSLg reports a single output (`rdp-0`),
  so the union-scene and per-output UV paths are covered by
  `layout.rs`'s unit tests (side-by-side, stacked, negative origin,
  mixed resolution, oversized span, no-primary) but have not been run
  against two physical monitors.
- **Idle/lock integration per desktop.** WSLg is not a desktop session:
  there is no GNOME/KDE/sway idle service to integrate with. The helpers
  ship documented with per-desktop status; see
  [`../release/LINUX_PACKAGING.md`](../release/LINUX_PACKAGING.md).
- **The GL rung crashes on teardown under WSLg.** With
  `--wgpu-backend gl`, every requested frame renders correctly (the
  lifecycle trace confirms the full count at 1, 10 and 60 frames) and the
  process then segfaults during exit, inside Mesa's EGL teardown. GL
  *offscreen* (`--benchmark-frames --gpu-scaffold --wgpu-backend gl`)
  exits cleanly, and the Vulkan rung is clean at every frame count. WSLg
  ships a degraded EGL stack here — `ZINK: failed to choose pdev`,
  `egl: failed to create dri2 screen`, `failed to get driver name for
  fd -1` — which is the likeliest cause, but no backtrace was obtainable
  (no gdb on the test host), so Mesa-on-WSLg versus a host teardown
  ordering bug is **not** settled. Vulkan leads the ladder, so the GL
  rung is only reached where Vulkan is absent. Needs a run on real
  hardware to close.

## Future work

- `xscreensaver` `-window-id` embedding, so MatriSaver can be a proper
  xscreensaver hack rather than its own fullscreen window. See
  `/usr/share/matrisaver/integration/xscreensaver.example` for why the
  naive `programs:` entry is a trap.
- A settings UI. The egui dialog is Windows-gated today.
- Wayland layer-shell (`wlr-layer-shell`) for a true overlay surface
  instead of borderless fullscreen.
