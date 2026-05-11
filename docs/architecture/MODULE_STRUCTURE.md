# matrisaver-core — Module Structure

Source root: `rust/crates/matrisaver-core/src/`

## Mechanical note — `include!()` stitching

The `runtime/` files are **not** Rust modules.  They are pulled into `lib.rs` via
`include!()`, which performs pure textual substitution before the compiler sees
anything.  Every included file shares the same scope as `lib.rs` itself — private
types, constants, and helper functions defined in one included file are visible in
all others with no import boilerplate.

---

## File map

```
src/
│
├── lib.rs                          Public API surface
│   ├── pub mod config              Variant/runtime config, Settings schema
│   ├── pub mod renderer            GlyphAtlas, GlyphInstance, FramePlan, plan_frame
│   ├── pub mod gpu;                → gpu.rs
│   ├── pub mod storage             Settings persistence (JSON)
│   ├── pub mod perf                FrameProfiler, PerfSummary
│   ├── ExitReason enum
│   ├── pub struct CoreRuntime      Field declarations
│   ├── impl CoreRuntime            Public methods only (new, tick, settings, …)
│   ├── include!("runtime/types.rs")
│   ├── include!("runtime/trace.rs")
│   ├── include!("runtime/overlay/…")   ← see overlay section
│   ├── include!("runtime/lifecycle/…") ← see lifecycle section
│   └── fn hash01                   Deterministic noise helper
│
├── gpu.rs                          GPU device setup, wgpu pipeline, atlas upload
│                                   (content of the former inline `pub mod gpu {}`)
│
└── runtime/
    │
    ├── types.rs                    All private types and constants shared across
    │                               overlay and lifecycle:
    │                               RowCell, RainColumn, GhostGlyph,
    │                               OverlayHeader, OverlayTargetCell,
    │                               OverlayIntroGlyph, OverlayIntroMode,
    │                               OverlayTuning, OverlayDenoiseMode,
    │                               OverlayTuningConfig,
    │                               OriginalLifecycleMutators,
    │                               OVERLAY_* constants, COLUMN_PITCH_SCALE
    │
    ├── trace.rs                    Lifecycle trace line generation (LIFECYCLE …)
    │
    ├── overlay/
    │   ├── state.rs                Overlay state machine
    │   │                           update_overlay_state
    │   │                           clear_overlay_locks
    │   │                           advance_overlay_headers
    │   │
    │   ├── inject.rs               Image → glyph grid conversion
    │   │                           inject_overlay_from_image
    │   │                           (sampling, level remapping, header construction)
    │   │
    │   ├── emit.rs                 GPU instance emission
    │   │                           emit_overlay_intro_instances
    │   │                           emit_overlay_header_instances
    │   │
    │   ├── io.rs                   File I/O and lookup helpers
    │   │                           load_overlay_tuning
    │   │                           resolve_overlay_tuning_path
    │   │                           overlay_image_paths
    │   │                           resolve_overlay_directory
    │   │                           overlay_glyph_lookup
    │   │
    │   └── image.rs                Image signal processing (pure functions)
    │                               sample_overlay_cell       (4-tap jitter sample)
    │                               preprocess_overlay_luminance
    │                               apply_overlay_median_filter
    │                               apply_overlay_bilateral_filter
    │                               apply_overlay_clahe
    │                               apply_overlay_unsharp
    │                               auto_levels
    │                               remap_level
    │                               overlay_glyph_index_for_luminance
    │
    └── lifecycle/
        ├── mutators.rs             Per-variant tuning knobs
        │                           original_lifecycle_mutators
        │                           variant_style_params
        │
        ├── frame.rs                Frame entry point
        │                           build_stream_instances   ← called every tick
        │                           column_pitch
        │                           column_count
        │
        ├── column.rs               Per-column simulation + instance emit
        │                           update_original_column
        │                           emit_original_instances
        │
        ├── cells.rs                Cell-level write / erase / ghost / volatile
        │                           write_head_row
        │                           erase_row
        │                           maybe_spawn_ghost
        │                           update_volatile_cells
        │                           update_ghosts
        │
        └── reset.rs                Column initialisation and reset
                                    advance_head_glyph
                                    reset_head
                                    reset_eraser
                                    reset_column_non_original
                                    ensure_rain_columns
```

---

## Approximate line counts after split

| File | Lines |
|---|---|
| `lib.rs` | ~450 |
| `gpu.rs` | ~1 500 |
| `runtime/types.rs` | ~320 |
| `runtime/trace.rs` | ~89 |
| `runtime/overlay/state.rs` | ~120 |
| `runtime/overlay/inject.rs` | ~300 |
| `runtime/overlay/emit.rs` | ~65 |
| `runtime/overlay/io.rs` | ~100 |
| `runtime/overlay/image.rs` | ~430 |
| `runtime/lifecycle/mutators.rs` | ~50 |
| `runtime/lifecycle/frame.rs` | ~190 |
| `runtime/lifecycle/column.rs` | ~200 |
| `runtime/lifecycle/cells.rs` | ~140 |
| `runtime/lifecycle/reset.rs` | ~250 |

Previously `lib.rs` alone was ~2 926 lines; `overlay.rs` ~1 015; `lifecycle.rs` ~858.

---

## Where to add new code

| What | Where |
|---|---|
| New overlay image filter | `runtime/overlay/image.rs` |
| New overlay trigger logic | `runtime/overlay/state.rs` |
| New per-column effect | `runtime/lifecycle/column.rs` |
| New variant mutator field | `runtime/types.rs` + `runtime/lifecycle/mutators.rs` |
| New private shared type | `runtime/types.rs` |
| New public API method | `lib.rs` `impl CoreRuntime` block |
| New GPU pipeline stage | `gpu.rs` |
| New config field | `lib.rs` `pub mod config` |
