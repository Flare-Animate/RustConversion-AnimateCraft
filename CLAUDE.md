# AnimationCraft — project brief

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
  no animation app. AnimationCraft fills that gap and follows the same playbook:
  native Rust, one command registry, CLI + agent control. Note "Craft" names and
  logos are ArtCraft's trademarks: talk to that team before shipping under this
  name, or pick our own.

## 2. What Adobe Animate is (feature map)

Status: ✅ in the demo · 🟡 partial · ⬜ planned

**Document & workspace**
- ✅ Stage with pasteboard, zoom/pan, document size, FPS, stage colour
- ✅ Panels: Tools, Timeline, Properties (context-sensitive), Library (search,
  sortable columns, animated preview; fills the column or docks left), Transform
- ✅ Scenes breadcrumb + edit-symbol mode, multiple scenes (Scene panel, Shift+F2) · ⬜ edit-in-place
- ✅ Multi-select (click, Shift+click, marquee, Select All), move/scale/rotate groups, arrow nudge,
  cut/copy/paste/paste-in-place/duplicate, Delete, Break Apart, stage right-click menu
- ✅ Library: drag to stage, rename, duplicate, change type, delete (removes instances)
- ✅ Rulers, guides, grid + snapping, Align panel, Info panel; native file dialogs, recent files, autosave + crash recovery
- ✅ Motion Editor (per-property graphs, key drag/retime, smooth/corner points, per-property eases)
- ⬜ Docking/workspaces, Color & Swatches, History, preferences

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
- ✅ M0.3 tools: Free Transform with 8 handles, edge skew, movable
  transformation point, Distort + Envelope modes; Transform panel (Ctrl+T) and
  Scale and Rotate; 3D Rotation / 3D Translation gizmos with perspective angle
  and vanishing point; Gradient fills (linear/radial, multi-stop) + Gradient
  Transform; Fluid Brush (velocity → width, tapered); Width tool (variable-width
  strokes); Bone tool + IK posing; Asset Warp (rigid MLS pins); Camera layer +
  tool; Rotation (view) tool; Oval start/end angle + inner radius; Eyedropper
  picks gradients
- ✅ **Merge Drawing** (default; J toggles Object Drawing): `merge.rs` keeps the
  layer's raw shape as disjoint fill regions + split stroke edges (i_overlay
  booleans). New fills cut what's under them, same colours merge, lines split
  fills and each other. Selection picks fills/lines (double-click: connected),
  marquee/lasso cut the shape; a selection floats until deselected, then merges.
  Edge bending and corner dragging with the Selection tool
- ✅ Brush modes (Normal/Fills/Behind/Selection/Inside), Eraser modes + Faucet,
  Paint Bucket gap size + filling enclosed areas, Lock Fill; Combine Objects
  (Union/Intersect/Punch/Crop); Smooth/Straighten/Optimize; Rectangle/Oval
  Primitives; dashed/dotted strokes with caps/joins; bitmap fills; shape bones
  (Bone tool inside a shape) + Bind tool
- ⬜ Toolbar gaps: Art/Pattern brushes, pressure/tilt, **Magic Wand**, ragged/
  hatched strokes, Ctrl-drag to add a corner, deleting single width points, Asset
  Warp on symbols/bitmaps, camera colour filters and layer depth
- ✅ Groups, bitmaps (PNG/JPG/GIF/BMP, animated GIF, sequences), SVG import, system fonts
- ⬜ Bitmap fills, dynamic/input text

**Timeline**
- ✅ Layers (visible / lock / outline, colour), frames, keyframes, blank keyframes,
  F5/F6/F7 and Shift variants, frame labels, onion skin (prev/next tint), loop
- ✅ Frame selection (click, drag range across layers, Shift+click, double-click span,
  layer-name click), drag to move keyframes, Clear Frames, Copy/Cut/Paste Frames,
  F5/F6/F7 on ranges, right-click frame menu, layer reorder by drag
