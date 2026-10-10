//! Document model mirroring Animate's DOM: Document → Timeline → Layer →
//! Keyframe (DOMFrame) → Element (shape or symbol instance), plus a Library
//! of symbols that each own a timeline.

use crate::deform::{project_path, Perspective, Transform3D, Warp};
use crate::geom::{morph_paths, pt, Matrix, Path, Pt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim_start_matches('#');
        let v = u32::from_str_radix(s, 16).ok()?;
        match s.len() {
            6 => Some(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)),
            8 => Some(Color { r: (v >> 24) as u8, g: (v >> 16) as u8, b: (v >> 8) as u8, a: v as u8 }),
            _ => None,
        }
    }
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color { r: l(self.r, o.r), g: l(self.g, o.g), b: l(self.b, o.b), a: l(self.a, o.a) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

/// A vector shape. Demo uses Animate's *Object Drawing* model (each shape is its
/// own object). *Merge Drawing* (shapes that cut/merge, Flash's edge + fill-style
/// planar map) is the big engine milestone — see CLAUDE.md.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Shape {
    pub path: Path,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    /// Gradient fill; overrides `fill` (which stays as the fallback colour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// Variable stroke width (Width tool / Fluid Brush): (t along path 0..1, width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_profile: Option<Vec<(f32, f32)>>,
    /// Asset Warp pins (non-destructive: `path` is the warped result).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<Warp>,
}

impl Shape {
    pub fn new(path: Path, fill: Option<Color>, stroke: Option<Stroke>) -> Shape {
        Shape { path, fill, stroke, ..Default::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientKind {
    Linear,
    Radial,
}

/// Gradient fill. `matrix` maps gradient space (linear: x from -1 to 1;
/// radial: unit circle) into the shape's local coordinates, like Flash's
/// gradient matrix. The Gradient Transform tool edits it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<(f32, Color)>,
    pub matrix: Matrix,
}

impl Gradient {
    /// A gradient fitted to a box (Animate's default placement).
    pub fn fit(kind: GradientKind, stops: Vec<(f32, Color)>, lo: Pt, hi: Pt) -> Gradient {
        let c = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        let (hw, hh) = ((hi.x - lo.x) / 2.0, (hi.y - lo.y) / 2.0);
        let s = match kind {
            GradientKind::Linear => Matrix::scale(hw.max(1.0), hw.max(1.0)),
            GradientKind::Radial => {
                let r = (hw * hw + hh * hh).sqrt().max(1.0);
                Matrix::scale(r, r)
            }
        };
        Gradient { kind, stops, matrix: Matrix::translate(c.x, c.y).concat(&s) }
    }
    pub fn lerp(&self, o: &Gradient, t: f32) -> Gradient {
        if self.kind != o.kind || self.stops.len() != o.stops.len() {
            return if t < 0.5 { self.clone() } else { o.clone() };
        }
        Gradient {
            kind: self.kind,
            stops: self.stops.iter().zip(&o.stops).map(|(a, b)| (a.0 + (b.0 - a.0) * t, a.1.lerp(b.1, t))).collect(),
            matrix: self.matrix.interpolate(&o.matrix, t, 0),
        }
    }
}

pub type SymbolId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// Timeline is synced to the parent timeline (Animate "Graphic").
    Graphic,
    /// Independent timeline (Animate "Movie Clip").
    MovieClip,
    Button,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    Loop,
    PlayOnce,
    SingleFrame,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub symbol: SymbolId,
    pub loop_mode: LoopMode,
    pub first_frame: u32,
}

/// Static text (Animate's Text tool, "Static Text" mode).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextEl {
    pub text: String,
    pub size: f32,
    pub color: Color,
    #[serde(default)]
    pub font: crate::text::FontFace,
    #[serde(default)]
    pub align: crate::text::Align,
    #[serde(default)]
    pub letter_spacing: f32,
}

