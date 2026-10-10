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
2. Select the ball (**V**), drag it at a frame between keyframes: a keyframe is
   added for you and the tween updates.
3. Draw a shape with **R** or **O**, press **F8** to make it a symbol.
4. Move to frame 20, press **F6**, move the shape, then right-click frame 1 in
   the timeline → **Create Classic Tween**. Set **Ease** in Properties.
5. Double-click a symbol in the Library to edit its own timeline; **Esc** to return.
6. File → Export → Animated GIF / PNG / SVG.

## Keyboard

| Key | Action | Key | Action |
|---|---|---|---|
| V | Selection | F5 / Shift+F5 | Insert / remove frame |
| Q | Free Transform (drag outside a corner to rotate) | F6 / Shift+F6 | Insert / clear keyframe |
| R / O / N | Rectangle / Oval / Line | F7 | Blank keyframe |
| Y | Pencil | F8 / Ctrl+F8 | Convert to / new symbol |
| K | Paint Bucket | Enter | Play / stop |
| H or Space | Hand | Ctrl+Z / Ctrl+Shift+Z | Undo / redo |
| Shift (drag) | Constrain | Ctrl+1 / Ctrl+2 | 100% / fit |

See `CLAUDE.md` for the full feature map, architecture and roadmap.