- ✅ Camera layer (pan / zoom / rotate, keyframed + classic-tweened)
- ✅ Layer folders (collapse, drag in/out), mask layers, guide + classic motion guide (orient to path), Layer Properties, duplicate layer
- ✅ Layer parenting (layer menu ▸ Parent Layer), onion markers (drag, anchor, Onion 2/5/All, outlines),
  Edit Multiple Frames, Reverse Frames, Convert to Frame-by-Frame, Ctrl+drag a layer's end
- ⬜ Layer depth (2.5D), parent-view link column, audio layers with waveform + lip-sync

**Symbols**
- ✅ Graphic (synced to parent timeline: Loop / Play Once / Single Frame,
  first frame), Movie Clip (own clock), Button (Up/Over/Down/Hit; Control ▸ Enable Simple Buttons)
- ✅ Convert to Symbol (F8), New Symbol (Ctrl+F8), nested symbols, Library
- ✅ Filters (drop shadow, blur, glow, bevel, adjust color; tweenable) and all 14 blend modes; instance names readable/settable from frame scripts
- ✅ Swap Symbol; colour effects None / Brightness / Tint / Advanced / Alpha
- ⬜ 9-slice scaling, cache-as-bitmap

**Animation**
- ✅ Classic tween: Animate-style matrix decomposition (scale, skew, rotation
  via shortest path, extra CW turns), alpha + tint interpolation, −100…+100 ease
- ✅ Shape tween with automatic correspondence
- ✅ Custom ease curves + 30 presets (classic, shape and motion tweens), shape hints
- ✅ Motion tweens: object spans, property keyframes (position on an editable Bezier motion
  path, rotation, skew, scale, alpha, brightness), Orient to path, rotate N times; F6/Shift+F6
  add/clear property keys; keys survive splitting, reversing and span stretching
- ⬜ 3D/filter/colour properties in motion tweens, tween presets, rig mapping,
  bone springiness / joint constraints, armature layers
- ✅ Bones (FK at evaluation, CCD inverse kinematics), Asset Warp pins, 3D
  properties and gradients all tween with Classic tweens

**Output**
- ✅ Animated GIF, PNG frame, SVG frame (same rasteriser as the stage)
- ✅ PNG sequence, MP4/WebM/MOV via ffmpeg, GIF options, Publish Settings, background export jobs
- ✅ HTML5 Canvas publish (own runtime: display lists, scripts, buttons, sounds; folder or one
  .html), sprite sheet + atlas (JSON hash/array, Starling XML), animated SVG (SMIL), Lottie JSON,
  sound mixed into video exports
