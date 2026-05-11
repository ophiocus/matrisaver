# Performance Research Cycle 01

Related docs:

- Docs index: [`../README.md`](../README.md)
- Architecture: [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md)
- Roadmap: [`../planning/ROADMAP.md`](../planning/ROADMAP.md)
- Baseline benchmark capture: [`PERFORMANCE_RESEARCH_CYCLE_02_BASELINE.md`](PERFORMANCE_RESEARCH_CYCLE_02_BASELINE.md)

## Purpose

Validate whether the current migration roadmap is likely to produce the best practical
performance across Windows, macOS, and Linux before deeper implementation commits.

## Research Questions

1. Is `wgpu` still the best cross-platform backend strategy for compatibility plus performance?
2. What glyph rendering path gives best cost/quality for Matrix-rain style rendering?
3. What bloom/glow strategy provides strong visuals without unacceptable frame cost?
4. What fallback and benchmarking plan should gate architecture decisions?

## Sources Reviewed

1. Official `wgpu` repository README and platform matrix (`github.com/gfx-rs/wgpu`).
2. Official `wgpu` docs pages for backend enums/selection behavior (`docs.rs/wgpu`).
3. Arm engineering case study on post-processing optimization.
4. Industry references for bloom technique fundamentals (GPU Gems historical chapter).

## Findings

### 1. Backend strategy remains sound

- `wgpu` supports native Vulkan, Metal, DX12, and OpenGL (downlevel/best effort), matching
  the cross-platform target of this project.
- This keeps one rendering core while letting each OS use native GPU paths.
- Environment controls (`WGPU_BACKEND`, `WGPU_ADAPTER_NAME`) are useful for diagnostics and
  forced fallback testing.

Decision:

- Continue with `wgpu` core strategy.
- Explicitly build a deterministic backend fallback policy in host startup logic.

### 2. Glyph rendering should prioritize atlas + instancing first

- For this workload (many repeated glyph quads with frequent animation), texture-atlas plus
  instanced quads is the best first implementation for throughput and simplicity.
- SDF/MSDF can improve scale quality but adds pipeline complexity and preprocessing overhead.
- Since feature parity is currently more important than arbitrary text shaping, bitmap-atlas
  first is the better step for performance and delivery risk.

Decision:

- Implement bitmap atlas + GPU instancing as phase-1 renderer.
- Keep SDF/MSDF as optional phase-2 quality upgrade after baseline profiling.

### 3. Bloom/glow must be constrained from day one

- Post-processing bloom can dominate frame cost if unconstrained.
- Proven optimization pattern: threshold bright regions, process at reduced resolution,
  separable blur (horizontal/vertical), then compose.
- Half/quarter-resolution glow with quality controls is likely the best default trade-off.

Decision:

- Ship glow with configurable quality tiers:
  - `Low`: quarter-res blur path
  - `Balanced`: half-res blur path (default)
  - `High`: full-res or extended blur radius

### 4. Performance must be benchmark-gated, not assumption-gated

- Cross-platform graphics choices should be locked only after reproducible benchmark results.
- Multi-monitor and mixed-DPI are mandatory test dimensions for this product.

Decision:

- Add performance acceptance gates before finalizing renderer internals.

## Updated Performance Plan

## Phase A: Baseline instrumentation

- Add frame timing breakdown:
  - update time
  - draw submission time
  - post-process time
- Record adapter/backend metadata and selected fallback path.

## Phase B: Renderer prototypes

- Prototype P1: bitmap atlas + instancing + reduced-res bloom.
- Prototype P2: same renderer with bloom disabled (to isolate bloom cost).
- Optional P3: SDF/MSDF experiment only if P1 quality fails acceptance criteria.

## Phase C: Benchmark matrix

- Windows: DX12 preferred, GL fallback check.
- Linux: Vulkan preferred, GL fallback check.
- macOS: Metal preferred.
- Test dimensions:
  - single monitor vs multi-monitor
  - 1080p, 1440p, 4K where available
  - integrated vs discrete GPU where possible

## Phase D: Gate criteria (initial)

- Stable target framerate for configured quality tier in each target class.
- No severe frame-time spikes during monitor transitions or resize events.
- Fallback backend still meets minimum viable animation smoothness.

## Impact on Roadmap

- No roadmap reset required.
- Milestone 1 remains active.
- Add explicit benchmark gate before locking final glow implementation details.

## Next Implementation Actions

1. Add timing instrumentation scaffold in `matrisaver-core` and host logging hooks.
2. Build first renderer skeleton around atlas + instanced quads.
3. Implement reduced-resolution glow path with quality tiers.
4. Run and publish first benchmark report as Cycle 02.
