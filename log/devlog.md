# Devlog

## 2026-10-10 — M0 demo
- Landed: ac-core (model, classic tween w/ Animate-style matrix decomposition + ease, shape tween w/ auto correspondence, graphic/movie-clip symbols, editor commands + snapshot undo, XFL edges parser), ac-render (tiny-skia, SVG/PNG/GIF), ac-cli, ac-app (egui editor: tools, stage, timeline, properties, library, onion skin, edit-symbol mode).
- 14 tests green. UI exercised under Xvfb with simulated mouse/keyboard: draw oval/rect/pencil, move, paint bucket, step frames, F6, drag to auto-keyframe.
- Still open: merge drawing; copy/paste; multi-select; command registry/MCP; FLA import; wasm build (static.rust-lang.org blocked in the build sandbox, so untested).

## 2026-10-10 — M0.1: core editing fixes (user report: panel dead, frames unselectable, can't delete)
- Root causes: no frame-selection model at all; Delete edited a new keyframe instead of the span holding the object (object reappeared on other frames); window opened at 1440x880 logical px, which overflows the screen at 125–150% Windows scaling (right panel/timeline off-screen); paste never fired (egui only emits Paste when the OS clipboard has text); drags anchored at drag-detect point, not press origin (marquee/rect short by a few px).
- New `frames` module (pure, tested): insert/remove frames, convert to (blank) keyframes, clear keyframes, clear frames, tween on range, copy/paste/move frames with Animate semantics (moving a keyframe stretches neighbouring spans).
- Editor: multi-selection (click, shift-click, marquee, select all), delete from owning keyframe (both ends of a tween), cut/copy/paste/paste-in-place/duplicate, break apart, multi-object convert to symbol, layer reorder, library rename/duplicate/delete/type.
- App split into stage/panels/timeline modules; window starts maximised; stage + timeline take focus from text fields so shortcuts always work; right-click menus on stage, frames and library; library drag-to-stage.
- 24 tests green. UI scenarios exercised under Xvfb via tools/ui_drive.py: frame click/range/context menu, keyframe drag, click+Delete, marquee+Delete, Ctrl+Z, typed W/alpha, rotate, stroke toggle, library delete with confirm, library drag-to-stage, copy/paste/cut.
- Still open: command registry/MCP; merge drawing; rulers/snapping; FLA import; real file dialogs (rfd).
