# Devlog

## 2026-10-10 — M0 demo
- Landed: ac-core (model, classic tween w/ Animate-style matrix decomposition + ease, shape tween w/ auto correspondence, graphic/movie-clip symbols, editor commands + snapshot undo, XFL edges parser), ac-render (tiny-skia, SVG/PNG/GIF), ac-cli, ac-app (egui editor: tools, stage, timeline, properties, library, onion skin, edit-symbol mode).
- 14 tests green. UI exercised under Xvfb with simulated mouse/keyboard: draw oval/rect/pencil, move, paint bucket, step frames, F6, drag to auto-keyframe.
- Still open: merge drawing; copy/paste; multi-select; command registry/MCP; FLA import; wasm build (static.rust-lang.org blocked in the build sandbox, so untested).
