# Artifacts Directory

This directory stores local runtime/debug outputs captured during migration and validation work.

Only this file is tracked in git. All other files under `artifacts/` are intentionally ignored.

## Logs So Far

- `lifecycle_trace.log`: per-frame lifecycle telemetry from `matrisaver-host-windows` when running `/s` or `/p` with `--lifecycle-trace-file`.

Typical fields seen in current captures:

- `frame`, `t`: runtime frame index and seconds since start.
- `variant`: active settings variant at runtime.
- `cols`: active rain column count.
- `active_heads`: number of currently visible head actors.
- `visible_cells`: number of lit row-memory cells.
- `ghosts`: active ghost glyph entities.
- `head_resets`, `eraser_resets`: cumulative lifecycle reset counters.
- `head_writes`: cumulative row writes by head actors.

In newer runtime builds, additional chain-oriented fields may appear for parity diagnostics
(for example `visible_chain_glyphs`, `chain_resets`, `glyph_swaps`, `overlay_active`,
`overlay_locked`, `overlay_injected`, `overlay_image`, and lead-y statistics).

## Purpose

These logs are used to:

- confirm movement lifecycle behavior during Win11 `/s` smoke runs,
- correlate visual regressions with concrete counters,
- preserve reproducible evidence for parity debugging without polluting the repo history.