- ⬜ SWF (Ruffle's MIT/Apache SWF crates), WebGL, publish profiles

**Interchange**
- ✅ `.fla` (CS5+, zipped XFL) and XFL folders: read + write (`ac-xfl`), round-trip tested on a
  hand-written fixture; unsupported parts are reported, never silently dropped
- 🟡 needs a corpus of real .fla files; motion-object tweens import static
- ⬜ legacy binary FLA (pre-CS5 compound file) read-only, AI/PSD import

**Sound**
- ✅ Import WAV/MP3/OGG/FLAC, keyframe sounds (Event/Start/Stop/Stream, repeat/loop), waveform
  in the timeline, playback, auto lip-sync (heuristic) on graphic instances
- ⬜ sound effects/envelopes, sounds inside symbols, AIFF

**Scripting** (see SCRIPTING.md)
- ✅ Command registry (`ac_core::commands`, ~50 dotted commands, 1-based frames)
- ✅ Frame scripts: sandboxed QuickJS, CreateJS-style `this.stop()/gotoAndPlay()`,
  Actions panel (F9), run during playback; never on open
- ✅ Authoring scripts: built-in JS, Python, Node.js, Ruby, any language via a
  JSON-lines protocol; Script Console, Output, Command Reference; confirm
  before external scripts; one undo step per run; CLI `run`
- ✅ JSFL compatibility shim (`fl`, document, timeline, layers, frames, library) for `.jsfl` files
- ⬜ MCP server over the same registry; recorded macros; script plug-in menu

## 3. Architecture

```
crates/
  ac-core    model, timeline evaluation, tweens (motion.rs), merge drawing (merge.rs), shape armatures (rig.rs), sounds (sound.rs), editor commands   (no UI, wasm-portable)
  ac-render  tiny-skia rasteriser shared by stage + exports (filters, blend modes, masks);
             SVG, GIF, PNG, PNG sequence, video via ffmpeg (with sound); HTML5 Canvas (html5.rs +
             html5_runtime.js), sprite sheets / Lottie (formats.rs), animated SVG; system fonts
  ac-audio   Symphonia decoding, Animate-rule sound mixing, waveforms, lip-sync analysis, cpal playback (feature)
  ac-xfl     .fla / XFL read + write (edges, .dat bitmaps, zip)
  ac-script  scripting: sandboxed frame scripts (QuickJS), authoring scripts (JS embedded, JSFL shim, others via JSON bridge)
  ac-cli     headless: info / gif / png / svg / svg-anim / html5 / sprites / lottie / xfl / convert / run / commands
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

The full feature-by-feature comparison with Animate and the current build order
(Phases 1–5) live in `docs/ANIMATE-GAP-ANALYSIS.md`; it supersedes M1–M7 below.

| Milestone | Goal |
|---|---|
| **M0 (done: demo)** | Model, classic + shape tweens, symbols, onion skin, egui editor, GIF/PNG/SVG export, CLI. M0.1: selection, frame ops, clipboard, library management; 24 tests |
| **M0.2 (done)** | Toolbar overhaul (17 working tools), command registry, multi-language scripting, frame scripts; 40 tests |
| **M0.3 (done)** | Every toolbar tool works (31): skew/distort/envelope, Transform panel, 3D tools, gradients, Fluid Brush, Width, Bone/IK, Asset Warp, Camera, view rotation; Library fills the panel / docks left; 48 tests |
| **Phase 1 (done)** | Native file dialogs, recent files, autosave/recovery; groups, Align/Info; folders, masks, motion guides; filters + blend modes; bitmaps + SVG import; system fonts; PNG sequence + video export; rulers/guides/grid; frame-script instance access; 59 tests |
| **Phase 3 (done)** | Merge Drawing engine (cut/merge, line splitting, floating selections, edge bending), brush/eraser modes, Faucet, bucket gaps, Lock Fill, Combine Objects, shape cleanup, primitives, stroke styles, bitmap fills, shape bones + Bind; 83 tests |
| **Phase 4 (done)** | .fla/XFL read + write, sounds (sync modes, waveform, playback, video audio, lip-sync), HTML5 Canvas publish, sprite sheets, animated SVG, Lottie, JSFL shim; 101 tests |
| **Phase 2 (done)** | Custom eases + presets, motion tweens + Motion Editor + editable motion path, shape hints, Reverse / Frame-by-Frame / span-end drag, onion markers + Edit Multiple Frames, button states + Simple Buttons, Swap Symbol, Brightness/Advanced colour, scenes, layer parenting; 73 tests |
| **M1** | Route every menu item through the registry; MCP server; rulers/guides, Align panel, native file dialogs |
| **M2** | Merge Drawing planar-map engine, Brush/Eraser/Pen/Subselection, gradients, Selection-tool edge bending |
| **M3** | `.fla`/XFL import with a corpus of real files; XFL export; round-trip tests |
| **M4** | Motion tweens + Motion Editor, custom eases, shape hints, masks, guide layers, layer parenting, camera |
| **M5** | Video (ffmpeg), sprite sheets, Lottie, HTML5 Canvas runtime, audio layers |
| **M6** | Bone springs/constraints, shape bones (Bind), filters and blend modes, text |
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
