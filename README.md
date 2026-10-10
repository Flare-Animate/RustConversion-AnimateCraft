# AnimationCraft (demo)

Open-source 2D animation authoring in Rust, following the Adobe Animate workflow:
symbols, layers, keyframes, classic and shape tweens, onion skin, and a familiar
Tools / Stage / Timeline / Properties / Library workspace.

Not affiliated with Adobe.

## Run it

Requires Rust 1.80+ (https://rustup.rs).

```sh
cargo run --release -p ac-app          # the editor (opens the demo scene)
cargo run -p ac-cli -- info demo       # inspect the demo document
cargo run -p ac-cli -- gif demo out.gif
cargo run -p ac-cli -- video demo out.mp4   # needs ffmpeg
cargo test --workspace
```

Linux needs the usual X11/Wayland + OpenGL libraries (any desktop has them).

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
   Double-click an instance (or a Library item) to edit its timeline; **Esc** returns.
6. **Library**: drag an item onto the stage; right-click to rename, duplicate,
   change type or delete.
7. File → Export → Animated GIF / PNG / SVG.

## Tools

Two-column Tools panel, organised like Animate (and PhotoCraft): each button is
a group; a corner triangle means more tools inside: **right-click or
long-press** to pick one. **…** at the bottom opens *Edit Toolbar* to hide
tools. Options for the active tool are in Properties ▸ **Tool**.

| Group | Tools (shortcut) |
|---|---|
| Select | Selection (V), Subselection (A), Free Transform (Q: scale, **skew**, rotate around a movable transformation point; **Distort** and **Envelope** modes), Gradient Transform (F), **3D Rotation (W)**, **3D Translation (G)**, Lasso (L), Polygon Lasso |
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
- **Width (U)**: drag out from a stroke to make it thicker there.
- **Gradients**: Properties ▸ Object ▸ Fill ▸ Linear/Radial (edit stops there),
  then the Gradient Transform tool (F) to move, rotate and stretch.
- **Camera (C)**: adds a Camera layer; drag pans, scroll zooms, Shift+drag
  rotates. Keyframe and tween it like any layer; playback shows the camera view.
- **Rotation (Shift+H)** turns the view only (double-click to reset).

### Files, import and export

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

### Layers, masks and guides

Right-click a layer name: **New Folder**, **Mask** (the layer's shapes reveal
the layer below), **Guide**, **Add Classic Motion Guide** (classic tweens on
the layer below follow the first stroke on the guide; tick *Orient to path* in
Frame properties), **Layer Properties**. Drag layers into and out of folders;
click a folder's triangle to collapse it.

### Objects

- **Group / Ungroup** (Ctrl+G / Ctrl+Shift+G), **Align panel** (Ctrl+K) and
  **Info panel** (Ctrl+I), also in Modify › Align.
- **Filters and blending** in Properties › Object: Drop Shadow, Blur, Glow,
  Bevel, Adjust Color (they tween), and 14 blend modes.
- **Instance names** for symbols, text, bitmaps and groups; frame scripts can
  use them (`this.ball.x += 5`).
- Text can use any **installed font** (searchable picker).
- **View › Rulers** (Ctrl+Alt+Shift+R): drag guides out of the rulers, drop a
  guide back on a ruler to delete it. **View › Grid** (Ctrl+') with Snap to
  Grid and Edit Grid; objects and drawing tools snap to guides and the grid.

### Library

Symbols and bitmaps are listed together. The Library fills the rest of the right column under Properties (Properties
grows first and pushes it down). Search, sortable Name/Type/Uses/Frames
columns, an animated preview with play, and drag-to-stage. **View ▸ Library
position ▸ Own panel on the left** docks it beside the Tools; Ctrl+L hides it.

## Scripting

- **Window ▸ Actions (F9):** frame scripts in JavaScript (`this.stop()`,
  `this.gotoAndPlay("label")`…), sandboxed, run during playback.
- **Commands ▸ Script Console:** automate the editor in **JavaScript
  (built in), Python, Node.js or Ruby**, or any language via a one-line JSON
  protocol. One undo step per run.
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

## UI smoke testing

`tools/ui_drive.py` drives the real app under Xvfb with synthetic mouse and
keyboard input (needs `pip install python-xlib`). See `log/devlog.md`.

See `CLAUDE.md` for the full feature map, architecture and roadmap.
