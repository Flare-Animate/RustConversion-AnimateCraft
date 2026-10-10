//! The command registry: every scriptable action has a stable dotted id and
//! JSON arguments. JavaScript, Python and any other language reach the editor
//! only through `execute`, so they all get the same capabilities, and adding
//! a command here makes it available everywhere at once.
//!
//! Conventions: frame and layer numbers are **1-based**, as shown in the
//! timeline. Colours are `"#RRGGBB"`, `"#RRGGBBAA"` or `null` for none.
//! Drawing commands draw on the current layer at the current frame.

use crate::editor::{Editor, FrameSel, Selection};
use crate::geom::{pt, Matrix, Path, Pt, Seg};
use crate::model::*;
use serde_json::{json, Value};

pub struct CommandInfo {
    pub id: &'static str,
    pub args: &'static str,
    pub summary: &'static str,
}

macro_rules! cmds {
    ($( $id:literal, $args:literal, $summary:literal; )*) => {
        pub const COMMANDS: &[CommandInfo] = &[ $( CommandInfo { id: $id, args: $args, summary: $summary } ),* ];
    };
}

cmds! {
    "doc.info", "", "Document size, fps, frame count, layers, library and current position";
    "doc.set", "width?, height?, fps?, background?, perspective_angle?, vanishing_x?, vanishing_y?", "Change document settings (incl. 3D perspective)";
    "timeline.goto", "frame", "Move the playhead";
    "timeline.select", "layer?, frame?, to_layer?, to_frame?", "Select frames (defaults: current layer/frame)";
    "timeline.insert_frames", "frames? (1), layer?, frame?", "F5: insert frames";
    "timeline.remove_frames", "layer?, frame?, to_frame?", "Shift+F5: remove frames";
    "timeline.insert_keyframe", "layer?, frame?", "F6: insert keyframe (copies content)";
    "timeline.insert_blank_keyframe", "layer?, frame?", "F7: insert blank keyframe";
    "timeline.clear_keyframe", "layer?, frame?", "Shift+F6: clear keyframe";
    "timeline.clear_frames", "layer?, frame?, to_frame?", "Clear frames (make them empty)";
    "timeline.set_tween", "kind ('classic'|'shape'|'none'), ease? (-100..100), rotate?, layer?, frame?", "Create or remove a tween on the span at the target";
    "timeline.set_label", "label, layer?, frame?", "Frame label on the keyframe at the target";
    "timeline.set_script", "script, layer?, frame?", "Frame script (JavaScript) on the keyframe at the target";
    "timeline.move_frames", "by, layer?, frame?, to_frame?", "Drag frames left/right";
    "layer.list", "", "All layers with flags and frame counts";
    "layer.add", "name?", "New layer above the current one; returns its number";
    "layer.delete", "layer?", "Delete a layer";
    "layer.select", "layer", "Make a layer current (number or name)";
    "layer.set", "layer?, name?, visible?, locked?, outline?", "Rename or change layer flags";
    "draw.rect", "x, y, w, h, radius?, fill?, stroke?, stroke_width?", "Rectangle";
    "draw.oval", "x, y, w, h, fill?, stroke?, stroke_width?", "Oval inside a box";
    "draw.line", "x1, y1, x2, y2, stroke?, stroke_width?", "Straight line";
    "draw.polystar", "cx, cy, r, sides? (5), star? (false), inner? (0.5), fill?, stroke?", "Polygon or star";
    "draw.path", "points [[x,y]...], closed?, smooth?, fill?, stroke?, stroke_width?", "Path through points";
    "draw.text", "x, y, text, size? (24), color?, font? ('sans'|'mono'), align?", "Static text";
    "select.all", "", "Select everything editable on the current frame";
    "select.none", "", "Clear the selection";
    "select.layer", "layer?", "Select every object on one layer (default: current) at the current frame";
    "select.at", "x, y, add?", "Select the topmost object at a point";
    "select.rect", "x, y, w, h, add?", "Select objects fully inside a box";
    "selection.info", "", "Selected objects with kind, name and bounds";
    "selection.move", "dx, dy", "Move the selection";
    "selection.set_position", "x, y", "Move the selection's top-left to x, y";
    "selection.scale", "sx, sy?", "Scale around the selection centre";
    "selection.rotate", "degrees", "Rotate around the selection centre";
    "selection.set", "fill?, stroke?, stroke_width?, alpha?, tint?, tint_amount?, name?, text?, size?", "Change properties of every selected object";
    "selection.delete", "", "Delete the selection";
    "selection.duplicate", "", "Duplicate (offset by 10 px)";
    "selection.to_symbol", "name, kind? ('graphic'|'movieclip'|'button')", "F8: convert to symbol; returns its id";
    "selection.break_apart", "", "Ctrl+B on a symbol instance";
    "selection.skew", "h? (deg), v? (deg)", "Skew horizontally / vertically around the selection centre";
    "selection.set_3d", "rx?, ry?, rz? (deg), z?", "3D rotation and depth (perspective-projected)";
    "selection.distort", "corners [[x,y] x4: TL, TR, BR, BL]", "Distort shapes onto a quadrilateral";
    "selection.set_gradient", "kind ('linear'|'radial'|'none'), colors [\"#RRGGBB\"...]", "Gradient fill on selected shapes";
    "bone.add", "x1, y1, x2, y2", "Bone from the object at (x1,y1) to the object at (x2,y2) on the current layer";
    "bone.list", "layer?", "Bones on a layer";
    "bone.clear", "layer?", "Remove all bones on a layer";
    "bone.drag", "x, y, to_x, to_y", "IK: drag the bonded object point at (x,y) to (to_x,to_y), posing the chain";
    "camera.add", "", "Add a camera layer to the scene";
    "camera.set", "x?, y?, zoom?, rotation?, frame?", "Camera centre, zoom (1 = 100%) and rotation at a frame (makes a keyframe)";
    "camera.remove", "", "Remove the camera layer";
    "selection.arrange", "front (bool)", "Bring to front / send to back";
    "library.list", "", "Library items";
    "library.place", "symbol (name or id), x, y", "Place an instance on the current layer";
    "library.rename", "symbol, name", "Rename a library item";
    "library.delete", "symbol", "Delete a library item and its instances";
    "symbol.edit", "symbol", "Enter a symbol's timeline";
    "symbol.exit", "", "Return to the main scene";
    "edit.undo", "", "Undo";
    "edit.redo", "", "Redo";
}

