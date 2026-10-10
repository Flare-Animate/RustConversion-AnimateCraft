//! Editing session: every mutation goes through here so undo/redo, the UI,
//! the CLI and (later) the MCP/scripting surface all share one command set.

use crate::geom::{Matrix, Pt};
use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub layer: usize,
    pub element: usize,
}

pub struct Editor {
    pub doc: Document,
    /// None = main scene; Some(id) = editing inside a symbol.
    pub editing: Option<SymbolId>,
    pub frame: u32,
    pub layer: usize,
    pub selection: Option<Selection>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub dirty_label: String,
}

#[derive(Clone)]
struct Snapshot {
    doc: Document,
    label: String,
}

const UNDO_LIMIT: usize = 200;

impl Editor {
    pub fn new(doc: Document) -> Self {
        Editor { doc, editing: None, frame: 0, layer: 0, selection: None, undo: vec![], redo: vec![], dirty_label: String::new() }
    }

    // ------------------------------------------------------------ plumbing
    fn checkpoint(&mut self, label: &str) {
        self.undo.push(Snapshot { doc: self.doc.clone(), label: label.into() });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty_label = label.into();
    }
    pub fn undo(&mut self) -> Option<String> {
        let s = self.undo.pop()?;
        let cur = std::mem::replace(&mut self.doc, s.doc);
        self.redo.push(Snapshot { doc: cur, label: s.label.clone() });
        self.fix_indices();
        Some(s.label)
    }
    pub fn redo(&mut self) -> Option<String> {
        let s = self.redo.pop()?;
        let cur = std::mem::replace(&mut self.doc, s.doc);
        self.undo.push(Snapshot { doc: cur, label: s.label.clone() });
        self.fix_indices();
        Some(s.label)
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    fn fix_indices(&mut self) {
        if let Some(id) = self.editing {
            if self.doc.symbol(id).is_none() {
                self.editing = None;
            }
        }
        let n = self.timeline().layers.len();
        self.layer = self.layer.min(n.saturating_sub(1));
        self.selection = None;
    }

    pub fn timeline(&self) -> &Timeline {
        match self.editing.and_then(|id| self.doc.symbol(id)) {
            Some(s) => &s.timeline,
            None => &self.doc.scene,
        }
    }
    pub fn timeline_mut(&mut self) -> &mut Timeline {
        match self.editing {
            Some(id) if self.doc.symbol(id).is_some() => &mut self.doc.symbol_mut(id).unwrap().timeline,
            _ => &mut self.doc.scene,
        }
    }
    fn cur_layer_mut(&mut self) -> &mut Layer {
        let i = self.layer;
        &mut self.timeline_mut().layers[i]
    }

    pub fn breadcrumb(&self) -> Vec<String> {
        let mut v = vec![self.doc.scene.name.clone()];
        if let Some(s) = self.editing.and_then(|id| self.doc.symbol(id)) {
            v.push(s.name.clone());
        }
        v
    }

    /// Draw items for the timeline currently being edited (with `src` set).
    pub fn evaluate_current(&self, frame: u32) -> Vec<DrawItem> {
        let ctx = EvalCtx { doc: &self.doc, global_frame: frame };
        let mut out = Vec::new();
        evaluate(&ctx, self.timeline(), frame, &Matrix::IDENTITY, 1.0, 0, &mut out);
        out
    }

    // ------------------------------------------------------------ navigation
    pub fn frame_count(&self) -> u32 {
        self.timeline().frame_count()
    }
    pub fn set_frame(&mut self, f: u32) {
        self.frame = f;
        self.selection = None;
    }
    pub fn enter_symbol(&mut self, id: SymbolId) {
        if self.doc.symbol(id).is_some() {
            self.editing = Some(id);
            self.frame = 0;
            self.layer = 0;
            self.selection = None;
        }
    }
    pub fn exit_to_scene(&mut self) {
        self.editing = None;
        self.frame = 0;
        self.layer = 0;
        self.selection = None;
    }

    // ------------------------------------------------------------ layers
    pub fn add_layer(&mut self) {
        self.checkpoint("New Layer");
        let tl = self.timeline_mut();
        let n = tl.layers.len();
        let end = tl.frame_count();
        let mut l = Layer::new(&format!("Layer {}", n + 1), n);
        l.end = end;
        let at = self.layer;
        self.timeline_mut().layers.insert(at, l);
        self.selection = None;
    }
    pub fn delete_layer(&mut self) {
        if self.timeline().layers.len() <= 1 {
            return;
        }
        self.checkpoint("Delete Layer");
        let i = self.layer;
        self.timeline_mut().layers.remove(i);
        self.fix_indices();
    }
    pub fn rename_layer(&mut self, i: usize, name: &str) {
        self.checkpoint("Rename Layer");
        self.timeline_mut().layers[i].name = name.into();
    }
    pub fn toggle_layer_flag(&mut self, i: usize, flag: LayerFlag) {
        let l = &mut self.timeline_mut().layers[i];
        match flag {
            LayerFlag::Visible => l.visible = !l.visible,
            LayerFlag::Locked => l.locked = !l.locked,
            LayerFlag::Outline => l.outline = !l.outline,
        }
    }

    // ------------------------------------------------------------ frames (F5/F6/F7)
    /// F5 – Insert Frame: extend the span under the playhead by one frame.
    pub fn insert_frame(&mut self) {
        self.checkpoint("Insert Frame");
        let f = self.frame;
        let layer = self.cur_layer_mut();
        if f >= layer.end {
            layer.end = f + 1;
            return;
        }
        let moved: Vec<(u32, Keyframe)> = layer.keyframes.range(f + 1..).map(|(k, v)| (*k, v.clone())).collect();
        for (k, _) in &moved {
            layer.keyframes.remove(k);
        }
        for (k, v) in moved {
            layer.keyframes.insert(k + 1, v);
        }
        layer.end += 1;
    }

    /// Shift+F5 – Remove Frame.
    pub fn remove_frame(&mut self) {
        let f = self.frame;
        let layer = &self.timeline().layers[self.layer];
        if f >= layer.end || layer.end <= 1 {
            return;
        }
        self.checkpoint("Remove Frames");
        let layer = self.cur_layer_mut();
        if layer.keyframes.contains_key(&f) && f != 0 {
            let next = layer.span_end(f);
            if next - f == 1 {
                layer.keyframes.remove(&f);
            }
        }
        let moved: Vec<(u32, Keyframe)> = layer.keyframes.range(f + 1..).map(|(k, v)| (*k, v.clone())).collect();
        for (k, _) in &moved {
            layer.keyframes.remove(k);
        }
        for (k, v) in moved {
            layer.keyframes.entry(k - 1).or_insert(v);
        }
        layer.end -= 1;
    }

    /// F6 – Insert Keyframe: copies what is visible (including mid-tween state).
    pub fn insert_keyframe(&mut self) {
        let f = self.frame;
        let layer = &self.timeline().layers[self.layer];
        if layer.keyframes.contains_key(&f) {
            return;
        }
        let (elements, prev_tween) = content_for(layer, f);
        self.checkpoint("Insert Keyframe");
        let layer = self.cur_layer_mut();
        let mut kf = Keyframe::with(elements);
        kf.tween = prev_tween; // splitting a tween keeps both halves tweened
        layer.keyframes.insert(f, kf);
        layer.end = layer.end.max(f + 1);
    }

    /// F7 – Insert Blank Keyframe.
    pub fn insert_blank_keyframe(&mut self) {
        let f = self.frame;
        self.checkpoint("Insert Blank Keyframe");
        let layer = self.cur_layer_mut();
        layer.keyframes.insert(f, Keyframe::blank());
        layer.end = layer.end.max(f + 1);
    }

    /// Shift+F6 – Clear Keyframe.
    pub fn clear_keyframe(&mut self) {
        let f = self.frame;
        if f == 0 || !self.timeline().layers[self.layer].keyframes.contains_key(&f) {
            return;
        }
        self.checkpoint("Clear Keyframe");
        self.cur_layer_mut().keyframes.remove(&f);
    }

    pub fn set_tween(&mut self, tween: Tween) {
        let f = self.frame;
        let layer = &self.timeline().layers[self.layer];
        let Some((start, _, _)) = layer.span_at(f) else { return };
        let label = match tween {
            Tween::None => "Remove Tween",
            Tween::Classic { .. } => "Create Classic Tween",
            Tween::Shape { .. } => "Create Shape Tween",
        };
        self.checkpoint(label);
        let layer = self.cur_layer_mut();
        // Animate needs an end keyframe; create one at the span end if missing.
        let end = layer.span_end(start);
        if !matches!(tween, Tween::None) && !layer.keyframes.contains_key(&end) && end > start + 1 {
            let last = end - 1;
            let content = layer.keyframes[&start].elements.clone();
            layer.keyframes.insert(last, Keyframe::with(content));
        }
        layer.keyframes.get_mut(&start).unwrap().tween = tween;
    }

    pub fn current_keyframe(&self) -> Option<(u32, &Keyframe)> {
        let layer = self.timeline().layers.get(self.layer)?;
        layer.span_at(self.frame).map(|(s, k, _)| (s, k))
    }

    pub fn set_frame_label(&mut self, label: &str) {
        let Some((start, _)) = self.current_keyframe() else { return };
        self.checkpoint("Frame Label");
        let l = self.cur_layer_mut();
        l.keyframes.get_mut(&start).unwrap().label = if label.is_empty() { None } else { Some(label.into()) };
    }

    // ------------------------------------------------------------ elements
    /// Make sure the playhead sits on a keyframe of the current layer
    /// (auto-keyframe, like editing on a motion-tween span) and return its start.
    fn ensure_keyframe(&mut self) -> u32 {
        let f = self.frame;
        let layer = &self.timeline().layers[self.layer];
        if !layer.keyframes.contains_key(&f) {
            let (content, prev_tween) = content_for(layer, f);
            let l = self.cur_layer_mut();
            let mut kf = Keyframe::with(content);
            kf.tween = prev_tween;
            l.keyframes.insert(f, kf);
            l.end = l.end.max(f + 1);
        }
        f
    }

    pub fn layer_locked(&self) -> bool {
        self.timeline().layers.get(self.layer).map(|l| l.locked || !l.visible).unwrap_or(true)
    }

    pub fn add_element(&mut self, el: Element) -> Option<Selection> {
        if self.layer_locked() {
            return None;
        }
        self.checkpoint(&format!("Add {}", el.name));
        let start = self.ensure_keyframe();
        let li = self.layer;
        let l = self.cur_layer_mut();
        let kf = l.keyframes.get_mut(&start).unwrap();
        kf.elements.push(el);
        let sel = Selection { layer: li, element: kf.elements.len() - 1 };
        self.selection = Some(sel);
        Some(sel)
    }

    /// Topmost element under `p` on unlocked, visible layers.
    pub fn hit_test(&self, p: Pt) -> Option<Selection> {
        let items = self.evaluate_current(self.frame);
        let tl = self.timeline();
        items.iter().rev().find_map(|it| {
            let (li, ei) = it.src?;
            if tl.layers[li].locked {
                return None;
            }
            it.hit(p).then_some(Selection { layer: li, element: ei })
        })
    }

    pub fn selected_element(&self) -> Option<Element> {
        let s = self.selection?;
        let layer = self.timeline().layers.get(s.layer)?;
        let (els, _) = layer_elements_at(layer, self.frame)?;
        els.get(s.element).cloned()
    }

    pub fn selection_bounds(&self) -> Option<(Pt, Pt)> {
        let s = self.selection?;
        let items = self.evaluate_current(self.frame);
        let mut b: Option<(Pt, Pt)> = None;
        for it in items.iter().filter(|it| it.src == Some((s.layer, s.element))) {
            if let Some((lo, hi)) = it.path.transformed(&it.matrix).bounds() {
                b = Some(match b {
                    None => (lo, hi),
                    Some((a, c)) => (crate::geom::pt(a.x.min(lo.x), a.y.min(lo.y)), crate::geom::pt(c.x.max(hi.x), c.y.max(hi.y))),
                });
            }
        }
        b
    }

    /// Apply `f` to the selected element (auto-keyframing). `label` names the undo step.
    pub fn modify_selected(&mut self, label: &str, f: impl FnOnce(&mut Element)) {
        let Some(s) = self.selection else { return };
        self.layer = s.layer;
        if self.layer_locked() {
            return;
        }
        self.checkpoint(label);
        let start = self.ensure_keyframe();
        let l = self.cur_layer_mut();
        if let Some(el) = l.keyframes.get_mut(&start).unwrap().elements.get_mut(s.element) {
            f(el);
        }
    }

    /// Same as `modify_selected` but merges into the previous undo step
    /// (used while dragging so one drag = one undo).
    pub fn modify_selected_continuous(&mut self, f: impl FnOnce(&mut Element)) {
        let Some(s) = self.selection else { return };
        let start = self.ensure_keyframe();
        let li = s.layer;
        let l = &mut self.timeline_mut().layers[li];
        if let Some(el) = l.keyframes.get_mut(&start).and_then(|k| k.elements.get_mut(s.element)) {
            f(el);
        }
    }
    pub fn begin_drag(&mut self) {
        self.checkpoint("Move");
    }

    pub fn delete_selected(&mut self) {
        let Some(s) = self.selection else { return };
        self.layer = s.layer;
        self.checkpoint("Delete");
        let start = self.ensure_keyframe();
        let l = self.cur_layer_mut();
        let els = &mut l.keyframes.get_mut(&start).unwrap().elements;
        if s.element < els.len() {
            els.remove(s.element);
        }
        self.selection = None;
    }

    pub fn arrange(&mut self, to_front: bool) {
        let Some(s) = self.selection else { return };
        self.layer = s.layer;
        self.checkpoint(if to_front { "Bring to Front" } else { "Send to Back" });
        let start = self.ensure_keyframe();
        let l = self.cur_layer_mut();
        let els = &mut l.keyframes.get_mut(&start).unwrap().elements;
        let e = els.remove(s.element);
        let idx = if to_front { els.len() } else { 0 };
        els.insert(idx, e);
        self.selection = Some(Selection { layer: s.layer, element: idx });
    }

    /// F8 – Convert to Symbol. Registration point = centre of the element bounds.
    pub fn convert_to_symbol(&mut self, name: &str, kind: SymbolKind) -> Option<SymbolId> {
        let s = self.selection?;
        let el = self.selected_element()?;
        let (lo, hi) = self.selection_bounds()?;
        let reg = crate::geom::pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        self.layer = s.layer;
        self.checkpoint("Convert to Symbol");
        let mut inner = el.clone();
        inner.matrix = Matrix::translate(-reg.x, -reg.y).concat(&el.matrix);
        inner.alpha = 1.0;
        inner.tint = None;
        let mut tl = Timeline::new(name);
        tl.layers[0].keyframes.insert(0, Keyframe::with(vec![inner]));
        let id = self.doc.add_symbol(name, kind, tl);
        let start = self.ensure_keyframe();
        let l = self.cur_layer_mut();
        let slot = &mut l.keyframes.get_mut(&start).unwrap().elements[s.element];
        let mut inst = Element::instance(name, id, Matrix::translate(reg.x, reg.y));
        inst.alpha = el.alpha;
        inst.tint = el.tint;
        *slot = inst;
        Some(id)
    }

    pub fn place_symbol(&mut self, id: SymbolId, at: Pt) {
        let Some(sym) = self.doc.symbol(id) else { return };
        if self.editing == Some(id) {
            return; // can't place a symbol inside itself
        }
        let el = Element::instance(&sym.name.clone(), id, Matrix::translate(at.x, at.y));
        self.add_element(el);
    }

    pub fn new_symbol(&mut self, name: &str, kind: SymbolKind) -> SymbolId {
        self.checkpoint("New Symbol");
        let id = self.doc.add_symbol(name, kind, Timeline::new(name));
        self.enter_symbol(id);
        id
    }

    pub fn rename_symbol(&mut self, id: SymbolId, name: &str) {
        self.checkpoint("Rename Symbol");
        if let Some(s) = self.doc.symbol_mut(id) {
            s.name = name.into();
            s.timeline.name = name.into();
        }
    }

    pub fn set_doc_props(&mut self, w: f32, h: f32, fps: f32, bg: Color) {
        self.checkpoint("Document Settings");
        self.doc.width = w;
        self.doc.height = h;
        self.doc.fps = fps;
        self.doc.background = bg;
    }

    /// Paint bucket on an element: sets fill (shapes) or tint (instances).
    pub fn paint_bucket(&mut self, sel: Selection, color: Color) {
        self.selection = Some(sel);
        self.modify_selected("Paint Bucket", |el| match &mut el.kind {
            ElementKind::Shape(s) => s.fill = Some(color),
            ElementKind::Instance(_) => el.tint = Some((color, 0.6)),
        });
    }
}

/// What a new keyframe at `f` should contain: the visible (possibly tweened)
/// content, or — past the layer's end — the last keyframe's content.
fn content_for(layer: &Layer, f: u32) -> (Vec<Element>, Tween) {
    if let Some((els, _)) = layer_elements_at(layer, f) {
        let tween = layer.span_at(f).map(|(_, k, _)| k.tween).unwrap_or(Tween::None);
        return (els, tween);
    }
    let last = layer.keyframes.values().next_back().map(|k| k.elements.clone()).unwrap_or_default();
    (last, Tween::None)
}

#[derive(Clone, Copy, Debug)]
pub enum LayerFlag {
    Visible,
    Locked,
    Outline,
}
