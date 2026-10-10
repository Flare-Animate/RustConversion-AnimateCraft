# AnimateCraft (demo)

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

## UI smoke testing

`tools/ui_drive.py` drives the real app under Xvfb with synthetic mouse and
keyboard input (needs `pip install python-xlib`). See `log/devlog.md`.

See `CLAUDE.md` for the full feature map, architecture and roadmap.
