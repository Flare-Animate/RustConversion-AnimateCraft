# AnimateCraft — project brief

Open-source 2D animation authoring in Rust, modelled on the Adobe Animate
(formerly Flash Professional) workflow. Read this file at the start of every
session; record progress in `log/devlog.md`.

## 1. Why now

- Feb 2, 2026: Adobe announced Animate would be discontinued on Mar 1, 2026.
  Feb 4: after backlash it reversed course. Animate stays available to new and
  existing users but is in **maintenance mode** (security and bug fixes only, no
  new features). Users asked Adobe to open-source it; it didn't.
- Animators, TV pipelines, game studios and educators depend on a workflow
  (symbols + timeline + tweens) that no open tool reproduces well. Synfig,
  OpenToonz, Krita, Blender Grease Pencil and Wick Editor each cover part of it.
- The only direct attempt found (Inamate, Feb 2026) is Go/TypeScript in the
  browser under a source-available BSL licence. Its HN thread asked for a native
  app, Rust, merge-drawing, real tools beyond onion skin, and colour management.
- ArtCraft (PhotoCraft, VectorCraft, FilmCraft, EffectCraft, SoundCraft, …) has
  no animation app. AnimateCraft fills that gap and follows the same playbook:
  native Rust, one command registry, CLI + agent control. Note "Craft" names and
  logos are ArtCraft's trademarks: talk to that team before shipping under this
  name, or pick our own.

## 2. What Adobe Animate is (feature map)

Status: ✅ in the demo · 🟡 partial · ⬜ planned

**Document & workspace**
- ✅ Stage with pasteboard, zoom/pan, document size, FPS, stage colour
- ✅ Panels: Tools, Timeline, Properties (context-sensitive), Library w/ preview
- ✅ Scenes breadcrumb + edit-symbol mode · ⬜ edit-in-place, multiple scenes
- ✅ Multi-select (click, Shift+click, marquee, Select All), move/scale/rotate groups, arrow nudge,
  cut/copy/paste/paste-in-place/duplicate, Delete, Break Apart, stage right-click menu
- ✅ Library: drag to stage, rename, duplicate, change type, delete (removes instances)
- ⬜ Rulers, guides, grid, snapping, Align panel, Info panel, workspaces

**Drawing**
- ✅ Object Drawing model (each shape its own object)
- ✅ Tools panel in PhotoCraft's structure (sections → flyout slots, last-used
  tool per slot, Edit Toolbar), Properties ▸ Tool tab with per-tool options
- ✅ Selection, Subselection (anchors + handles), Free Transform, Lasso,
  Polygon Lasso, Pen + Add/Delete/Convert Anchor, Text (static, outlined via
  ab_glyph), Line, Rectangle (corner radius), Oval, PolyStar, Pencil
  (Straighten/Smooth/Ink), Paint Brush (filled outline), Paint Bucket, Ink
  Bottle, Eyedropper (switches to Bucket/Ink Bottle), Eraser (object mode),
  Hand, Zoom; snapping with guides
- ⬜ **Merge Drawing** model: Flash's planar map where shapes cut/merge, fill
  styles left/right of each edge, Selection-tool edge bending. This is the core
  engine milestone; nothing else in open source does it well.
- ⬜ Pressure/tilt, Fluid Brush, Art/Pattern brushes, fill-cutting Eraser modes,
  gradients (linear/radial) + Gradient Transform, bitmap fills, Width tool,
  dynamic/input text, system fonts, Rotate View, Camera, Bone, Asset Warp

**Timeline**
- ✅ Layers (visible / lock / outline, colour), frames, keyframes, blank keyframes,
  F5/F6/F7 and Shift variants, frame labels, onion skin (prev/next tint), loop
- ✅ Frame selection (click, drag range across layers, Shift+click, double-click span,
  layer-name click), drag to move keyframes, Clear Frames, Copy/Cut/Paste Frames,
  F5/F6/F7 on ranges, right-click frame menu, layer reorder by drag
- ⬜ Layer folders, guide & motion-guide layers, masks, layer parenting,
  layer depth (2.5D), Camera layer, multi-frame edit,
  audio layers with waveform + lip-sync

**Symbols**
- ✅ Graphic (synced to parent timeline: Loop / Play Once / Single Frame,
  first frame), Movie Clip (own clock), Button (static for now)
- ✅ Convert to Symbol (F8), New Symbol (Ctrl+F8), nested symbols, Library
- ⬜ Button states (Up/Over/Down/Hit), 9-slice scaling, swap symbol, filters
  (blur, drop shadow, glow, bevel), blend modes, cache-as-bitmap

**Animation**
- ✅ Classic tween: Animate-style matrix decomposition (scale, skew, rotation
  via shortest path, extra CW turns), alpha + tint interpolation, −100…+100 ease