pub fn find(id: &str) -> Option<&'static CommandInfo> {
    COMMANDS.iter().find(|c| c.id == id)
}

// ------------------------------------------------------------------ argument helpers

fn num(a: &Value, k: &str) -> Option<f32> {
    a.get(k).and_then(Value::as_f64).map(|v| v as f32)
}
fn req(a: &Value, k: &str) -> Result<f32, String> {
    num(a, k).ok_or_else(|| format!("missing number '{k}'"))
}
fn string<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}
fn boolean(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(Value::as_bool)
}

/// `"#RRGGBB"`, `"#RRGGBBAA"`, `[r,g,b(,a)]`, or null (= none).
fn color_arg(a: &Value, k: &str) -> Result<Option<Option<Color>>, String> {
    match a.get(k) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) => Color::from_hex(s).map(|c| Some(Some(c))).ok_or_else(|| format!("bad colour '{s}' for '{k}' (use \"#RRGGBB\")")),
        Some(Value::Array(v)) if v.len() >= 3 => {
            let c = |i: usize| v.get(i).and_then(Value::as_f64).unwrap_or(255.0).clamp(0.0, 255.0) as u8;
            Ok(Some(Some(Color { r: c(0), g: c(1), b: c(2), a: c(3) })))
        }
        Some(other) => Err(format!("bad colour {other} for '{k}'")),
    }
}

fn hex(c: Color) -> String {
    if c.a == 255 { c.to_hex() } else { format!("{}{:02X}", c.to_hex(), c.a) }
}

/// Layer by 1-based number or by name.
fn layer_arg(ed: &Editor, a: &Value, k: &str) -> Result<Option<usize>, String> {
    let n = ed.timeline().layers.len();
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(v)) => {
            let i = v.as_u64().unwrap_or(0) as usize;
            if i == 0 || i > n {
                return Err(format!("layer {i} out of range (1..={n})"));
            }
            Ok(Some(i - 1))
        }
        Some(Value::String(s)) => ed.timeline().layers.iter().position(|l| l.name == *s).map(Some).ok_or_else(|| format!("no layer named '{s}'")),
        Some(other) => Err(format!("bad layer {other}")),
    }
}

fn frame_arg(a: &Value, k: &str) -> Result<Option<u32>, String> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let f = v.as_u64().ok_or_else(|| format!("'{k}' must be a frame number"))?;
            if f == 0 {
                return Err(format!("'{k}': frames are numbered from 1"));
            }
            Ok(Some(f as u32 - 1))
        }
    }
}

