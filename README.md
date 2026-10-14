# AnimationCraft (demo)

Open-source 2D animation authoring in Rust, following the Adobe Animate workflow:
symbols, layers, keyframes, classic, shape and motion tweens with a Motion Editor,
onion skin, scenes, and a familiar
Tools / Stage / Timeline / Properties / Library workspace.

Not affiliated with Adobe.

## Install (Windows)

Download `AnimationCraft-<version>-setup.exe` from the repository's Releases (or
the **Windows build** workflow run under Actions) and run it. It installs for
your user only (no admin prompt), adds Start Menu and Desktop shortcuts, opens
`.acx` files with AnimationCraft, and can be removed from *Apps & features*. A
portable zip (`animationcraft.exe` + `animationcraft-cli.exe`) is published too.

Build them yourself: on Windows, `cargo build --release -p ac-app -p ac-cli`
then `makensis packaging/windows/installer.nsi`; from Linux,
`sh tools/build-windows.sh` (MinGW + NSIS).

## Run it (from source)

Requires Rust 1.95+ (https://rustup.rs; EffectCraft needs it).

```sh
cargo run --release -p ac-app          # the editor (opens the demo scene)
cargo run -p ac-cli -- info demo       # inspect the demo document
cargo run -p ac-cli -- gif demo out.gif
cargo run -p ac-cli -- video demo out.mp4   # needs ffmpeg
cargo run -p ac-cli -- html5 demo out.html  # one web page that plays the movie
cargo run -p ac-cli -- xfl demo out.fla     # .fla / XFL (any document argument may be a .fla too)
cargo test --workspace
```

Linux needs the usual X11/Wayland + OpenGL libraries (any desktop has them) and,
for sound playback, the ALSA headers (`libasound2-dev` / `alsa-lib-devel`). Build
without sound output with `cargo run -p ac-app --no-default-features`.

## Try this

1. Press **Enter** to play; **,** and **.** step frames. Turn on **Onion skin**.
2. **Select objects**: click (Shift+click adds), or drag a box around them on
   empty stage. Drag to move, drag a corner to scale (Shift keeps proportions);
   with **Q** drag just outside a corner to rotate. Arrow keys nudge.
3. **Remove objects**: **Delete**/**Backspace**, the 🗑 Delete button in
   Properties, or right-click → Delete. Cut/Copy/Paste/Duplicate with
   Ctrl+X/C/V/D; Ctrl+Z undoes.
4. **Frames**: click a cell to select it, drag across cells/layers for a range,
   Shift+click to extend, double-click to select a whole span, click a layer
   name for all its frames. Drag a selection to move keyframes. Right-click for
   F5/F6/F7, tweens, Clear Frames and Copy/Paste Frames. Delete clears frames.
5. Draw with **R**, **O**, **N**, **Y**; press **F8** to make a symbol.
   Double-click an instance or a group to edit it in place (the rest of the
   scene stays visible, dimmed); double-click a Library item for isolated
   editing. **Esc** (or double-clicking empty space) goes back.
6. **Library**: drag an item onto the stage; right-click to rename, duplicate,
   change type or delete.
7. File → Export → Export Image (PNG / JPEG / GIF), SVG, Animated GIF, video,
   HTML5 Canvas, sprite sheets, Lottie, .fla.

## Tools

Two-column Tools panel, organised like Animate (and PhotoCraft): each button is
a group; a corner triangle means more tools inside: **right-click or
long-press** to pick one. **…** at the bottom opens *Edit Toolbar* to hide
tools. Options for the active tool are in Properties ▸ **Tool**.

| Group | Tools (shortcut) |
|---|---|
| Select | Selection (V), Subselection (A), Free Transform (Q: scale, **skew**, rotate around a movable transformation point; **Distort** and **Envelope** modes), Gradient Transform (F), **3D Rotation (W)**, **3D Translation (G)**, Lasso (L), Polygon Lasso, **Magic Wand** |
| Draw | Pen (P) + Add (=) / Delete (−) / Convert Anchor, Text (T), Line (N), Rectangle (R, corner radius), Oval (O, start/end angle, inner radius), PolyStar |
| Paint | Paint Brush (Y), **Fluid Brush (B)**, Pencil (Shift+Y), **Width (U)**, **Bone (M)**, **Asset Warp (Shift+W)**, Paint Bucket (K), Ink Bottle (S), **Eyedropper (I)**, Eraser (E) |
| View | **Camera (C)**, Hand (H / Space), **Rotation (Shift+H)**, Zoom (Z) |
| Colours | Stroke / Fill / none, Swap (X), Black & White (D), Snap; fill type Solid / Linear / Radial gradient |

### Transforms

- **Free Transform (Q)**: corner handles scale (Shift = proportional, Alt =
  around the transformation point), side handles stretch one axis, dragging an
  **edge** skews (top/bottom = horizontal, left/right = vertical), just outside
  a corner rotates. Drag the white dot to move the transformation point.
- **Distort / Envelope**: pick the mode in Properties ▸ Tool or Modify ▸
  Transform; drag the green handles (shapes; Break Apart symbols first).
- **Window ▸ Transform (Ctrl+T)**: exact W/H %, rotate or skew H/V, 3D
  rotation and centre, flip, Remove Transform, and *Copy selection and apply
  transform* for radial patterns. Modify ▸ Transform ▸ Scale and Rotate
  (Ctrl+Alt+S).
- **3D Rotation (W)**: red line = X, green = Y, blue ring = Z, orange ring =
  free, centre dot = 3D centre. **3D Translation (G)**: X/Y arrows, centre dot =
  depth. Perspective angle and vanishing point: Properties ▸ Doc. 3D values
  tween with Classic tweens.

### Bones, warp, width, camera

- **Bone (M)**: drag from one object to another on the same layer; start on a
  joint to extend the chain. Then drag any bonded object with the Selection
  tool to pose the chain (inverse kinematics); poses become keyframes.
  Properties ▸ Object shows the bone and Remove bone / Remove all bones.
- **Asset Warp (Shift+W)**: click a shape to drop pins, drag pins to bend it,
  Alt+click removes a pin.
- **Width (U)**: drag out from a stroke to make it thicker there; Alt+click a
  width point to delete it; Copy/Paste width profile in Properties.
- **Paint Brush (Y)**: seven brush shapes (round, oval, square, flat, slanted)
  and the five brush modes. **Magic Wand**: Break Apart a bitmap (Ctrl+B), then
  click a colour to select it (threshold in Properties ▸ Tool).
- **Selection (V)** on merge shapes: drag an edge to bend it, Ctrl-drag to add
  a corner.
- **Gradients**: Properties ▸ Object ▸ Fill ▸ Linear/Radial (edit stops and
  Flow: extend / reflect / repeat there), then the Gradient Transform tool (F)
  to move, rotate and stretch (bitmap fills too).
- **Camera (C)**: adds a Camera layer; drag pans, scroll zooms, Shift+drag
  rotates. Keyframe and tween it like any layer; playback shows the camera view.
  Camera effects (brightness, tint, alpha, blur) apply to the whole shot.
- **Free camera (VCam)**: Properties ▸ Camera ▸ Mode ▸ Free. The stage stays put
  and the camera becomes an orange frame on it: drag inside to move, a corner to
  zoom, the knob to rotate (Shift: 15° steps). Follow a named object (with lag
  and offset), add shake, and tick "Preview through camera" to see the shot.
- **Width (U)** works on drawing objects and on merge-shape lines.
- **Rotation (Shift+H)** turns the view only (double-click to reset).

### Files, import and export

- **Documents are `.acx` files**: a ZIP package, like `.fla`, holding the
  document as readable parts (`manifest.json`, `document.json`, one JSON file
  per scene and per symbol) plus images as PNG files, sounds in their original
  format and a thumbnail. Older single-file `.acx.json` documents still open
  (and save, if you give that extension).
- **File › Open / Save** use your system's file dialogs; **Open Recent**
  remembers 10 files. Unsaved work is **autosaved** every minute, and after a
  crash AnimationCraft offers to restore it. Closing with unsaved changes asks
  first. Drop images, SVGs or documents onto the window to import/open them.
- **Import** (Ctrl+R): PNG, JPG, GIF, BMP (animated GIFs become frames),
  numbered **image sequences**, and **SVG** (editable shapes, gradients, text).
  Bitmaps live in the Library with "Allow smoothing".
- **Export**: PNG, SVG, animated GIF, **PNG sequence**, **video** (MP4 /
  WebM / MOV through [ffmpeg](https://ffmpeg.org); on Windows
  `winget install ffmpeg`). **File › Publish Settings** (Ctrl+Shift+F12) sets
  scale, frame range, transparency and GIF options. Long exports run in the
  background with a progress window.
- **.fla and XFL**: File › Open reads CS5-or-later `.fla` files and XFL
  folders (shapes become merge shapes, drawing objects, symbols, tweens,
  custom eases, text, bitmaps, sounds, filters, frame scripts); saving keeps
  the format. **File › Export › Export .fla / XFL** writes either (a name
  ending in `.xfl` makes an uncompressed folder). Anything the other format
  can't hold is listed in the Output panel instead of disappearing.
- **Publish HTML5 Canvas**: a folder (index.html + movie.js + runtime +
  images) or a single self-contained `.html`. It plays exactly what the stage
  shows, runs frame scripts (with `addEventListener("click", …)` on named
  instances), makes buttons live and plays sounds.
- **Generate Sprite Sheet** (PNG + JSON hash / JSON array / Starling XML,
  trimmed, duplicates shared), **Animated SVG** (SMIL, plays in browsers) and
  **Lottie** JSON.

### Effects, motion blur, tracing

- **Motion Blur** (Properties › Filters › Add filter): blurs an object along its
  real movement since the previous frame (shutter angle, samples), through tweens,
  motion guides, nested symbols and the camera.
- **EffectCraft effects** (Add filter › EffectCraft effects, with search): about 200
  effects from ArtCraft's EffectCraft — blurs, distortions, stylize, generators,
  colour, keying. EffectCraft is a git dependency: `cargo update -p effectcraft-effects`
  brings in its latest version. They render in the stage and every raster export.
- **Trace Bitmap** (Modify › Trace Bitmap…): colour threshold, minimum area, curve fit
  and corner threshold; the bitmap becomes merge-shape fills that meet exactly.
  **Centerline** mode turns line art into lines along the middle of the ink, with
  its thickness.
- **Shift & Trace** (timeline toolbar): the previous and next keyframes as ghosts you
  slide (drag; Shift+drag for the next one) and turn (Alt+drag) under your drawing.
- **Import Photoshop (PSD)**: one bitmap layer per Photoshop layer, in place.
- **Export Timing Sheet (XDTS)**: layer timing for Clip Studio Paint / OpenToonz.

### From OpenToonz: styles, deformers, assistants, painting

- **Stroke styles** (Properties ▸ Fill and stroke): besides Animate's Solid, Dashed,
  Dotted, Ragged, Stipple and Hatched, OpenToonz's Herringbone, Chain, Circlets,
  Scratchy, Rope, Tulle, Plait, Fuzz, Bubbles, Jagged, Zigzag, Wave and Curl, each
  with its own parameters and extra colours.
- **Deformation tools** (flyout next to the Width tool): **Magnet** pulls everything
  within a radius, **Pinch** drags a line (smooth or a corner), **Pump** thickens or
  thins a line, **Bender** bends lines where they cross a segment you draw,
  **Iron** smooths wobbles. They work on drawing objects and merge shapes.
- **Drawing assistants** (Assistants tool): guides that straighten freehand
  strokes (Line or parallels, Vanishing point, Ellipse or concentric) and symmetry
  that repeats them (Mirror, Star with optional kaleidoscope, Grid), for the Pencil,
  Paint Brush and Fluid Brush.
- **Raster Brush** (Brush flyout): 25 MyPaint brushes (pencils, inks, charcoal,
  chalk, airbrush, watercolour, smudge, erasers) painting pixels into a bitmap on
  the current keyframe, frame by frame.
- **Motion Editor**: right-click a key ▸ *Interpolation to next key*: Constant,
  Linear, Ease In/Out, Exponential, **Expression** (`sin(frame*10)*20`,
  `100 - rotation/2`, `random(0, 5)` …) or Similar Shape.
- **Joint constraints** (Properties ▸ Bones): turn a joint off or limit how far it
  bends.

### Sound

- **File › Import › Import Sound** (WAV, MP3, OGG Vorbis, FLAC) or drop a
  file on the window. Drag a sound from the Library onto the stage to start it
  on the current keyframe, or pick it in **Properties › Sound** with **Sync**
  Event / Start / Stop / Stream and Repeat ×N or Loop.
- The waveform is drawn in the timeline frames; sound plays with the
  timeline and is mixed into video and HTML5 exports.
- **Lip Syncing**: select a graphic instance of a mouth symbol whose frames
  1–10 are Neutral, MBP, Ai, E, O, U, FV, L, WQ, etc., pick the voice layer in
  Properties and press **Lip Sync**: it keyframes the mouth to the sound
  (a loudness/frequency heuristic, ready for hand tweaks).

### Layers, masks and guides

Right-click a layer name: **New Folder**, **Mask** (the layer's shapes reveal
the layer below), **Guide**, **Add Classic Motion Guide** (classic tweens on
the layer below follow the first stroke on the guide; tick *Orient to path* in
Frame properties), **Layer Properties**. Drag layers into and out of folders;
click a folder's triangle to collapse it.

### Animation

- **Eases**: Properties ▸ Frame ▸ *Ease* picks the simple −100…100 ease, one of
  30 presets (Quad … Elastic, In / Out / In Out) or a **custom curve**: drag
  points and handles, double-click to add, right-click to delete (Alt breaks a
  handle). Works for classic, shape and motion tweens.
- **Motion tweens**: right-click a frame ▸ *Create Motion Tween* (a one-frame
  span grows to one second). Move to any frame and move/rotate/scale/fade the
  object: that writes property keyframes (diamonds), not new keyframes. The
  **motion path** shows on the stage: drag its squares to move positions, drag
  the line to bend it, use Subselection (A) for Bezier handles. F6 keys every
  property at the playhead, Shift+F6 clears them. Ctrl+drag (or just drag, for a
  motion tween) a layer's last frame to lengthen/shorten it; keys stretch.
- **Motion Editor** (double-click a motion span, or Window ▸ Motion Editor):
  one graph per property; drag keys up/down for values, left/right to retime,
  click a graph to move the playhead, right-click a key for Smooth / Corner /
  Delete, and give each property its own ease.
- **Shape hints**: on the first or last keyframe of a shape tween press
  Ctrl+Shift+H, then drag the lettered hint onto the outline on both keyframes
  (yellow/green = placed, red = off the outline).
- **Reverse Frames** and **Convert to Frame-by-Frame** in the frame menu.
- **Onion skin**: drag the markers above the frames; *Markers* ▸ Anchor /
  Onion 2 / 5 / All; *Outlines*; **Edit Multiple** shows every keyframe in the
  range and moves them together.
- **Layer parenting**: right-click a layer ▸ *Parent Layer*: its objects follow
  the first object on the parent layer (shown as an arrow and name).
- **Scenes**: Insert ▸ Scene, Window ▸ Scene (Shift+F2) to switch, rename,
  reorder, duplicate or delete. Exports play the scenes in order.

### Drawing: merge shapes and drawing objects

- **Merge Drawing** is the default (as in Animate): shapes drawn on a layer
  become one shape. A new shape **cuts** what is under it, same-coloured fills
  **merge**, and a line drawn across a fill **splits** it. Press **J** (or
  Tool properties ▸ Drawing model) for **Object Drawing**, where each shape is
  its own object.
- **Selection tool** on merge shapes: click a fill or a line to select just
  that part (double-click: a fill with its outline, or joined lines); drag a box
  or lasso to cut out part of a shape. A selection floats until you deselect,
  then merges where you dropped it. Drag an **edge** to bend it, a **corner**
  to move it.
- **Paint Brush** modes (Normal, Fills, Behind, Selection, Inside), **Eraser**
  modes (Normal, Fills, Lines, Selected Fills, Inside) and **Faucet**, **Paint
  Bucket** gap size (fills areas enclosed by lines) and **Lock Fill**.
- Modify ▸ **Combine Objects** (Union, Intersect, Punch, Crop) and ▸ **Shape**
  (Smooth, Straighten, Optimize); Ctrl+B on a drawing object turns it into
  merge shape.
- **Rectangle / Oval Primitive** (in the Rectangle flyout): corner radii,
  angles and inner radius stay editable in Properties.
- Stroke **style** (solid, dashed, dotted), **cap** and **join**, and
  **bitmap fills** from the library, in Properties for drawing objects.
- **Shape bones**: with the Bone tool, drag *inside* one shape to add bones to
  it; drag a bone with the Selection tool to bend the shape (poses tween). The
  **Bind** tool (Bone flyout) chooses which points follow which bone.

### Objects

- **Group / Ungroup** (Ctrl+G / Ctrl+Shift+G), **Align panel** (Ctrl+K) and
  **Info panel** (Ctrl+I), also in Modify › Align.
- **Filters and blending** in Properties › Object: Drop Shadow, Blur, Glow,
  Bevel, Adjust Color (they tween), and 14 blend modes.
- **Color effect** for symbol instances: Brightness, Tint, **Advanced**
  (per-channel % and offset) or Alpha; **Swap Symbol** keeps the transform.
- **Buttons**: a Button symbol's first four frames are Up / Over / Down / Hit.
  Control ▸ **Enable Simple Buttons** (Ctrl+Alt+B) makes them react on the stage.
- **Instance names** for symbols, text, bitmaps and groups; frame scripts can
  use them (`this.ball.x += 5`).
- Text can use any **installed font** (searchable picker).
- **View › Rulers** (Ctrl+Alt+Shift+R): drag guides out of the rulers, drop a
  guide back on a ruler to delete it. **View › Grid** (Ctrl+') with Snap to
  Grid and Edit Grid; objects and drawing tools snap to guides and the grid.

### Library

Symbols, bitmaps and sounds are listed together (a sound shows its waveform in the preview). The Library fills the rest of the right column under Properties (Properties
grows first and pushes it down). Search, sortable Name/Type/Uses/Frames
columns, an animated preview with play, and drag-to-stage. **View ▸ Library
position ▸ Own panel on the left** docks it beside the Tools; Ctrl+L hides it.

## Scripting

- **Window ▸ Actions (F9):** frame scripts in JavaScript (`this.stop()`,
  `this.gotoAndPlay("label")`…), sandboxed, run during playback.
- **Commands ▸ Script Console:** automate the editor in **JavaScript
  (built in), Python, Node.js or Ruby**, or any language via a one-line JSON
  protocol. One undo step per run.
- **JSFL**: existing `.jsfl` scripts run through a compatibility layer
  (`fl.getDocumentDOM()`, timeline, layers, frames, library, selection).
- Headless: `cargo run -p ac-cli -- run examples/scripts/stars.py --gif stars.gif`

See [SCRIPTING.md](SCRIPTING.md) for the API, examples and the safety model.

## Keyboard

| Key | Action | Key | Action |
|---|---|---|---|
| V | Selection | F5 / Shift+F5 | Insert / remove frames |
| Q | Free Transform | F6 / Shift+F6 | Insert / clear keyframe |
| R / O / N | Rectangle / Oval / Line | F7 | Blank keyframe |
| Y | Pencil | F8 / Ctrl+F8 | Convert to / new symbol |
| K | Paint Bucket | Delete | Delete objects, or clear selected frames |
| H or Space | Hand | Ctrl+X / C / V | Cut / copy / paste (Shift+V = in place) |
| Arrows | Nudge (Shift = 10 px) | Ctrl+D / Ctrl+A / Ctrl+B | Duplicate / select all / break apart |
| Enter | Play / stop | Ctrl+Z / Ctrl+Shift+Z | Undo / redo |
| Esc | Deselect, then leave symbol | Ctrl+1 / Ctrl+2 | 100% / fit |
| F9 | Actions panel | F2 | Output panel |
| Ctrl+T | Transform panel | Ctrl+L | Library |
| Ctrl+G / Ctrl+Shift+G | Group / Ungroup | Ctrl+K / Ctrl+I | Align / Info panels |
| Ctrl+R | Import to stage | Ctrl+Shift+F12 | Publish Settings |
| Ctrl+N / Ctrl+O / Ctrl+S | New / Open / Save | Ctrl+' / Ctrl+; | Grid / Guides |
| W / G | 3D Rotation / Translation | M / U / B | Bone / Width / Fluid Brush |
| Ctrl+Shift+H | Add shape hint | Shift+F2 | Scene panel |
| Ctrl+Alt+B | Enable Simple Buttons | Ctrl+drag last frame | Lengthen / shorten a layer |
| J | Object Drawing on/off | Esc | Deselect (merges a floating shape selection) |

## UI smoke testing

`tools/ui_drive.py` drives the real app under Xvfb with synthetic mouse and
keyboard input (needs `pip install python-xlib`). See `log/devlog.md`.

See `CLAUDE.md` for the full feature map, architecture and roadmap.

## Legal and provenance

AnimationCraft is an independent project. It is **not affiliated with,
endorsed by or sponsored by Adobe**; "Adobe", "Animate" and "Flash" are
trademarks of Adobe and are used here only to describe compatibility.

It is written **clean-room**: from public documentation and the observed
behaviour of the user interface, never from Adobe's code, decompiled or
disassembled programs, or files taken from an Adobe install (icons, presets,
samples, templates). `.fla`/XFL support is for interoperability and was built
from the published structure of the format and files this project wrote
itself. The rules every contribution follows are in
[CONTRIBUTING.md](CONTRIBUTING.md); please report anything that looks out of
place.

Licensed `MIT OR Apache-2.0`. Third-party crates keep their own licences
(MIT/Apache/BSD-style, plus MPL-2.0 for the Symphonia audio decoders and usvg,
used unmodified).
