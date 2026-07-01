# Feature request — Shake-to-menu (in-screensaver menu overlay)

*Status: proposed / not started. Filed 2026-06-03. Targeting v0.3.6+.*

Replace the current "any mouse movement exits the screensaver" behaviour
with a shake-triggered in-app menu, so the user gets a proper UI without
having to launch the admin dialog through the Windows screensaver
control panel.

## Motivation

- **Discoverability.** Right now the only path to Options / About / the
  repo URL is Windows' Personalization → Screen Saver → Settings, which
  is a UX that Windows itself has been demoting since Windows 10. Most
  users don't know it's there.
- **Cursor hygiene.** The mouse cursor is currently visible over the
  rain when the screensaver is running, breaking the effect.
- **Standard modern-app expectations.** Users expect an in-app menu
  from any full-screen experience (games, media players, other
  screensavers with a modern shell).

## Behavioural spec

### 1. Cursor hidden by default

In full-screen screensaver mode (`/s`), hide the cursor with
`ShowCursor(FALSE)` at surface bring-up. Cursor stays hidden while the
menu is closed, becomes visible when the menu opens, hidden again when
the menu closes.

### 2. Shake detection replaces "any movement exits"

- Track raw cursor position across recent frames (e.g. last ~500 ms).
- **Shake** = total path length > some threshold (~200 px) with
  ≥ 3 direction reversals within the window.
- On shake: open the menu overlay.
- Non-shake movement: no-op (stays in screensaver).
- Keyboard input (any key) and mouse click continue to exit
  immediately — same as today. Only the "wiggle the mouse to exit"
  contract changes.
- The Windows preview mode (`/p HWND`) does NOT get shake — it's a tiny
  render into the Personalization dialog, no user interaction expected.

### 3. Menu overlay

A translucent centred panel drawn on top of the running rain (rain
keeps animating behind it), rendered via the existing egui integration
already used for the config dialog:

```
  ┌─────────────────────────┐
  │       MATRISAVER        │
  │─────────────────────────│
  │   ▶  Options            │
  │      About              │
  │      Share              │
  │      Exit               │
  └─────────────────────────┘
```

- Arrow keys / mouse hover move the highlight.
- Enter or click selects.
- Escape or shake-again closes the menu (returns to screensaver, cursor
  re-hides).

### 4. Menu actions

| Item | Behaviour |
|---|---|
| **Options** | Spawn `matrisaver-host-windows.exe /c 0` in a new process. That's the standard Windows-screensaver "Settings" path — invokes the same egui config dialog that Personalization does. Screensaver stays running behind. |
| **About** | Show an info pane (still in the overlay): app name, version (from Cargo.toml at compile time), creator ("ophiocus"), first-release date (locked at 2026-05, needs sourcing from git tag `v0.1.0` metadata or hardcoded const), repo URL, license (MIT), credit block (Rezmason MIT glyphs, Matrix Code NFT CC-personal-use). Back button returns to menu. |
| **Share** | Show a QR code encoding `https://github.com/ophiocus/matrisaver`. Rendered inline in the overlay. Rust `qrcode` crate renders to a matrix that egui can paint as a checkerboard. Back button returns to menu. |
| **Exit** | `std::process::exit(0)` — clean shutdown. |

### 5. About-pane data source

- Version: `env!("CARGO_PKG_VERSION")` at compile time.
- Creator: `env!("CARGO_PKG_AUTHORS")` — currently
  `"MatriSaver Contributors"`, may want to override to `"ophiocus"` /
  personal handle for the display.
- First-release date: needs a source of truth. Options:
  1. Hardcode `FIRST_RELEASE: &str = "2026-05"` next to the version
     handling code. Simplest, one-line update.
  2. Bake it in via a `build.rs` that reads `git tag --sort=creatordate | head -1` and emits a `pub const`. More work, self-maintaining.
  Recommend (1) unless we're worried about tag rewrites again.
- Repo URL: hardcode `https://github.com/ophiocus/matrisaver` — same
  string the QR encodes.

## Non-goals (out of scope for this ticket)

- Multi-monitor menu placement. Menu always renders on the main screen
  (the same screen where `overlay_reference_rect` anchors overlays).
- Persistent menu preferences. Menu opens fresh each time.
- Localisation. English only for now; the whole app is English-only.

## Dependencies to add

- [`qrcode`](https://crates.io/crates/qrcode) — QR code generation.
  MIT/Apache. Small, no C deps.
- (Already have `eframe`/`egui` in the host crate — reuse.)

## Implementation notes

- The menu overlay is rendered at the end of the frame, AFTER the wgpu
  rain draw. Alpha-blend a semi-transparent panel over the final
  swapchain image. egui already knows how to composite into a wgpu
  render pass.
- Shake detection lives in the input handler that currently triggers
  the `/s`-mode exit. Replace the "exit on any movement > 3px" branch
  with the path-length + reversal-count check.
- Spawning `matrisaver-host-windows.exe /c 0` needs the current
  executable path — `std::env::current_exe()`. The `0` HWND is the
  "no parent, own window" case; Windows Personalization normally
  passes its own HWND, but a self-spawn from within the screensaver
  runs standalone.

## Related

- Task #12 (Bane admin knobs) — those sliders would land in the
  same config dialog `Options` opens.
- Ties into the "polish gaps" list from the v0.3.4 lift review — this
  is one of them.

## Estimate

- Cursor hide + shake detection: half a day.
- Menu overlay + Options spawn + Exit: half a day.
- About pane: 2 hours.
- Share (QR): 2 hours.
- Testing + preflight: 2 hours.

Total: **~2 focused days.**