- ✅ Shape tween with automatic correspondence
- ⬜ Shape hints, Motion tweens (property keyframes on a span + motion path +
  Motion Editor curves), custom ease graphs and presets, Bone tool / IK,
  Asset Warp, Puppet pins, rig mapping

**Output**
- ✅ Animated GIF, PNG frame, SVG frame (same rasteriser as the stage)
- ⬜ PNG sequence, MP4/WebM via ffmpeg, sprite sheet + texture atlas, HTML5
  Canvas runtime, Lottie JSON, SWF (Ruffle's MIT/Apache SWF crates), animated SVG

**Interchange**
- 🟡 XFL `edges` path parser (experimental, needs a corpus)
- ⬜ `.fla` import (CS5+ .fla is a ZIP of the XFL folder: `DOMDocument.xml`,
  `LIBRARY/*.xml`, `bin/`, media), uncompressed XFL read/write, legacy binary
  FLA (pre-CS5 compound file) read-only, SVG/AI/PSD import

**Scripting** (see SCRIPTING.md)
- ✅ Command registry (`ac_core::commands`, ~50 dotted commands, 1-based frames)
- ✅ Frame scripts: sandboxed QuickJS, CreateJS-style `this.stop()/gotoAndPlay()`,
  Actions panel (F9), run during playback; never on open
- ✅ Authoring scripts: built-in JS, Python, Node.js, Ruby, any language via a
  JSON-lines protocol; Script Console, Output, Command Reference; confirm
  before external scripts; one undo step per run; CLI `run`
- ⬜ MCP server over the same registry; recorded macros; script plug-in menu

## 3. Architecture

```
crates/
  ac-core    model, timeline evaluation, tweens, editor commands, XFL bits   (no UI, wasm-portable)
  ac-render  tiny-skia rasteriser shared by stage + exports; SVG, GIF, PNG
  ac-script  scripting: sandboxed frame scripts (QuickJS), authoring scripts (JS embedded, others via JSON bridge)
  ac-cli     headless: info / gif / png / svg / save-demo / run / commands
  ac-app     egui desktop editor (Animate "Essentials" layout)
```

- **Document model** mirrors Animate's DOM so XFL maps 1:1:
  Document → Timeline → Layer → Keyframe (DOMFrame) → Element (shape | instance),
  plus Library of Symbols each owning a Timeline. Keyframes live in a
  `BTreeMap<start, Keyframe>`; a keyframe spans to the next one.
- **Evaluation** is pure: `evaluate(timeline, frame) → Vec<DrawItem>`. Tweens are
  resolved at evaluation time, never baked. Graphic symbols map parent frame →
  local frame; movie clips use the global clock.
- **Editing** goes through `Editor` (every mutation = one undo step; drags merge).
  Undo is snapshot-based for now; switch to command diffs when documents grow.
- **One renderer**: the stage, library preview, onion skin and every export use
  `ac_render::draw_items`, so what you see is what you export. A GPU path
  (wgpu/vello) can replace it later behind the same interface.
- Native format: JSON (`.acx.json`) for now → zipped package with media later.

## 4. Roadmap

| Milestone | Goal |
|---|---|
| **M0 (done: demo)** | Model, classic + shape tweens, symbols, onion skin, egui editor, GIF/PNG/SVG export, CLI. M0.1: selection, frame ops, clipboard, library management; 24 tests |
| **M0.2 (done)** | Toolbar overhaul (17 working tools), command registry, multi-language scripting, frame scripts; 40 tests |
| **M1** | Route every menu item through the registry; MCP server; rulers/guides, Align panel, native file dialogs |
| **M2** | Merge Drawing planar-map engine, Brush/Eraser/Pen/Subselection, gradients, Selection-tool edge bending |
| **M3** | `.fla`/XFL import with a corpus of real files; XFL export; round-trip tests |
| **M4** | Motion tweens + Motion Editor, custom eases, shape hints, masks, guide layers, layer parenting, camera |
| **M5** | Video (ffmpeg), sprite sheets, Lottie, HTML5 Canvas runtime, audio layers |
| **M6** | Bone tool / IK, Asset Warp, filters and blend modes, text |
| **M7** | Web build (wasm), plug-ins, docs site, first public release |

## 5. Rules

- Clean-room: build from public documentation and observed behaviour only.
  Never copy Adobe code, icons, UI artwork or sample files. Don't use "Adobe"
  or "Animate" in the product name.
- Shortcuts follow Animate's defaults (V, Q, R, O, N, Y, K, H, F5–F8, Enter, `,` `.`).
- Before finishing a task: `cargo test --workspace`, `cargo clippy --workspace`,
  and for UI work run `AC_SCREENSHOT=out.png cargo run -p ac-app` (works under
  Xvfb) and look at the result. Append a devlog entry.
- Demo content must be original or public domain.
