# Scripting AnimateCraft

AnimateCraft has two kinds of script. They look similar but have very
different jobs and safety rules, so they are kept apart on purpose.

| | Frame scripts | Authoring scripts |
|---|---|---|
| Animate equivalent | Actions panel code (ActionScript / HTML5 Canvas JS) | JSFL commands |
| Lives in | the document, on a keyframe | a file on your computer, or the Script Console |
| Language | JavaScript (CreateJS-style API) | JavaScript (built in), Python, Node.js, Ruby, or any language |
| Runs | during playback only | when *you* run it |
| Can do | `stop`, `play`, `gotoAndPlay/Stop`, `trace` | everything the menus can do: draw, edit, timeline, library, export, save |
| Safety | sandboxed: no files, network or editor; 100 ms + 32 MB per frame | external languages run with your permissions; the app asks first |

## Frame scripts (Window ▸ Actions, F9)

Select a keyframe, press **F9**, write code, **Apply** (Ctrl+Enter). The
**Insert snippet** menu has the common ones. Control ▸ **Enable Frame Scripts**
turns them on or off during playback.

```js
// `this` is the timeline, as in Animate's HTML5 Canvas documents.
this.stop();
this.gotoAndPlay("start");          // frame label…
this.gotoAndStop(0);                // …or 0-based frame number (HTML5 Canvas numbering)
trace("frame", this.currentFrame, "of", this.totalFrames);
this.loops = (this.loops || 0) + 1; // `this.x` persists between frames
```

Frame scripts never run when a document is opened, cannot read or write
files, cannot reach the network, and cannot change the document. A runaway
loop is stopped after 100 ms and memory is capped at 32 MB. That is what makes
it safe to open files other people send you.

## Authoring scripts (Commands ▸ Script Console)

Pick a language, write or load a script, press **Run**. A whole run is **one
undo step**. Every language gets the same commands (Commands ▸ Command
Reference lists them all):

```js
// JavaScript — built in, nothing to install
ac.layer.add({ name: "Stars" });
ac.draw.polystar({ cx: 100, cy: 100, r: 30, star: true, fill: "#FFC41F" });
ac.select.layer({ layer: "Stars" });
ac.selection.rotate({ degrees: 30 });
ac.export.gif({ path: "out.gif" });
```

```python
# Python — uses the Python installed on your computer
import animatecraft as ac
ac.layer.add(name="Stars")
ac.draw.polystar(cx=100, cy=100, r=30, star=True, fill="#FFC41F")
info = ac.doc.info()
print(info["frames"], "frames")
```

```js
// Node.js (.mjs)
import ac from "./animatecraft.mjs";
ac.draw.rect({ x: 10, y: 10, w: 80, h: 40, radius: 8 });
```

```ruby
# Ruby
require "animatecraft"
AnimateCraft.call("draw.text", x: 20, y: 20, text: "Hello", size: 32)
```

Run a file from the console's **Script file** box, or headless:

```sh
cargo run -p ac-cli -- run examples/scripts/stars.py --gif stars.gif
cargo run -p ac-cli -- run examples/scripts/grid.js --doc demo --save out.acx.json
cargo run -p ac-cli -- commands          # list every command + the protocol
```

Conventions: frames and layers are numbered from 1 (as in the timeline);
colours are `"#RRGGBB"` or `"#RRGGBBAA"`, `null` means none; relative paths
are relative to the script's folder.

### Transforms, 3D, gradients, bones and the camera

The tools added in M0.3 are scriptable too (coordinates are stage pixels,
frames are 1-based):

```js
ac.selection.skew({ h: 20, v: 0 });                    // degrees, about the selection centre
ac.selection.distort({ corners: [[0,0],[120,10],[110,90],[0,100]] }); // TL, TR, BR, BL (shapes)
ac.selection.set_3d({ rx: 0, ry: 45, rz: 0, z: 80 });  // 3D rotation + depth
ac.doc.set({ perspective_angle: 70, vanishing_x: 275, vanishing_y: 200 });
ac.selection.set_gradient({ kind: "radial", colors: ["#FFFFFF", "#2D8CEB"] }); // or "linear" / "none"
ac.bone.add({ x1: 100, y1: 100, x2: 180, y2: 100 });   // from the object at (x1,y1) to the one at (x2,y2)
ac.bone.drag({ x: 260, y: 100, to_x: 220, to_y: 180 }); // pose with inverse kinematics
ac.camera.set({ x: 275, y: 200, zoom: 1.5, rotation: 10, frame: 24 });
```

### Any other language

Python, Node and Ruby use tiny helper libraries, but the protocol is simple
enough to use from anything that can print and read lines. The script writes
one line per command to stdout:

    \x1eAC {"id": 1, "cmd": "draw.rect", "args": {"x": 10, "y": 10, "w": 50, "h": 50}}

(`\x1e` is the ASCII record-separator character) and reads one JSON line back
from stdin: `{"id": 1, "result": …}` or `{"id": 1, "error": "…"}`. Anything
else the script prints goes to the Output panel. Add the interpreter for a
file extension in `ScriptOptions::interpreters` (Lua is preconfigured).

## Does supporting several languages cause problems?

Mostly no, because of how it is built:

- **One API.** Languages don't get their own bindings; they all call the same
  command registry (`ac_core::commands`). A new command is instantly available
  everywhere, and behaviour can't drift between languages.
- **No extra dependencies for the app.** Only JavaScript is embedded
  (QuickJS, ~1 MB). Python, Node and Ruby are *not* bundled: they're used if
  installed. If one is missing you get a clear "couldn't start python" message.
- **Isolation.** External scripts are separate processes with a timeout
  (30 s default), a line-size limit and a command limit; a crash or infinite
  loop in a script can't take the editor down.

The real costs to keep in mind:

- **Security.** An external script is a normal program with your
  permissions. That's why the app asks before running one, and why documents
  can never carry them. Treat scripts like any downloaded program.
- **Portability of shared scripts.** A Python script only runs for people who
  have Python. For scripts meant to be shared, built-in JavaScript is the
  safe choice.
- **Exported animations.** Frame scripts stay JavaScript only, because an
  HTML5/web export can run JavaScript natively; Python in a browser would need
  ~10 MB of Pyodide per movie.