/// Apply optional layer/frame/to_layer/to_frame args as the frame target.
fn target(ed: &mut Editor, a: &Value) -> Result<(), String> {
    let l = layer_arg(ed, a, "layer")?;
    let f = frame_arg(a, "frame")?;
    let tl = layer_arg(ed, a, "to_layer")?;
    let tf = frame_arg(a, "to_frame")?;
    if l.is_none() && f.is_none() && tl.is_none() && tf.is_none() {
        return Ok(());
    }
    let l0 = l.unwrap_or(ed.layer);
    let f0 = f.unwrap_or(ed.frame);
    ed.select_frames(FrameSel::between((l0, f0), (tl.unwrap_or(l0), tf.unwrap_or(f0))));
    Ok(())
}

fn symbol_arg(ed: &Editor, a: &Value) -> Result<SymbolId, String> {
    match a.get("symbol") {
        Some(Value::Number(n)) => {
            let id = n.as_u64().unwrap_or(0) as SymbolId;
            ed.doc.symbol(id).map(|_| id).ok_or_else(|| format!("no symbol with id {id}"))
        }
        Some(Value::String(s)) => ed.doc.library.iter().find(|x| x.name == *s).map(|x| x.id).ok_or_else(|| format!("no library item named '{s}'")),
        _ => Err("missing 'symbol' (name or id)".into()),
    }
}

fn shape_style(a: &Value, default_fill: Option<Color>) -> Result<(Option<Color>, Option<Stroke>), String> {
    let fill = color_arg(a, "fill")?.unwrap_or(default_fill);
    let width = num(a, "stroke_width").unwrap_or(1.0);
    let stroke = color_arg(a, "stroke")?.unwrap_or(None).map(|c| Stroke { color: c, width });
    Ok((fill, stroke))
}

fn add_shape(ed: &mut Editor, name: &str, path: Path, fill: Option<Color>, stroke: Option<Stroke>) -> Result<Value, String> {
    if fill.is_none() && stroke.is_none() {
        return Err("shape needs a fill or a stroke".into());
    }
    let s = ed.add_element(Element::shape(name, Shape { path, fill, stroke, ..Default::default() })).ok_or("current layer is locked or hidden")?;
    Ok(json!({ "layer": s.layer + 1, "index": s.element }))
}

fn element_json(ed: &Editor, s: Selection) -> Value {
    let Some(el) = ed.element_at(s) else { return Value::Null };
    let (kind, extra) = match &el.kind {
        ElementKind::Shape(sh) => ("shape", json!({ "fill": sh.fill.map(hex), "stroke": sh.stroke.map(|s| hex(s.color)), "stroke_width": sh.stroke.map(|s| s.width) })),
        ElementKind::Instance(i) => ("instance", json!({ "symbol": ed.doc.symbol(i.symbol).map(|s| s.name.clone()) })),
        ElementKind::Text(t) => ("text", json!({ "text": t.text, "size": t.size, "color": hex(t.color) })),
    };
    let b = ed.bounds_of(&[s]);
    json!({
        "layer": s.layer + 1, "index": s.element, "name": el.name, "kind": kind, "alpha": el.alpha,
        "x": b.map(|b| b.0.x), "y": b.map(|b| b.0.y),
        "width": b.map(|b| b.1.x - b.0.x), "height": b.map(|b| b.1.y - b.0.y),
        "props": extra,
    })
}

