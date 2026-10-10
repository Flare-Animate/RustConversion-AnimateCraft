//! Document model mirroring Animate's DOM: Document → Timeline → Layer →
//! Keyframe (DOMFrame) → Element (shape or symbol instance), plus a Library
//! of symbols that each own a timeline.

use crate::geom::{morph_paths, Matrix, Path, Pt};
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub path: Path,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ElementKind {
    Shape(Shape),
    Instance(Instance),
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
}

impl Element {
    pub fn shape(name: &str, shape: Shape) -> Self {
        Element { name: name.into(), matrix: Matrix::IDENTITY, alpha: 1.0, tint: None, kind: ElementKind::Shape(shape) }
    }
    pub fn instance(name: &str, symbol: SymbolId, matrix: Matrix) -> Self {
        Element {
            name: name.into(),
            matrix,
            alpha: 1.0,
            tint: None,
            kind: ElementKind::Instance(Instance { symbol, loop_mode: LoopMode::Loop, first_frame: 0 }),
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
}

impl Keyframe {
    pub fn blank() -> Self {
        Keyframe { elements: vec![], tween: Tween::None, label: None }
    }
    pub fn with(elements: Vec<Element>) -> Self {
        Keyframe { elements, tween: Tween::None, label: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Normal,
    Guide,
    Folder,
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
    Some((tweened.unwrap_or_else(|| kf.elements.clone()), local))
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
            if let (Tween::Shape { .. }, ElementKind::Shape(sa), ElementKind::Shape(sb)) = (tween, &a.kind, &b.kind) {
                out.kind = ElementKind::Shape(Shape {
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
                ElementKind::Shape(s) => out.push(DrawItem {
                    path: s.path.clone(),
                    fill: s.fill,
                    stroke: s.stroke,
                    matrix: m,
                    alpha: a,
                    tint: el.tint,
                    outline,
                    src: None,
                }),
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
            if depth == 0 {
                for it in &mut out[first..] {
                    it.src = Some((li, ei));
                }
            }
        }
    }
}

pub fn evaluate_scene(doc: &Document, frame: u32) -> Vec<DrawItem> {
    let ctx = EvalCtx { doc, global_frame: frame };
    let mut out = Vec::new();
    evaluate(&ctx, &doc.scene, frame, &Matrix::IDENTITY, 1.0, 0, &mut out);
    out
}
