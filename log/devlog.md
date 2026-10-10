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

## 2026-10-10 — M0.2: tools + scripting (user asked for Animate's missing tools and Python/JS scripting)
- Studied PhotoCraft (storytold/photocraft, MIT/Apache) for structure only: toolbar = sections of flyout slots remembering the last tool, Edit Toolbar, commands with stable ids, tools as pointer→ops+overlays, JSON-lines automation with limits, WASM-sandboxed plug-ins. No code copied.
- Toolbar rebuilt (toolbar.rs) with Animate's set; 17 tools working, 7 shown dimmed as planned. Properties now Tool/Object/Frame/Doc tabs.
- Core: rounded rect, PolyStar, anchor editing (pick/move/insert/delete/convert), text outlines (ab_glyph + bundled Ubuntu/Hack fonts), command registry (commands.rs), frame scripts on keyframes (serde-default so old files load), undo squashing.
- ac-script: QuickJS sandbox (memory cap, interrupt deadline) for frame scripts and embedded authoring JS; external-process bridge with \x1eAC JSON-lines protocol + helpers for Python/Node/Ruby; timeouts, size and command limits.
- Bugs found by driving the UI: DragValue clamping reported as a change rescaled zero-width objects 100x per frame (fixed: only user interactions count as edits, panel-wide); inline text editor lost focus to the creating click; example scripts used select.all and rotated other layers (added select.layer).
- 40 tests green (28 core, 10 scripting incl. real Python/Node/Ruby runs, 2 render). Xvfb runs: flyout, PolyStar, Pen (curve + close), Text, Brush, Subselection drag, Eyedropper→Bucket, Eraser, Lasso, Zoom box, Script Console JS + Python (with confirm), Actions snippet → playback stops at frame 24.
- Still open: script runs block the UI thread (fine for short scripts; move to a worker + progress later); planned tools; MCP server.