impl TextEl {
    pub fn outline(&self) -> Path {
        crate::text::text_path(&self.text, self.size, self.font, self.align, self.letter_spacing)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ElementKind {
    Shape(Shape),
    Instance(Instance),
    Text(TextEl),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub name: String,
    pub matrix: Matrix,
    /// Color effect: alpha multiplier 0..1
    pub alpha: f32,
    /// Color effect: tint (color, amount 0..1)
    pub tint: Option<(Color, f32)>,
    pub kind: ElementKind,
    /// 3D Rotation / 3D Translation (perspective-projected at render time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub three_d: Option<Transform3D>,
}

impl Element {
    pub fn shape(name: &str, shape: Shape) -> Self {
        Element { name: name.into(), matrix: Matrix::IDENTITY, alpha: 1.0, tint: None, kind: ElementKind::Shape(shape), three_d: None }
    }
    pub fn text(text: &str, size: f32, color: Color, at: Pt) -> Self {
        Element {
            name: "Text".into(),
            matrix: Matrix::translate(at.x, at.y),
            alpha: 1.0,
            tint: None,
            kind: ElementKind::Text(TextEl { text: text.into(), size, color, font: Default::default(), align: Default::default(), letter_spacing: 0.0 }),
            three_d: None,
        }
    }
    pub fn instance(name: &str, symbol: SymbolId, matrix: Matrix) -> Self {
        Element {
            name: name.into(),
            matrix,
            alpha: 1.0,
            tint: None,
            kind: ElementKind::Instance(Instance { symbol, loop_mode: LoopMode::Loop, first_frame: 0 }),
            three_d: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tween {
    None,
    /// Matrix + color-effect interpolation (Animate "Classic Tween").
    Classic { ease: f32, rotate_turns: i32 },
    /// Path + color morph (Animate "Shape Tween").
    Shape { ease: f32 },
}

impl Tween {
    pub fn ease(&self) -> f32 {
        match *self {
            Tween::None => 0.0,
            Tween::Classic { ease, .. } | Tween::Shape { ease } => ease,
        }
    }
}

/// Animate's simple ease: -100 = ease in, +100 = ease out (quadratic).
pub fn apply_ease(t: f32, ease: f32) -> f32 {
    let e = (ease / 100.0).clamp(-1.0, 1.0);
    let curved = if e >= 0.0 { 1.0 - (1.0 - t) * (1.0 - t) } else { t * t };
    t + (curved - t) * e.abs()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub elements: Vec<Element>,
    pub tween: Tween,
    pub label: Option<String>,
    /// Frame script (JavaScript, CreateJS-style API) run when playback enters this frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

impl Keyframe {
    pub fn blank() -> Self {
        Keyframe { elements: vec![], tween: Tween::None, label: None, script: None }
    }
    pub fn with(elements: Vec<Element>) -> Self {
        Keyframe { elements, tween: Tween::None, label: None, script: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Normal,
    Guide,
    Folder,
    /// Camera layer: its keyframes hold the camera (pan/zoom/rotate), tweenable.
    Camera,
}

/// An IK bone linking two objects on the same layer (element indices are
/// the same in every keyframe of the layer). Points are in local coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    pub parent: usize,
    pub child: usize,
    /// Where the bone starts, in the parent's local space.
    pub head: Pt,
    /// The joint, in the parent's local space…
    pub tail_parent: Pt,
    /// …and the same joint in the child's local space.
    pub tail_child: Pt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub color: Color,
    pub visible: bool,
    pub locked: bool,
    pub outline: bool,
    pub kind: LayerKind,
    /// Keyframes by start frame (0-based). A keyframe spans until the next one.
    pub keyframes: BTreeMap<u32, Keyframe>,
    /// Number of frames the layer occupies (exclusive end).
    pub end: u32,
    /// IK armature (Bone tool).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bones: Vec<Bone>,
}

const LAYER_COLORS: [Color; 6] = [
    Color::rgb(0x4F, 0xFF, 0x4F),
    Color::rgb(0xFF, 0x4F, 0xFF),
    Color::rgb(0x4F, 0xFF, 0xFF),
    Color::rgb(0xFF, 0x80, 0x00),
    Color::rgb(0x99, 0x66, 0xFF),
    Color::rgb(0xFF, 0xFF, 0x4F),
];

impl Layer {
    pub fn new(name: &str, index: usize) -> Self {
        let mut keyframes = BTreeMap::new();
        keyframes.insert(0, Keyframe::blank());
        Layer {
            name: name.into(),
            color: LAYER_COLORS[index % LAYER_COLORS.len()],
            visible: true,
            locked: false,
            outline: false,
            kind: LayerKind::Normal,
            keyframes,
            end: 1,
            bones: vec![],
        }
    }

    /// The keyframe governing `frame`: (start, keyframe, next keyframe start if any).
    pub fn span_at(&self, frame: u32) -> Option<(u32, &Keyframe, Option<u32>)> {
        if frame >= self.end {
            return None;
        }
        let (start, kf) = self.keyframes.range(..=frame).next_back()?;
        let next = self.keyframes.range(frame + 1..).next().map(|(s, _)| *s);
        Some((*start, kf, next))
    }

    /// Exclusive end of the span starting at `start`.
    pub fn span_end(&self, start: u32) -> u32 {
        self.keyframes.range(start + 1..).next().map(|(s, _)| *s).unwrap_or(self.end)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub name: String,
    /// Index 0 is the top layer (as listed in the Timeline panel).
    pub layers: Vec<Layer>,
}

impl Timeline {
    pub fn new(name: &str) -> Self {
        Timeline { name: name.into(), layers: vec![Layer::new("Layer 1", 0)] }
    }
    pub fn frame_count(&self) -> u32 {
        self.layers.iter().map(|l| l.end).max().unwrap_or(1).max(1)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub timeline: Timeline,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub width: f32,
    pub height: f32,
    pub fps: f32,
    pub background: Color,
    pub scene: Timeline,
    pub library: Vec<Symbol>,
    pub next_symbol_id: SymbolId,
    /// 3D: perspective angle (degrees) and vanishing point (default: stage centre).
    #[serde(default = "default_perspective")]
    pub perspective_angle: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vanishing_point: Option<Pt>,
}

fn default_perspective() -> f32 {
    55.0
}

impl Default for Document {
    fn default() -> Self {
        Document {
            width: 550.0,
            height: 400.0,
            fps: 24.0,
            background: Color::rgb(255, 255, 255),
            scene: Timeline::new("Scene 1"),
            library: vec![],
            next_symbol_id: 1,
            perspective_angle: 55.0,
            vanishing_point: None,
        }
    }
}

impl Document {
    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.library.iter().find(|s| s.id == id)
    }
    pub fn symbol_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.library.iter_mut().find(|s| s.id == id)
    }
    pub fn add_symbol(&mut self, name: &str, kind: SymbolKind, timeline: Timeline) -> SymbolId {
        let id = self.next_symbol_id;
        self.next_symbol_id += 1;
        self.library.push(Symbol { id, name: name.into(), kind, timeline });
        id
    }
    pub fn perspective(&self) -> Perspective {
        Perspective { angle_deg: self.perspective_angle, vanishing: self.vanishing_point.unwrap_or(pt(self.width / 2.0, self.height / 2.0)), stage_width: self.width }
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("document serialises")
    }
    pub fn from_json(s: &str) -> Result<Document, serde_json::Error> {
        serde_json::from_str(s)
    }
}

// ---------------------------------------------------------------- evaluation

/// One flattened, ready-to-draw shape.
#[derive(Clone, Debug)]
pub struct DrawItem {
    pub path: Path,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    pub matrix: Matrix,
    pub alpha: f32,
    pub tint: Option<(Color, f32)>,
    pub outline: Option<Color>,
    /// (layer index, element index) in the timeline being evaluated — top level only.
    pub src: Option<(usize, usize)>,
    /// Gradient fill in the item's local space.
    pub gradient: Option<Gradient>,
}

impl DrawItem {
    pub fn hit(&self, p: Pt) -> bool {
        let Some(inv) = self.matrix.invert() else { return false };
        let lp = inv.apply(p);
        let scale = self.matrix.decompose().scale_x.abs().max(0.01);
        let tol = self.stroke.map(|s| s.width / 2.0).unwrap_or(0.0) + 3.0 / scale;
        self.path.hit(lp, self.fill.is_some(), tol)
    }
}

/// Elements of a layer at `frame`, with tweens resolved.
/// Returns (elements, frames since keyframe start).
pub fn layer_elements_at(layer: &Layer, frame: u32) -> Option<(Vec<Element>, u32)> {
    let (start, kf, next) = layer.span_at(frame)?;
    let local = frame - start;
    let tweened = match (kf.tween, next) {
        (Tween::None, _) | (_, None) => None,
        (tween, Some(next_start)) if frame > start => {
            let to = &layer.keyframes[&next_start];
            let t = apply_ease(local as f32 / (next_start - start) as f32, tween.ease());
            Some(interpolate_elements(&kf.elements, &to.elements, t, tween))
        }
        _ => None,
    };
    let mut els = tweened.unwrap_or_else(|| kf.elements.clone());
    if !layer.bones.is_empty() {
        apply_bones(&layer.bones, &mut els);
    }
    Some((els, local))
}

/// Forward kinematics: move each bone's child so its joint sits on the
/// parent's joint (parents first). Rotations come from the keyframes/tweens.
pub fn apply_bones(bones: &[Bone], els: &mut [Element]) {
    let depth = |b: &Bone| -> usize {
        let mut d = 0;
        let mut p = b.parent;
        while let Some(pb) = bones.iter().find(|x| x.child == p) {
            d += 1;
            p = pb.parent;
            if d > 64 {
                break;
            }
        }
        d
    };
    let mut order: Vec<&Bone> = bones.iter().collect();
    order.sort_by_key(|b| depth(b));
    for b in order {
        if b.parent >= els.len() || b.child >= els.len() {
            continue;
        }
        let target = els[b.parent].matrix.apply(b.tail_parent);
        let cur = els[b.child].matrix.apply(b.tail_child);
        els[b.child].matrix.tx += target.x - cur.x;
        els[b.child].matrix.ty += target.y - cur.y;
    }
}

fn interpolate_elements(from: &[Element], to: &[Element], t: f32, tween: Tween) -> Vec<Element> {
    from.iter()
        .enumerate()
        .map(|(i, a)| {
            let Some(b) = to.get(i) else { return a.clone() };
            let turns = if let Tween::Classic { rotate_turns, .. } = tween { rotate_turns } else { 0 };
            let mut out = a.clone();
            out.matrix = a.matrix.interpolate(&b.matrix, t, turns);
            out.alpha = a.alpha + (b.alpha - a.alpha) * t;
            out.tint = match (a.tint, b.tint) {
                (Some((ca, ta)), Some((cb, tb))) => Some((ca.lerp(cb, t), ta + (tb - ta) * t)),
                (Some((c, ta)), None) => Some((c, ta * (1.0 - t))),
                (None, Some((c, tb))) => Some((c, tb * t)),
                (None, None) => None,
            };
            out.three_d = Transform3D::lerp(a.three_d, b.three_d, t);
            if let (Tween::Shape { .. }, ElementKind::Shape(sa), ElementKind::Shape(sb)) = (tween, &a.kind, &b.kind) {
                out.kind = ElementKind::Shape(Shape {
                    gradient: match (&sa.gradient, &sb.gradient) {
                        (Some(x), Some(y)) => Some(x.lerp(y, t)),
                        (x, y) => if t < 0.5 { x.clone() } else { y.clone() },
                    },
                    width_profile: if t < 0.5 { sa.width_profile.clone() } else { sb.width_profile.clone() },
                    warp: None,
                    path: morph_paths(&sa.path, &sb.path, t),
                    fill: match (sa.fill, sb.fill) {
                        (Some(x), Some(y)) => Some(x.lerp(y, t)),
                        (x, y) => if t < 0.5 { x } else { y },
                    },
                    stroke: match (sa.stroke, sb.stroke) {
                        (Some(x), Some(y)) => Some(Stroke { color: x.color.lerp(y.color, t), width: x.width + (y.width - x.width) * t }),
                        (x, y) => if t < 0.5 { x } else { y },
                    },
                });
            }
            out
        })
        .collect()
}

pub struct EvalCtx<'a> {
    pub doc: &'a Document,
    /// Global frame (used by movie clips, which run on their own clock).
    pub global_frame: u32,
}

impl<'a> EvalCtx<'a> {
    pub fn new(doc: &'a Document, global_frame: u32) -> Self {
        EvalCtx { doc, global_frame }
    }
}

fn shape_items(s: &Shape, m: Matrix, a: f32, tint: Option<(Color, f32)>, outline: Option<Color>, out: &mut Vec<DrawItem>) {
    let item = |path: Path, fill: Option<Color>, stroke: Option<Stroke>, gradient: Option<Gradient>| DrawItem { path, fill, stroke, matrix: m, alpha: a, tint, outline, src: None, gradient };
    match (&s.width_profile, s.stroke) {
        // Variable-width stroke: drawn as a filled outline.
        (Some(profile), Some(st)) if !profile.is_empty() => {
            out.push(item(s.path.clone(), s.fill, None, s.gradient.clone()));
            let scaled: Vec<(f32, f32)> = profile.iter().map(|(t, w)| (*t, w * st.width)).collect();
            out.push(item(crate::deform::variable_stroke(&s.path, &scaled), Some(st.color), None, None));
        }
        _ => out.push(item(s.path.clone(), s.fill, s.stroke, s.gradient.clone())),
    }
}

/// Evaluate a timeline at `frame` into draw items, bottom layer first.
pub fn evaluate(ctx: &EvalCtx, tl: &Timeline, frame: u32, parent: &Matrix, alpha: f32, depth: u32, out: &mut Vec<DrawItem>) {
    if depth > 16 {
        return; // recursive symbol guard
    }
    for (li, layer) in tl.layers.iter().enumerate().rev() {
        if !layer.visible || layer.kind != LayerKind::Normal {
            continue;
        }
        let Some((elements, local)) = layer_elements_at(layer, frame) else { continue };
        let outline = layer.outline.then_some(layer.color);
        for (ei, el) in elements.iter().enumerate() {
            let m = parent.concat(&el.matrix);
            let a = alpha * el.alpha;
            let first = out.len();
            match &el.kind {
                ElementKind::Shape(s) => shape_items(s, m, a, el.tint, outline, out),
                ElementKind::Text(t) => out.push(DrawItem { path: t.outline(), fill: Some(t.color), stroke: None, matrix: m, alpha: a, tint: el.tint, outline, src: None, gradient: None }),
                ElementKind::Instance(inst) => {
                    let Some(sym) = ctx.doc.symbol(inst.symbol) else { continue };
                    let dur = sym.timeline.frame_count();
                    let f = match sym.kind {
                        SymbolKind::MovieClip => ctx.global_frame % dur,
                        SymbolKind::Button => 0,
                        SymbolKind::Graphic => {
                            let raw = inst.first_frame + local;
                            match inst.loop_mode {
                                LoopMode::Loop => raw % dur,
                                LoopMode::PlayOnce => raw.min(dur - 1),
                                LoopMode::SingleFrame => inst.first_frame.min(dur - 1),
                            }
                        }
                    };
                    evaluate(ctx, &sym.timeline, f, &m, a, depth + 1, out);
                    for it in &mut out[first..] {
                        if it.tint.is_none() {
                            it.tint = el.tint;
                        }
                        if outline.is_some() {
                            it.outline = outline;
                        }
                    }
                }
            }
            // 3D: project everything this object produced.
            if let Some(td) = el.three_d.filter(|t| !t.is_identity()) {
                let center = m.apply(td.center);
                let persp = ctx.doc.perspective();
                for it in &mut out[first..] {
                    it.path = project_path(&it.path, &it.matrix, &td, center, &persp);
                    if let Some(g) = &mut it.gradient {
                        g.matrix = it.matrix.concat(&g.matrix);
                    }
                    it.matrix = Matrix::IDENTITY;
                }
            }
            if depth == 0 {
                for it in &mut out[first..] {
                    it.src = Some((li, ei));
                }
            }
        }
    }
}

/// Camera view matrix of a timeline at `frame`, if it has a visible camera layer.
pub fn camera_view(doc: &Document, tl: &Timeline, frame: u32) -> Option<Matrix> {
    let layer = tl.layers.iter().find(|l| l.kind == LayerKind::Camera && l.visible)?;
    let (els, _) = layer_elements_at(layer, frame)?;
    let cam = els.first()?.matrix;
    Some(camera_matrix_to_view(doc, &cam))
}

/// The camera element's matrix is translate(centre)·rotate(r)·scale(zoom);
/// the view maps that framing onto the stage.
pub fn camera_matrix_to_view(doc: &Document, cam: &Matrix) -> Matrix {
    let d = cam.decompose();
    let zoom = d.scale_x.abs().max(0.01);
    Matrix::translate(doc.width / 2.0, doc.height / 2.0)
        .concat(&Matrix::scale(zoom, zoom))
        .concat(&Matrix::rotate(-d.skew_y))
        .concat(&Matrix::translate(-cam.tx, -cam.ty))
}

/// Default camera: centred on the stage, zoom 100%, no rotation.
pub fn default_camera(doc: &Document) -> Element {
    let mut e = Element::shape("Camera", Shape::default());
    e.matrix = Matrix::translate(doc.width / 2.0, doc.height / 2.0);
    e
}

pub fn apply_view(items: &mut [DrawItem], view: &Matrix) {
    for it in items {
        it.matrix = view.concat(&it.matrix);
    }
}

pub fn evaluate_scene(doc: &Document, frame: u32) -> Vec<DrawItem> {
    let ctx = EvalCtx::new(doc, frame);
    let mut out = Vec::new();
    evaluate(&ctx, &doc.scene, frame, &Matrix::IDENTITY, 1.0, 0, &mut out);
    if let Some(v) = camera_view(doc, &doc.scene, frame) {
        apply_view(&mut out, &v);
    }
    out
}