fn center(ed: &Editor) -> Result<Pt, String> {
    let (lo, hi) = ed.selection_bounds().ok_or("nothing selected")?;
    Ok(pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
}

fn need_selection(ed: &Editor) -> Result<(), String> {
    if ed.selection.is_empty() {
        Err("nothing selected (use select.at, select.rect or select.all first)".into())
    } else {
        Ok(())
    }
}

// ------------------------------------------------------------------ dispatch

pub fn execute(ed: &mut Editor, id: &str, a: &Value) -> Result<Value, String> {
    let a = if a.is_null() { &json!({}) } else { a };
    match id {
        "doc.info" => {
            let tl = ed.timeline();
            Ok(json!({
                "width": ed.doc.width, "height": ed.doc.height, "fps": ed.doc.fps,
                "background": hex(ed.doc.background),
                "frames": tl.frame_count(),
                "current_frame": ed.frame + 1, "current_layer": ed.layer + 1,
                "editing_symbol": ed.editing.and_then(|i| ed.doc.symbol(i)).map(|s| s.name.clone()),
                "layers": tl.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
                "library": ed.doc.library.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            }))
        }
        "doc.set" => {
            let d = &ed.doc;
            let bg = color_arg(a, "background")?.flatten().unwrap_or(d.background);
            let (w, h, fps) = (num(a, "width").unwrap_or(d.width), num(a, "height").unwrap_or(d.height), num(a, "fps").unwrap_or(d.fps));
            if w < 1.0 || h < 1.0 || !(0.1..=240.0).contains(&fps) {
                return Err("width/height must be ≥ 1 and fps 0.1–240".into());
            }
            let pa = num(a, "perspective_angle");
            let vx = num(a, "vanishing_x");
            let vy = num(a, "vanishing_y");
            ed.set_doc_props(w, h, fps, bg);
            if let Some(v) = pa {
                ed.doc.perspective_angle = v.clamp(1.0, 179.0);
            }
            if vx.is_some() || vy.is_some() {
                let cur = ed.doc.perspective().vanishing;
                ed.doc.vanishing_point = Some(pt(vx.unwrap_or(cur.x), vy.unwrap_or(cur.y)));
            }
            Ok(Value::Null)
        }
        "timeline.goto" => {
            let f = frame_arg(a, "frame")?.ok_or("missing 'frame'")?;
            ed.set_frame(f);
            ed.frame_sel = None;
            Ok(Value::Null)
        }
        "timeline.select" => {
            target(ed, a)?;
            if ed.frame_sel.is_none() {
                ed.select_frames(FrameSel::cell(ed.layer, ed.frame));
            }
            Ok(Value::Null)
        }
        "timeline.insert_frames" => {
            target(ed, a)?;
            let n = num(a, "frames").unwrap_or(1.0).max(1.0) as u32;
            if n > 1 {
                let s = ed.frame_target();
                ed.select_frames(FrameSel { f1: s.f0 + n - 1, ..s });
            }
            ed.insert_frame();
            Ok(Value::Null)
        }
        "timeline.remove_frames" => {
            target(ed, a)?;
            ed.remove_frame();
            Ok(Value::Null)
        }
        "timeline.insert_keyframe" => {
            target(ed, a)?;
            ed.insert_keyframe();
            Ok(Value::Null)
        }
        "timeline.insert_blank_keyframe" => {
            target(ed, a)?;
            ed.insert_blank_keyframe();
            Ok(Value::Null)
        }
        "timeline.clear_keyframe" => {
            target(ed, a)?;
            ed.clear_keyframe();
            Ok(Value::Null)
        }
        "timeline.clear_frames" => {
            target(ed, a)?;
            ed.clear_frames();
            Ok(Value::Null)
        }
        "timeline.set_tween" => {
            target(ed, a)?;
            let ease = num(a, "ease").unwrap_or(0.0).clamp(-100.0, 100.0);
            let tween = match string(a, "kind").unwrap_or("classic") {
                "classic" => Tween::Classic { ease, rotate_turns: num(a, "rotate").unwrap_or(0.0) as i32 },
                "shape" => Tween::Shape { ease },
                "none" => Tween::None,
                k => return Err(format!("unknown tween kind '{k}' (classic, shape, none)")),
            };
            ed.set_tween(tween);
            Ok(Value::Null)
        }
        "timeline.set_label" => {
            target(ed, a)?;
            ed.set_frame_label(string(a, "label").unwrap_or(""));
            Ok(Value::Null)
        }
        "timeline.set_script" => {
            target(ed, a)?;
            ed.set_frame_script(string(a, "script").unwrap_or(""));
            Ok(Value::Null)
        }
        "timeline.move_frames" => {
            target(ed, a)?;
            if ed.frame_sel.is_none() {
                ed.select_frames(FrameSel::cell(ed.layer, ed.frame));
            }
            ed.move_frames(req(a, "by")? as i64);
            Ok(Value::Null)
        }
        "layer.list" => Ok(Value::Array(
            ed.timeline()
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| json!({ "layer": i + 1, "name": l.name, "visible": l.visible, "locked": l.locked, "outline": l.outline, "frames": l.end, "keyframes": l.keyframes.keys().map(|k| k + 1).collect::<Vec<_>>() }))
                .collect(),
        )),
        "layer.add" => {
            ed.add_layer();
            if let Some(n) = string(a, "name") {
                let i = ed.layer;
                ed.rename_layer(i, n);
            }
            Ok(json!(ed.layer + 1))
        }
        "layer.delete" => {
            if let Some(l) = layer_arg(ed, a, "layer")? {
                ed.layer = l;
            }
            if ed.timeline().layers.len() <= 1 {
                return Err("can't delete the only layer".into());
            }
            ed.delete_layer();
            Ok(Value::Null)
        }
        "layer.select" => {
            ed.layer = layer_arg(ed, a, "layer")?.ok_or("missing 'layer'")?;
            ed.frame_sel = None;
            Ok(Value::Null)
        }
        "layer.set" => {
            let i = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            if let Some(n) = string(a, "name") {
                ed.rename_layer(i, n);
            }
            let l = &ed.timeline().layers[i];
            let (v, lk, o) = (l.visible, l.locked, l.outline);
            if boolean(a, "visible").map(|x| x != v).unwrap_or(false) {
                ed.toggle_layer_flag(i, crate::editor::LayerFlag::Visible);
            }
            if boolean(a, "locked").map(|x| x != lk).unwrap_or(false) {
                ed.toggle_layer_flag(i, crate::editor::LayerFlag::Locked);
            }
            if boolean(a, "outline").map(|x| x != o).unwrap_or(false) {
                ed.toggle_layer_flag(i, crate::editor::LayerFlag::Outline);
            }
            Ok(Value::Null)
        }
        "draw.rect" => {
            let (fill, stroke) = shape_style(a, Some(Color::rgb(0x2D, 0x8C, 0xEB)))?;
            let p = Path::rounded_rect(req(a, "x")?, req(a, "y")?, req(a, "w")?, req(a, "h")?, num(a, "radius").unwrap_or(0.0));
            add_shape(ed, "Rectangle", p, fill, stroke)
        }
        "draw.oval" => {
            let (fill, stroke) = shape_style(a, Some(Color::rgb(0x2D, 0x8C, 0xEB)))?;
            let (x, y, w, h) = (req(a, "x")?, req(a, "y")?, req(a, "w")?, req(a, "h")?);
            add_shape(ed, "Oval", Path::ellipse(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0), fill, stroke)
        }
        "draw.line" => {
            let c = color_arg(a, "stroke")?.flatten().unwrap_or(Color::rgb(0, 0, 0));
            let p = Path { segs: vec![Seg::M(pt(req(a, "x1")?, req(a, "y1")?)), Seg::L(pt(req(a, "x2")?, req(a, "y2")?))] };
            add_shape(ed, "Line", p, None, Some(Stroke { color: c, width: num(a, "stroke_width").unwrap_or(1.0) }))
        }
        "draw.polystar" => {
            let (fill, stroke) = shape_style(a, Some(Color::rgb(0xFF, 0xC4, 0x1F)))?;
            let r = req(a, "r")?;
            let p = Path::polystar(req(a, "cx")?, req(a, "cy")?, r, r, num(a, "sides").unwrap_or(5.0) as usize, boolean(a, "star").unwrap_or(false), num(a, "inner").unwrap_or(0.5));
            add_shape(ed, "PolyStar", p, fill, stroke)
        }
        "draw.path" => {
            let pts: Vec<Pt> = a
                .get("points")
                .and_then(Value::as_array)
                .ok_or("missing 'points' [[x,y], ...]")?
                .iter()
                .filter_map(|p| Some(pt(p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32)))
                .collect();
            if pts.len() < 2 {
                return Err("need at least 2 points".into());
            }
            let closed = boolean(a, "closed").unwrap_or(false);
            let mut path = if boolean(a, "smooth").unwrap_or(false) { Path::smooth_polyline(&pts, 0.0) } else { Path::polygon(&pts) };
            if !closed {
                path.segs.retain(|s| !matches!(s, Seg::Z));
            } else if !matches!(path.segs.last(), Some(Seg::Z)) {
                path.segs.push(Seg::Z);
            }
            let (fill, stroke) = shape_style(a, None)?;
            let stroke = if fill.is_none() && stroke.is_none() { Some(Stroke { color: Color::rgb(0, 0, 0), width: num(a, "stroke_width").unwrap_or(1.0) }) } else { stroke };
            add_shape(ed, "Path", path, if closed { fill } else { None }, stroke)
        }
        "draw.text" => {
            let text = string(a, "text").ok_or("missing 'text'")?;
            let color = color_arg(a, "color")?.flatten().unwrap_or(Color::rgb(0, 0, 0));
            let mut el = Element::text(text, num(a, "size").unwrap_or(24.0).max(1.0), color, pt(req(a, "x")?, req(a, "y")?));
            if let ElementKind::Text(t) = &mut el.kind {
                if string(a, "font") == Some("mono") {
                    t.font = crate::text::FontFace::Mono;
                }
                t.align = match string(a, "align") {
                    Some("center") => crate::text::Align::Center,
                    Some("right") => crate::text::Align::Right,
                    _ => crate::text::Align::Left,
                };
            }
            let s = ed.add_element(el).ok_or("current layer is locked or hidden")?;
            Ok(json!({ "layer": s.layer + 1, "index": s.element }))
        }
        "select.all" => {
            ed.select_all();
            Ok(json!(ed.selection.len()))
        }
        "select.layer" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            ed.select_all();
            ed.selection.retain(|s| s.layer == li);
            ed.layer = li;
            Ok(json!(ed.selection.len()))
        }
        "select.none" => {
            ed.selection.clear();
            Ok(Value::Null)
        }
        "select.at" => {
            let hit = ed.hit_test(pt(req(a, "x")?, req(a, "y")?));
            ed.select(hit, boolean(a, "add").unwrap_or(false));
            Ok(json!(hit.is_some()))
        }
        "select.rect" => {
            let (x, y) = (req(a, "x")?, req(a, "y")?);
            ed.select_in_rect(pt(x, y), pt(x + req(a, "w")?, y + req(a, "h")?), boolean(a, "add").unwrap_or(false));
            Ok(json!(ed.selection.len()))
        }
        "selection.info" => Ok(Value::Array(ed.selection.clone().into_iter().map(|s| element_json(ed, s)).collect())),
        "selection.move" => {
            need_selection(ed)?;
            let (dx, dy) = (num(a, "dx").unwrap_or(0.0), num(a, "dy").unwrap_or(0.0));
            ed.modify_selected("Move", |e| {
                e.matrix.tx += dx;
                e.matrix.ty += dy;
            });
            Ok(Value::Null)
        }
        "selection.set_position" => {
            need_selection(ed)?;
            let (lo, _) = ed.selection_bounds().ok_or("selection has no size")?;
            let (dx, dy) = (req(a, "x")? - lo.x, req(a, "y")? - lo.y);
            ed.modify_selected("Move", |e| {
                e.matrix.tx += dx;
                e.matrix.ty += dy;
            });
            Ok(Value::Null)
        }
        "selection.scale" => {
            need_selection(ed)?;
            let c = center(ed)?;
            let sx = req(a, "sx")?;
            let sy = num(a, "sy").unwrap_or(sx);
            let m = Matrix::translate(c.x, c.y).concat(&Matrix::scale(sx, sy)).concat(&Matrix::translate(-c.x, -c.y));
            ed.modify_selected("Scale", |e| e.matrix = m.concat(&e.matrix));
            Ok(Value::Null)
        }
        "selection.rotate" => {
            need_selection(ed)?;
            let c = center(ed)?;
            let m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(req(a, "degrees")?.to_radians())).concat(&Matrix::translate(-c.x, -c.y));
            ed.modify_selected("Rotate", |e| e.matrix = m.concat(&e.matrix));
            Ok(Value::Null)
        }
        "selection.set" => {
            need_selection(ed)?;
            let fill = color_arg(a, "fill")?;
            let stroke = color_arg(a, "stroke")?;
            let sw = num(a, "stroke_width");
            let alpha = num(a, "alpha");
            let tint = color_arg(a, "tint")?;
            let amt = num(a, "tint_amount");
            let name = string(a, "name").map(String::from);
            let text = string(a, "text").map(String::from);
            let size = num(a, "size");
            ed.modify_selected("Set Properties", |e| {
                if let Some(v) = alpha {
                    e.alpha = v.clamp(0.0, 1.0);
                }
                if let Some(n) = &name {
                    e.name = n.clone();
                }
                if let Some(t) = tint {
                    e.tint = t.map(|c| (c, amt.unwrap_or(0.6)));
                } else if let (Some(v), Some((c, _))) = (amt, e.tint) {
                    e.tint = Some((c, v));
                }
                match &mut e.kind {
                    ElementKind::Shape(s) => {
                        if let Some(f) = fill {
                            s.fill = f;
                        }
                        if let Some(st) = stroke {
                            s.stroke = st.map(|c| Stroke { color: c, width: sw.or(s.stroke.map(|x| x.width)).unwrap_or(1.0) });
                        } else if let (Some(w), Some(st)) = (sw, s.stroke.as_mut()) {
                            st.width = w;
                        }
                    }
                    ElementKind::Text(t) => {
                        if let Some(Some(c)) = fill {
                            t.color = c;
                        }
                        if let Some(x) = &text {
                            t.text = x.clone();
                        }
                        if let Some(v) = size {
                            t.size = v.max(1.0);
                        }
                    }
                    ElementKind::Instance(_) => {}
                }
            });
            Ok(Value::Null)
        }
        "selection.delete" => Ok(json!(ed.delete_selected())),
        "selection.duplicate" => {
            need_selection(ed)?;
            ed.duplicate();
            Ok(Value::Null)
        }
        "selection.to_symbol" => {
            need_selection(ed)?;
            let kind = match string(a, "kind").unwrap_or("graphic") {
                "graphic" => SymbolKind::Graphic,
                "movieclip" | "movie_clip" => SymbolKind::MovieClip,
                "button" => SymbolKind::Button,
                k => return Err(format!("unknown symbol kind '{k}'")),
            };
            let name = string(a, "name").ok_or("missing 'name'")?;
            Ok(json!(ed.convert_to_symbol(name, kind).ok_or("could not convert (locked layer?)")?))
        }
        "selection.break_apart" => {
            need_selection(ed)?;
            ed.break_apart();
            Ok(Value::Null)
        }
        "selection.skew" => {
            need_selection(ed)?;
            let c = center(ed)?;
            let (h, v) = (num(a, "h").unwrap_or(0.0).to_radians(), num(a, "v").unwrap_or(0.0).to_radians());
            // x' = x + tan(h)·y ; y' = y + tan(v)·x
            let sk = Matrix { a: 1.0, b: v.tan(), c: h.tan(), d: 1.0, tx: 0.0, ty: 0.0 };
            let m = Matrix::translate(c.x, c.y).concat(&sk).concat(&Matrix::translate(-c.x, -c.y));
            ed.modify_selected("Skew", |e| e.matrix = m.concat(&e.matrix));
            Ok(Value::Null)
        }
        "selection.set_3d" => {
            need_selection(ed)?;
            let (rx, ry, rz, z) = (num(a, "rx"), num(a, "ry"), num(a, "rz"), num(a, "z"));
            ed.modify_selected("3D", |e| {
                let mut t = e.three_d.unwrap_or_default();
                if let Some(v) = rx { t.rx = v; }
                if let Some(v) = ry { t.ry = v; }
                if let Some(v) = rz { t.rz = v; }
                if let Some(v) = z { t.z = v; }
                e.three_d = if t.is_identity() { None } else { Some(t) };
            });
            Ok(Value::Null)
        }
        "selection.distort" => {
            need_selection(ed)?;
            let cs: Vec<Pt> = a.get("corners").and_then(Value::as_array).ok_or("missing 'corners' [[x,y] x4]")?
                .iter().filter_map(|p| Some(pt(p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32))).collect();
            if cs.len() != 4 {
                return Err("'corners' needs 4 points: top-left, top-right, bottom-right, bottom-left".into());
            }
            let (lo, hi) = ed.selection_bounds().ok_or("selection has no size")?;
            let quad = [cs[0], cs[1], cs[2], cs[3]];
            let mut skipped = 0;
            for s in ed.selection.clone() {
                if !matches!(ed.element_at(s).map(|e| e.kind), Some(ElementKind::Shape(_))) { skipped += 1; }
            }
            ed.modify_selected("Distort", |e| {
                let m = e.matrix;
                if let (ElementKind::Shape(sh), Some(inv)) = (&mut e.kind, m.invert()) {
                    let world = sh.path.transformed(&m);
                    sh.path = crate::deform::distort(&world, lo, hi, &quad).transformed(&inv);
                    sh.warp = None;
                }
            });
            Ok(json!({ "skipped_non_shapes": skipped }))
        }
        "selection.set_gradient" => {
            need_selection(ed)?;
            let kind = match string(a, "kind").unwrap_or("linear") {
                "linear" => Some(GradientKind::Linear),
                "radial" => Some(GradientKind::Radial),
                "none" => None,
                k => return Err(format!("unknown gradient kind '{k}'")),
            };
            let colors: Vec<Color> = match a.get("colors").and_then(Value::as_array) {
                Some(v) => v.iter().filter_map(|c| c.as_str().and_then(Color::from_hex)).collect(),
                None => vec![Color::rgb(255, 255, 255), Color::rgb(0, 0, 0)],
            };
            if kind.is_some() && colors.len() < 2 {
                return Err("need at least 2 colors".into());
            }
            let n = colors.len();
            let stops: Vec<(f32, Color)> = colors.into_iter().enumerate().map(|(i, c)| (i as f32 / (n - 1).max(1) as f32, c)).collect();
            ed.modify_selected("Gradient", |e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    sh.gradient = kind.and_then(|k| sh.path.bounds().map(|(lo, hi)| Gradient::fit(k, stops.clone(), lo, hi)));
                }
            });
            Ok(Value::Null)
        }
        "bone.add" => {
            let p1 = pt(req(a, "x1")?, req(a, "y1")?);
            let p2 = pt(req(a, "x2")?, req(a, "y2")?);
            let h1 = ed.hit_test(p1).ok_or("no object at (x1, y1)")?;
            let h2 = ed.hit_test(p2).ok_or("no object at (x2, y2)")?;
            if h1.layer != h2.layer {
                return Err("both objects must be on the same layer".into());
            }
            ed.add_bone(h1.layer, h1.element, h2.element, p1, p2)?;
            Ok(Value::Null)
        }
        "bone.list" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            Ok(Value::Array(ed.bones(li).iter().map(|b| json!({ "parent": b.parent, "child": b.child })).collect()))
        }
        "bone.clear" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            ed.clear_bones(li);
            Ok(Value::Null)
        }
        "bone.drag" => {
            let p = pt(req(a, "x")?, req(a, "y")?);
            let to = pt(req(a, "to_x")?, req(a, "to_y")?);
            let h = ed.hit_test(p).ok_or("no object at (x, y)")?;
            if ed.bones(h.layer).is_empty() {
                return Err("that object's layer has no bones".into());
            }
            let el = ed.element_at(h).ok_or("no object")?;
            let grab = el.matrix.invert().ok_or("object has no size")?.apply(p);
            ed.begin_edit("Pose Bones");
            ed.ik_drag(h.layer, h.element, grab, to);
            Ok(Value::Null)
        }
        "camera.add" => Ok(json!(ed.add_camera() + 1)),
        "camera.remove" => {
            ed.remove_camera();
            Ok(Value::Null)
        }
        "camera.set" => {
            if ed.camera_layer().is_none() {
                ed.add_camera();
            }
            if let Some(f) = frame_arg(a, "frame")? {
                ed.set_frame(f);
            }
            let cur = ed.camera_at(ed.frame).unwrap_or_else(|| default_camera(&ed.doc).matrix);
            let (c, rot, zoom) = Editor::camera_params(&cur);
            let m = Editor::camera_from(pt(num(a, "x").unwrap_or(c.x), num(a, "y").unwrap_or(c.y)), num(a, "rotation").unwrap_or(rot), num(a, "zoom").unwrap_or(zoom));
            ed.begin_edit("Camera");
            ed.set_camera(m);
            Ok(Value::Null)
        }
        "selection.arrange" => {
            need_selection(ed)?;
            ed.arrange(boolean(a, "front").unwrap_or(true));
            Ok(Value::Null)
        }
        "library.list" => Ok(Value::Array(
            ed.doc
                .library
                .iter()
                .map(|s| json!({ "id": s.id, "name": s.name, "kind": format!("{:?}", s.kind), "frames": s.timeline.frame_count(), "layers": s.timeline.layers.len() }))
                .collect(),
        )),
        "library.place" => {
            let id = symbol_arg(ed, a)?;
            if ed.layer_locked() {
                return Err("current layer is locked or hidden".into());
            }
            ed.place_symbol(id, pt(req(a, "x")?, req(a, "y")?));
            Ok(Value::Null)
        }
        "library.rename" => {
            let id = symbol_arg(ed, a)?;
            ed.rename_symbol(id, string(a, "name").ok_or("missing 'name'")?);
            Ok(Value::Null)
        }
        "library.delete" => {
            let id = symbol_arg(ed, a)?;
            ed.delete_symbol(id);
            Ok(Value::Null)
        }
        "symbol.edit" => {
            let id = symbol_arg(ed, a)?;
            ed.enter_symbol(id);
            Ok(Value::Null)
        }
        "symbol.exit" => {
            ed.exit_to_scene();
            Ok(Value::Null)
        }
        "edit.undo" => Ok(json!(ed.undo())),
        "edit.redo" => Ok(json!(ed.redo())),
        other => Err(format!("unknown command '{other}'")),
    }
}

/// Reference text for help panes and generated helper modules.
pub fn reference() -> String {
    let mut s = String::new();
    for c in COMMANDS {
        s += &format!("{:<32} {:<60} {}\n", c.id, c.args, c.summary);
    }
    s
}
