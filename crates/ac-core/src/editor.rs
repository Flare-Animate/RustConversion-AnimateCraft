//! Editing session: every mutation goes through here so undo/redo, the UI,
//! the CLI and (later) the MCP/scripting surface all share one command set.

use crate::frames::{self, FrameClip};
use crate::geom::{pt, Matrix, Pt};
use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Selection {
    pub layer: usize,
    pub element: usize,
}

/// A rectangular block of timeline cells (inclusive on both axes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameSel {
    pub layer0: usize,
    pub layer1: usize,
    pub f0: u32,
    pub f1: u32,
}

impl FrameSel {
    pub fn cell(layer: usize, frame: u32) -> Self {
        FrameSel { layer0: layer, layer1: layer, f0: frame, f1: frame }
    }
    pub fn between(a: (usize, u32), b: (usize, u32)) -> Self {
        FrameSel { layer0: a.0.min(b.0), layer1: a.0.max(b.0), f0: a.1.min(b.1), f1: a.1.max(b.1) }
    }
    pub fn contains(&self, layer: usize, frame: u32) -> bool {
        (self.layer0..=self.layer1).contains(&layer) && (self.f0..=self.f1).contains(&frame)
    }
    pub fn frames(&self) -> u32 {
        self.f1 - self.f0 + 1
    }
    pub fn is_single(&self) -> bool {
        self.layer0 == self.layer1 && self.f0 == self.f1
    }
}

pub struct Editor {
    pub doc: Document,
    /// None = main scene; Some(id) = editing inside a symbol.
    pub editing: Option<SymbolId>,
    pub frame: u32,
    pub layer: usize,
    /// Selected stage elements (sorted, unique).
    pub selection: Vec<Selection>,
    /// Selected timeline cells.
    pub frame_sel: Option<FrameSel>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    element_clip: Vec<(Element, usize)>,
    frame_clip: Vec<FrameClip>,
    /// Show the scene through the camera layer (playback, Camera tool).
    pub camera_view: bool,
}

#[derive(Clone)]
struct Snapshot {
    doc: Document,
    label: String,
}

const UNDO_LIMIT: usize = 200;

impl Editor {
    pub fn new(doc: Document) -> Self {
        Editor {
            doc,
            editing: None,
            frame: 0,
            layer: 0,
            selection: vec![],
            frame_sel: None,
            undo: vec![],
            redo: vec![],
            element_clip: vec![],
            frame_clip: vec![],
            camera_view: false,
        }
    }

    // ------------------------------------------------------------ plumbing
    fn checkpoint(&mut self, label: &str) {
        self.undo.push(Snapshot { doc: self.doc.clone(), label: label.into() });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
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
        self.selection.clear();
        if let Some(fs) = &mut self.frame_sel {
            if fs.layer1 >= n {
                self.frame_sel = None;
            }
        }
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
        if self.camera_view && self.editing.is_none() {
            if let Some(v) = camera_view(&self.doc, &self.doc.scene, frame) {
                apply_view(&mut out, &v);
            }
        }
        out
    }

    // ------------------------------------------------------------ navigation
    pub fn frame_count(&self) -> u32 {
        self.timeline().frame_count()
    }
    /// Move the playhead. Keeps the stage selection only if it stays valid.
    pub fn set_frame(&mut self, f: u32) {
        if f != self.frame {
            self.frame = f;
            self.selection.clear();
        }
    }
    pub fn enter_symbol(&mut self, id: SymbolId) {
        if self.doc.symbol(id).is_some() {
            self.editing = Some(id);
            self.frame = 0;
            self.layer = 0;
            self.selection.clear();
            self.frame_sel = None;
        }
    }
    pub fn exit_to_scene(&mut self) {
        self.editing = None;
        self.frame = 0;
        self.layer = 0;
        self.selection.clear();
        self.frame_sel = None;
    }

    // ------------------------------------------------------------ layers
    pub fn add_layer(&mut self) {
        self.checkpoint("New Layer");
        let end = self.timeline().frame_count();
        let n = self.timeline().layers.len();
        let mut l = Layer::new(&format!("Layer {}", n + 1), n);
        l.end = end;
        let at = self.layer.min(n);
        self.timeline_mut().layers.insert(at, l);
        self.layer = at;
        self.selection.clear();
        self.frame_sel = None;
    }
    pub fn delete_layer(&mut self) {
        if self.timeline().layers.len() <= 1 {
            return;
        }
        self.checkpoint("Delete Layer");
        let i = self.layer;
        self.timeline_mut().layers.remove(i);
        self.frame_sel = None;
        self.fix_indices();
    }
    pub fn move_layer(&mut self, from: usize, to: usize) {
        let n = self.timeline().layers.len();
        if from >= n || to >= n || from == to {
            return;
        }
        self.checkpoint("Move Layer");
        let l = self.timeline_mut().layers.remove(from);
        self.timeline_mut().layers.insert(to, l);
        self.layer = to;
        self.selection.clear();
        self.frame_sel = None;
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
        // Hidden or locked layers can't keep selected objects.
        let tl = self.timeline();
        let keep: Vec<Selection> = self.selection.iter().copied().filter(|s| tl.layers.get(s.layer).map(|l| l.visible && !l.locked).unwrap_or(false)).collect();
        self.selection = keep;
    }

    // ------------------------------------------------------------ frame selection
    /// The selected block, or the cell under the playhead on the current layer.
    pub fn frame_target(&self) -> FrameSel {
        self.frame_sel.unwrap_or(FrameSel::cell(self.layer, self.frame))
    }
    pub fn select_frames(&mut self, sel: FrameSel) {
        let n = self.timeline().layers.len();
        if n == 0 {
            return;
        }
        let sel = FrameSel { layer0: sel.layer0.min(n - 1), layer1: sel.layer1.min(n - 1), ..sel };
        self.frame_sel = Some(sel);
        self.layer = sel.layer0;
        if self.frame != sel.f0 {
            self.frame = sel.f0;
        }
        self.selection.clear();
    }
    /// Double-click a frame: select the whole span it belongs to.
    pub fn select_span(&mut self, layer: usize, frame: u32) {
        let Some(l) = self.timeline().layers.get(layer) else { return };
        let sel = match l.span_at(frame) {
            Some((s, _, _)) => FrameSel { layer0: layer, layer1: layer, f0: s, f1: l.span_end(s) - 1 },
            None => FrameSel::cell(layer, frame),
        };
        self.select_frames(sel);
    }
    /// Click a layer name: select all of that layer's frames.
    pub fn select_layer_frames(&mut self, layer: usize) {
        let Some(l) = self.timeline().layers.get(layer) else { return };
        let end = l.end.max(1);
        self.frame_sel = Some(FrameSel { layer0: layer, layer1: layer, f0: 0, f1: end - 1 });
        self.layer = layer;
        self.selection.clear();
    }

    fn for_each_layer(&mut self, label: &str, mut f: impl FnMut(&mut Layer, &FrameSel)) {
        let sel = self.frame_target();
        self.checkpoint(label);
        let tl = self.timeline_mut();
        for li in sel.layer0..=sel.layer1.min(tl.layers.len().saturating_sub(1)) {
            f(&mut tl.layers[li], &sel);
        }
        self.selection.clear();
    }

    /// F5 – Insert Frame(s): as many frames as are selected.
    pub fn insert_frame(&mut self) {
        self.for_each_layer("Insert Frames", |l, s| frames::insert_frames(l, s.f1, s.frames()));
    }
    /// Shift+F5 – Remove Frames.
    pub fn remove_frame(&mut self) {
        self.for_each_layer("Remove Frames", |l, s| frames::remove_frames(l, s.f0, s.f1));
        if let Some(s) = &mut self.frame_sel {
            s.f1 = s.f0;
        }
    }
    /// F6 – Insert Keyframe (copies what is visible, including mid-tween state).
    pub fn insert_keyframe(&mut self) {
        self.for_each_layer("Insert Keyframe", |l, s| frames::convert_to_keyframes(l, s.f0, s.f1, false));
    }
    /// F7 – Insert Blank Keyframe.
    pub fn insert_blank_keyframe(&mut self) {
        self.for_each_layer("Insert Blank Keyframe", |l, s| frames::convert_to_keyframes(l, s.f0, s.f1, true));
    }
    /// Shift+F6 – Clear Keyframe.
    pub fn clear_keyframe(&mut self) {
        self.for_each_layer("Clear Keyframe", |l, s| frames::clear_keyframes(l, s.f0, s.f1));
    }
    /// Delete/Backspace on selected frames – Clear Frames.
    pub fn clear_frames(&mut self) {
        self.for_each_layer("Clear Frames", |l, s| frames::clear_frames(l, s.f0, s.f1));
    }
    pub fn set_tween(&mut self, tween: Tween) {
        let label = match tween {
            Tween::None => "Remove Tween",
            Tween::Classic { .. } => "Create Classic Tween",
            Tween::Shape { .. } => "Create Shape Tween",
        };
        self.for_each_layer(label, |l, s| frames::set_tween(l, s.f0, s.f1, tween));
    }
    /// Change only the ease/rotation of existing tweens in the target.
    pub fn set_tween_params(&mut self, ease: f32, turns: i32) {
        self.for_each_layer("Tween Settings", |l, s| {
            let starts: Vec<u32> = l.keyframes.keys().copied().filter(|&k| k <= s.f1 && l.span_end(k) > s.f0).collect();
            for k in starts {
                let kf = l.keyframes.get_mut(&k).unwrap();
                kf.tween = match kf.tween {
                    Tween::Classic { .. } => Tween::Classic { ease, rotate_turns: turns },
                    Tween::Shape { .. } => Tween::Shape { ease },
                    Tween::None => Tween::None,
                };
            }
        });
    }
    pub fn copy_frames(&mut self) {
        let s = self.frame_target();
        let tl = self.timeline();
        self.frame_clip = (s.layer0..=s.layer1).filter_map(|i| tl.layers.get(i)).map(|l| frames::copy_frames(l, s.f0, s.f1)).collect();
    }
    pub fn cut_frames(&mut self) {
        self.copy_frames();
        self.clear_frames();
    }
    pub fn can_paste_frames(&self) -> bool {
        !self.frame_clip.is_empty()
    }
    pub fn paste_frames(&mut self) {
        if self.frame_clip.is_empty() {
            return;
        }
        let s = self.frame_target();
        let clips = self.frame_clip.clone();
        self.checkpoint("Paste Frames");
        let tl = self.timeline_mut();
        for (i, clip) in clips.iter().enumerate() {
            if let Some(l) = tl.layers.get_mut(s.layer0 + i) {
                frames::paste_frames(l, s.f0, clip);
            }
        }
        let len = clips[0].len;
        self.frame_sel = Some(FrameSel { layer0: s.layer0, layer1: (s.layer0 + clips.len() - 1).min(self.timeline().layers.len() - 1), f0: s.f0, f1: s.f0 + len - 1 });
        self.selection.clear();
    }
    /// Drag the selected frames left/right by `delta`.
    pub fn move_frames(&mut self, delta: i64) {
        let Some(s) = self.frame_sel else { return };
        if delta == 0 {
            return;
        }
        self.checkpoint("Move Frames");
        let tl = self.timeline_mut();
        for li in s.layer0..=s.layer1 {
            if let Some(l) = tl.layers.get_mut(li) {
                frames::move_frames(l, s.f0, s.f1, delta);
            }
        }
        let d = delta.max(-(s.f0 as i64));
        let f0 = (s.f0 as i64 + d) as u32;
        self.frame_sel = Some(FrameSel { f0, f1: f0 + (s.f1 - s.f0), ..s });
        self.frame = f0;
        self.selection.clear();
    }

    /// The keyframe governing the playhead on the current layer.
    pub fn current_keyframe(&self) -> Option<(u32, &Keyframe)> {
        let layer = self.timeline().layers.get(self.layer)?;
        layer.span_at(self.frame).map(|(s, k, _)| (s, k))
    }

    pub fn set_frame_label(&mut self, label: &str) {
        let Some((start, _)) = self.current_keyframe() else { return };
        self.checkpoint("Frame Label");
        let li = self.layer;
        let l = &mut self.timeline_mut().layers[li];
        l.keyframes.get_mut(&start).unwrap().label = if label.is_empty() { None } else { Some(label.into()) };
    }

    pub fn set_frame_script(&mut self, script: &str) {
        let li = self.layer;
        let f = self.frame;
        if self.timeline().layers.get(li).is_none() {
            return;
        }
        self.checkpoint("Frame Script");
        let l = &mut self.timeline_mut().layers[li];
        // Scripts live on keyframes; make one here if needed (as Animate does).
        if !l.keyframes.contains_key(&f) {
            frames::convert_to_keyframes(l, f, f, false);
        }
        let kf = l.keyframes.get_mut(&f).unwrap();
        kf.script = if script.trim().is_empty() { None } else { Some(script.into()) };
    }

    /// Undo-stack depth; pair with `squash_undo` to make a batch (a script run) one step.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }
    pub fn squash_undo(&mut self, since: usize, label: &str) {
        if self.undo.len() > since + 1 {
            self.undo.truncate(since + 1);
        }
        if let Some(s) = self.undo.get_mut(since) {
            s.label = label.into();
        }
    }

    // ------------------------------------------------------------ elements
    /// Which keyframe an edit at the playhead lands in, creating one if needed:
    /// - past the layer's end: a new keyframe (Animate inserts one for you)
    /// - inside a static span: the span's keyframe (same object on every frame)
    /// - inside a tween: a new keyframe here when `pose` (moving/scaling an object
    ///   mid-tween sets a new pose), otherwise the span's keyframe
    fn target_keyframe(&mut self, layer: usize, pose: bool) -> Option<u32> {
        let f = self.frame;
        let l = self.timeline().layers.get(layer)?;
        let key = match l.span_at(f) {
            None => {
                let l = &mut self.timeline_mut().layers[layer];
                frames::convert_to_keyframes(l, f, f, false);
                return Some(f);
            }
            Some((s, _, _)) if s == f => f,
            Some((s, kf, _)) => {
                if pose && !matches!(kf.tween, Tween::None) {
                    let l = &mut self.timeline_mut().layers[layer];
                    frames::convert_to_keyframes(l, f, f, false);
                    f
                } else {
                    s
                }
            }
        };
        Some(key)
    }

    pub fn layer_editable(&self, layer: usize) -> bool {
        self.timeline().layers.get(layer).map(|l| !l.locked && l.visible && l.kind != LayerKind::Camera).unwrap_or(false)
    }
    pub fn layer_locked(&self) -> bool {
        !self.layer_editable(self.layer)
    }

    pub fn add_element(&mut self, el: Element) -> Option<Selection> {
        if self.layer_locked() {
            return None;
        }
        self.checkpoint(&format!("Add {}", el.name));
        let li = self.layer;
        let start = self.target_keyframe(li, false)?;
        let kf = self.timeline_mut().layers[li].keyframes.get_mut(&start).unwrap();
        kf.elements.push(el);
        let sel = Selection { layer: li, element: kf.elements.len() - 1 };
        self.selection = vec![sel];
        self.frame_sel = None;
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

    /// Topmost editable object under `p` on `layer`, ignoring `skip`.
    pub fn hit_test_on_layer(&self, p: Pt, layer: usize, skip: Option<usize>) -> Option<usize> {
        self.evaluate_current(self.frame).iter().rev().find_map(|it| {
            let (li, ei) = it.src?;
            (li == layer && Some(ei) != skip && it.hit(p)).then_some(ei)
        })
    }

    pub fn is_selected(&self, s: Selection) -> bool {
        self.selection.contains(&s)
    }
    /// Click: replace selection. Shift-click: toggle.
    pub fn select(&mut self, s: Option<Selection>, toggle: bool) {
        match (s, toggle) {
            (Some(s), true) => {
                if let Some(i) = self.selection.iter().position(|x| *x == s) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(s);
                    self.selection.sort();
                }
            }
            (Some(s), false) => self.selection = vec![s],
            (None, false) => self.selection.clear(),
            (None, true) => {}
        }
        if let Some(s) = self.selection.first() {
            self.layer = s.layer;
            self.frame_sel = None;
        }
    }
    pub fn select_all(&mut self) {
        let items = self.evaluate_current(self.frame);
        let mut v: Vec<Selection> = items.iter().filter_map(|it| it.src).filter(|(l, _)| self.layer_editable(*l)).map(|(layer, element)| Selection { layer, element }).collect();
        v.sort();
        v.dedup();
        self.selection = v;
        self.frame_sel = None;
    }
    /// Marquee: select elements whose bounds lie fully inside the rectangle.
    pub fn select_in_rect(&mut self, a: Pt, b: Pt, add: bool) {
        let (lo, hi) = (pt(a.x.min(b.x), a.y.min(b.y)), pt(a.x.max(b.x), a.y.max(b.y)));
        let mut found: Vec<Selection> = Vec::new();
        let items = self.evaluate_current(self.frame);
        let mut bounds: std::collections::BTreeMap<(usize, usize), (Pt, Pt)> = Default::default();
        for it in &items {
            let Some(src) = it.src else { continue };
            if !self.layer_editable(src.0) {
                continue;
            }
            if let Some((l, h)) = it.path.transformed(&it.matrix).bounds() {
                let e = bounds.entry(src).or_insert((l, h));
                *e = (pt(e.0.x.min(l.x), e.0.y.min(l.y)), pt(e.1.x.max(h.x), e.1.y.max(h.y)));
            }
        }
        for ((layer, element), (l, h)) in bounds {
            if l.x >= lo.x && l.y >= lo.y && h.x <= hi.x && h.y <= hi.y {
                found.push(Selection { layer, element });
            }
        }
        if add {
            self.selection.extend(found);
            self.selection.sort();
            self.selection.dedup();
        } else {
            self.selection = found;
        }
        self.frame_sel = None;
    }

    pub fn element_at(&self, s: Selection) -> Option<Element> {
        let layer = self.timeline().layers.get(s.layer)?;
        let (els, _) = layer_elements_at(layer, self.frame)?;
        els.get(s.element).cloned()
    }
    /// The first selected element (drives the Properties panel).
    pub fn selected_element(&self) -> Option<Element> {
        self.element_at(*self.selection.first()?)
    }

    pub fn bounds_of(&self, sels: &[Selection]) -> Option<(Pt, Pt)> {
        let items = self.evaluate_current(self.frame);
        let mut b: Option<(Pt, Pt)> = None;
        for it in items.iter().filter(|it| it.src.map(|(l, e)| sels.contains(&Selection { layer: l, element: e })).unwrap_or(false)) {
            if let Some((lo, hi)) = it.path.transformed(&it.matrix).bounds() {
                b = Some(match b {
                    None => (lo, hi),
                    Some((a, c)) => (pt(a.x.min(lo.x), a.y.min(lo.y)), pt(c.x.max(hi.x), c.y.max(hi.y))),
                });
            }
        }
        b
    }
    pub fn selection_bounds(&self) -> Option<(Pt, Pt)> {
        self.bounds_of(&self.selection)
    }

    /// Apply `f` to every selected element on editable layers. `label` names the undo step.
    pub fn modify_selected(&mut self, label: &str, f: impl Fn(&mut Element)) {
        if self.selection.is_empty() {
            return;
        }
        self.checkpoint(label);
        self.modify_selected_continuous(f);
    }

    /// Like `modify_selected` without a new undo step (call `begin_drag` first).
    pub fn modify_selected_continuous(&mut self, f: impl Fn(&mut Element)) {
        let sels = self.selection.clone();
        let mut layers: Vec<usize> = sels.iter().map(|s| s.layer).collect();
        layers.dedup();
        for li in layers {
            if !self.layer_editable(li) {
                continue;
            }
            let Some(start) = self.target_keyframe(li, true) else { continue };
            let l = &mut self.timeline_mut().layers[li];
            if let Some(kf) = l.keyframes.get_mut(&start) {
                for s in sels.iter().filter(|s| s.layer == li) {
                    if let Some(el) = kf.elements.get_mut(s.element) {
                        f(el);
                    }
                }
            }
        }
    }
    pub fn begin_drag(&mut self) {
        self.checkpoint("Move");
    }
    /// Start a continuous edit with a named undo step (pair with the `*_continuous` APIs).
    pub fn begin_edit(&mut self, label: &str) {
        self.checkpoint(label);
    }

    /// Delete the selected elements from the keyframe they live in.
    pub fn delete_selected(&mut self) -> usize {
        let mut sels = self.selection.clone();
        sels.retain(|s| self.layer_editable(s.layer));
        if sels.is_empty() {
            return 0;
        }
        self.checkpoint(if sels.len() == 1 { "Delete" } else { "Delete Objects" });
        sels.sort();
        let mut removed = 0;
        for s in sels.iter().rev() {
            let e = s.element;
            self.remap_bones(s.layer, |i| if i == e { None } else if i > e { Some(i - 1) } else { Some(i) });
            let Some(start) = self.target_keyframe(s.layer, false) else { continue };
            let l = &mut self.timeline_mut().layers[s.layer];
            if let Some(kf) = l.keyframes.get_mut(&start) {
                if s.element < kf.elements.len() {
                    kf.elements.remove(s.element);
                    removed += 1;
                }
                // Keep a following tween's pairing intact: remove the same slot there too.
                let next = l.keyframes.range(start + 1..).next().map(|(k, _)| *k);
                let tweened = !matches!(l.keyframes[&start].tween, Tween::None);
                if tweened {
                    if let Some(n) = next {
                        let nk = l.keyframes.get_mut(&n).unwrap();
                        if s.element < nk.elements.len() {
                            nk.elements.remove(s.element);
                        }
                    }
                }
            }
        }
        self.selection.clear();
        removed
    }

    pub fn copy_selected(&mut self) -> usize {
        self.element_clip = self.selection.iter().filter_map(|s| self.element_at(*s).map(|e| (e, s.layer))).collect();
        self.element_clip.len()
    }
    pub fn cut_selected(&mut self) {
        if self.copy_selected() > 0 {
            self.delete_selected();
        }
    }
    pub fn can_paste(&self) -> bool {
        !self.element_clip.is_empty()
    }
    /// Paste onto the current layer (offset unless `in_place`).
    pub fn paste(&mut self, in_place: bool) {
        if self.element_clip.is_empty() || self.layer_locked() {
            return;
        }
        let clip = self.element_clip.clone();
        self.checkpoint(if in_place { "Paste in Place" } else { "Paste" });
        let li = self.layer;
        let Some(start) = self.target_keyframe(li, false) else { return };
        let kf = self.timeline_mut().layers[li].keyframes.get_mut(&start).unwrap();
        let first = kf.elements.len();
        for (mut el, _) in clip {
            if !in_place {
                el.matrix = Matrix::translate(10.0, 10.0).concat(&el.matrix);
            }
            kf.elements.push(el);
        }
        let n = kf.elements.len();
        self.selection = (first..n).map(|element| Selection { layer: li, element }).collect();
        self.frame_sel = None;
    }
    pub fn duplicate(&mut self) {
        if self.copy_selected() > 0 {
            if let Some(s) = self.selection.first() {
                self.layer = s.layer;
            }
            self.paste(false);
        }
    }

    /// Duplicate exactly on top of the original (used by Copy and Transform).
    pub fn duplicate_in_place(&mut self) {
        if self.copy_selected() > 0 {
            if let Some(s) = self.selection.first() {
                self.layer = s.layer;
            }
            self.paste(true);
        }
    }

    /// Ctrl+Up/Down style arrange within one layer (first selected element).
    pub fn arrange(&mut self, to_front: bool) {
        let Some(&s) = self.selection.first() else { return };
        if !self.layer_editable(s.layer) {
            return;
        }
        self.checkpoint(if to_front { "Bring to Front" } else { "Send to Back" });
        let Some(start) = self.target_keyframe(s.layer, false) else { return };
        let els = &mut self.timeline_mut().layers[s.layer].keyframes.get_mut(&start).unwrap().elements;
        if s.element >= els.len() {
            return;
        }
        let n = els.len();
        let e = els.remove(s.element);
        let idx = if to_front { els.len() } else { 0 };
        els.insert(idx, e);
        // keep bone references pointing at the same objects
        let mut order: Vec<usize> = (0..n).collect();
        let moved = order.remove(s.element);
        order.insert(idx, moved);
        self.remap_bones(s.layer, |i| order.iter().position(|&o| o == i));
        self.selection = vec![Selection { layer: s.layer, element: idx }];
    }

    /// F8 – Convert to Symbol. All selected objects on the first selected
    /// layer go into the new symbol; registration point = centre of their bounds.
    pub fn convert_to_symbol(&mut self, name: &str, kind: SymbolKind) -> Option<SymbolId> {
        let first = *self.selection.first()?;
        let sels: Vec<Selection> = self.selection.iter().copied().filter(|s| s.layer == first.layer).collect();
        let els: Vec<Element> = sels.iter().filter_map(|s| self.element_at(*s)).collect();
        if els.is_empty() || !self.layer_editable(first.layer) {
            return None;
        }
        let (lo, hi) = self.bounds_of(&sels)?;
        let reg = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        self.checkpoint("Convert to Symbol");
        let inner: Vec<Element> = els
            .iter()
            .map(|el| {
                let mut e = el.clone();
                e.matrix = Matrix::translate(-reg.x, -reg.y).concat(&el.matrix);
                e
            })
            .collect();
        let mut tl = Timeline::new(name);
        tl.layers[0].keyframes.insert(0, Keyframe::with(inner));
        let id = self.doc.add_symbol(name, kind, tl);
        let start = self.target_keyframe(first.layer, false)?;
        let slot = sels[0].element;
        let gone: Vec<usize> = sels.iter().map(|s| s.element).collect();
        let kept: Vec<usize> = gone.iter().skip(1).copied().collect();
        let mapping = move |i: usize| if gone.contains(&i) { None } else { Some(i - kept.iter().filter(|&&k| k < i).count()) };
        self.remap_bones(first.layer, mapping);
        let kf = self.timeline_mut().layers[first.layer].keyframes.get_mut(&start).unwrap();
        for s in sels.iter().skip(1).rev() {
            if s.element < kf.elements.len() {
                kf.elements.remove(s.element);
            }
        }
        if slot < kf.elements.len() {
            kf.elements[slot] = Element::instance(name, id, Matrix::translate(reg.x, reg.y));
        }
        self.selection = vec![Selection { layer: first.layer, element: slot }];
        Some(id)
    }

    /// Break Apart (Ctrl+B) for symbol instances: replace with the symbol's
    /// current-frame content, keeping the visual result.
    pub fn break_apart(&mut self) {
        let Some(&s) = self.selection.first() else { return };
        let Some(el) = self.element_at(s) else { return };
        let ElementKind::Instance(inst) = &el.kind else { return };
        let Some(sym) = self.doc.symbol(inst.symbol) else { return };
        let mut parts: Vec<Element> = Vec::new();
        for layer in sym.timeline.layers.iter().rev() {
            if let Some((els, _)) = layer_elements_at(layer, inst.first_frame.min(sym.timeline.frame_count() - 1)) {
                for mut e in els {
                    e.matrix = el.matrix.concat(&e.matrix);
                    e.alpha *= el.alpha;
                    parts.push(e);
                }
            }
        }
        if !self.layer_editable(s.layer) {
            return;
        }
        self.checkpoint("Break Apart");
        let Some(start) = self.target_keyframe(s.layer, false) else { return };
        let els = &mut self.timeline_mut().layers[s.layer].keyframes.get_mut(&start).unwrap().elements;
        if s.element >= els.len() {
            return;
        }
        els.remove(s.element);
        let n = parts.len();
        for (i, p) in parts.into_iter().enumerate() {
            els.insert(s.element + i, p);
        }
        let at = s.element;
        self.remap_bones(s.layer, |i| if i == at { None } else if i > at { Some(i + n - 1) } else { Some(i) });
        self.selection = (s.element..s.element + n).map(|element| Selection { layer: s.layer, element }).collect();
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

    /// Number of instances of `id` anywhere in the document.
    pub fn symbol_usage(&self, id: SymbolId) -> usize {
        let count = |tl: &Timeline| {
            tl.layers.iter().flat_map(|l| l.keyframes.values()).flat_map(|k| &k.elements).filter(|e| matches!(&e.kind, ElementKind::Instance(i) if i.symbol == id)).count()
        };
        count(&self.doc.scene) + self.doc.library.iter().map(|s| count(&s.timeline)).sum::<usize>()
    }

    /// Delete a library item and every instance of it.
    pub fn delete_symbol(&mut self, id: SymbolId) {
        self.checkpoint("Delete Library Item");
        let strip = |tl: &mut Timeline| {
            for l in &mut tl.layers {
                for k in l.keyframes.values_mut() {
                    k.elements.retain(|e| !matches!(&e.kind, ElementKind::Instance(i) if i.symbol == id));
                }
            }
        };
        strip(&mut self.doc.scene);
        for s in &mut self.doc.library {
            strip(&mut s.timeline);
        }
        self.doc.library.retain(|s| s.id != id);
        if self.editing == Some(id) {
            self.editing = None;
        }
        self.fix_indices();
    }

    pub fn duplicate_symbol(&mut self, id: SymbolId) -> Option<SymbolId> {
        let s = self.doc.symbol(id)?.clone();
        self.checkpoint("Duplicate Symbol");
        Some(self.doc.add_symbol(&format!("{} copy", s.name), s.kind, s.timeline))
    }

    pub fn set_symbol_kind(&mut self, id: SymbolId, kind: SymbolKind) {
        self.checkpoint("Symbol Type");
        if let Some(s) = self.doc.symbol_mut(id) {
            s.kind = kind;
        }
    }

    pub fn set_doc_props(&mut self, w: f32, h: f32, fps: f32, bg: Color) {
        self.checkpoint("Document Settings");
        self.doc.width = w;
        self.doc.height = h;
        self.doc.fps = fps;
        self.doc.background = bg;
    }

    /// 3D perspective: field of view and vanishing point (`None` = stage centre).
    pub fn set_perspective(&mut self, angle: f32, vanishing: Option<Pt>) {
        self.checkpoint("Perspective");
        self.doc.perspective_angle = angle.clamp(1.0, 179.0);
        self.doc.vanishing_point = vanishing;
    }

    /// Paint bucket on an element: sets fill (shapes) or tint (instances).
    pub fn paint_bucket(&mut self, sel: Selection, color: Color) {
        self.selection = vec![sel];
        self.modify_selected("Paint Bucket", |el| match &mut el.kind {
            ElementKind::Shape(s) => s.fill = Some(color),
            ElementKind::Instance(_) => el.tint = Some((color, 0.6)),
            ElementKind::Text(t) => t.color = color,
        });
    }
}

// ------------------------------------------------------------ bones (IK)
impl Editor {
    pub fn bones(&self, layer: usize) -> &[Bone] {
        self.timeline().layers.get(layer).map(|l| l.bones.as_slice()).unwrap_or(&[])
    }

    /// Re-index bones after objects on a layer were removed/reordered;
    /// bones touching a removed object are dropped.
    pub fn remap_bones(&mut self, layer: usize, f: impl Fn(usize) -> Option<usize>) {
        let Some(l) = self.timeline_mut().layers.get_mut(layer) else { return };
        if l.bones.is_empty() {
            return;
        }
        l.bones = l
            .bones
            .iter()
            .filter_map(|b| Some(Bone { parent: f(b.parent)?, child: f(b.child)?, ..*b }))
            .collect();
    }

    /// Objects connected below `e` (including `e`).
    pub fn bone_subtree(bones: &[Bone], e: usize) -> Vec<usize> {
        let mut out = vec![e];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i];
            for b in bones.iter().filter(|b| b.parent == cur) {
                if !out.contains(&b.child) {
                    out.push(b.child);
                }
            }
            i += 1;
        }
        out
    }

    /// Joint an object rotates around: where it attaches to its parent, or
    /// (for the root) the head of its first bone.
    pub fn bone_pivot(bones: &[Bone], els: &[Element], e: usize) -> Option<Pt> {
        if let Some(b) = bones.iter().find(|b| b.child == e) {
            return els.get(e).map(|el| el.matrix.apply(b.tail_child));
        }
        bones.iter().find(|b| b.parent == e).and_then(|b| els.get(e).map(|el| el.matrix.apply(b.head)))
    }

    /// Bone tool: link `parent` → `child` with a bone from `head` to `tail` (doc coords).
    pub fn add_bone(&mut self, layer: usize, parent: usize, child: usize, head: Pt, tail: Pt) -> Result<(), String> {
        if parent == child {
            return Err("drag from one object to a different one".into());
        }
        if !self.layer_editable(layer) {
            return Err("the layer is locked or hidden".into());
        }
        let bones = self.bones(layer).to_vec();
        if bones.iter().any(|b| b.child == child) {
            return Err("that object is already attached to a bone".into());
        }
        if Self::bone_subtree(&bones, child).contains(&parent) {
            return Err("that would make a loop in the armature".into());
        }
        let l = &self.timeline().layers[layer];
        let (els, _) = layer_elements_at(l, self.frame).ok_or("nothing on this frame")?;
        let pm = els.get(parent).ok_or("no such object")?.matrix.invert().ok_or("object has no size")?;
        let cm = els.get(child).ok_or("no such object")?.matrix.invert().ok_or("object has no size")?;
        let bone = Bone { parent, child, head: pm.apply(head), tail_parent: pm.apply(tail), tail_child: cm.apply(tail) };
        self.checkpoint("Add Bone");
        self.timeline_mut().layers[layer].bones.push(bone);
        Ok(())
    }

    /// Remove the bone that attaches `child` to its parent.
    pub fn remove_bone(&mut self, layer: usize, child: usize) {
        if !self.bones(layer).iter().any(|b| b.child == child) {
            return;
        }
        self.checkpoint("Remove Bone");
        self.timeline_mut().layers[layer].bones.retain(|b| b.child != child);
    }

    pub fn clear_bones(&mut self, layer: usize) {
        if self.bones(layer).is_empty() {
            return;
        }
        self.checkpoint("Remove Bones");
        self.timeline_mut().layers[layer].bones.clear();
    }

    /// Inverse kinematics: pull the point `grab` (local to `element`) towards
    /// `target`, rotating the chain of parents (CCD). Dragging an armature's
    /// root moves the whole armature. Writes a pose keyframe at the playhead.
    /// Call `begin_drag` first so a drag is one undo step.
    pub fn ik_drag(&mut self, layer: usize, element: usize, grab: Pt, target: Pt) {
        let f = self.frame;
        let Some(l) = self.timeline().layers.get(layer) else { return };
        let bones = l.bones.clone();
        let Some((mut els, _)) = layer_elements_at(l, f) else { return };
        if element >= els.len() {
            return;
        }
        let mut chain = vec![element];
        while let Some(b) = bones.iter().find(|b| b.child == *chain.last().unwrap()) {
            if chain.contains(&b.parent) {
                break;
            }
            chain.push(b.parent);
        }
        let root = *chain.last().unwrap();
        let affected = Self::bone_subtree(&bones, root);
        if chain.len() == 1 {
            let eff = els[element].matrix.apply(grab);
            for &y in &affected {
                els[y].matrix.tx += target.x - eff.x;
                els[y].matrix.ty += target.y - eff.y;
            }
        } else {
            for _ in 0..16 {
                for &x in &chain {
                    let Some(j) = Self::bone_pivot(&bones, &els, x) else { continue };
                    let eff = els[element].matrix.apply(grab);
                    let a1 = (eff.y - j.y).atan2(eff.x - j.x);
                    let a2 = (target.y - j.y).atan2(target.x - j.x);
                    let mut d = a2 - a1;
                    while d > std::f32::consts::PI {
                        d -= std::f32::consts::TAU;
                    }
                    while d < -std::f32::consts::PI {
                        d += std::f32::consts::TAU;
                    }
                    if eff.dist(j) < 1e-3 {
                        continue;
                    }
                    let r = Matrix::translate(j.x, j.y).concat(&Matrix::rotate(d)).concat(&Matrix::translate(-j.x, -j.y));
                    for y in Self::bone_subtree(&bones, x) {
                        els[y].matrix = r.concat(&els[y].matrix);
                    }
                }
                if els[element].matrix.apply(grab).dist(target) < 0.25 {
                    break;
                }
            }
        }
        apply_bones(&bones, &mut els);
        let l = &mut self.timeline_mut().layers[layer];
        if !l.keyframes.contains_key(&f) {
            frames::convert_to_keyframes(l, f, f, false);
        }
        if let Some(kf) = l.keyframes.get_mut(&f) {
            for y in affected {
                if let (Some(dst), Some(src)) = (kf.elements.get_mut(y), els.get(y)) {
                    dst.matrix = src.matrix;
                }
            }
        }
    }
}

// ------------------------------------------------------------ camera
impl Editor {
    /// Index of the scene's camera layer, if any.
    pub fn camera_layer(&self) -> Option<usize> {
        self.doc.scene.layers.iter().position(|l| l.kind == LayerKind::Camera)
    }

    /// Add a camera layer (top of the scene) if there isn't one.
    pub fn add_camera(&mut self) -> usize {
        if let Some(i) = self.camera_layer() {
            return i;
        }
        self.checkpoint("Add Camera");
        let end = self.doc.scene.frame_count();
        let mut l = Layer::new("Camera", 5);
        l.kind = LayerKind::Camera;
        l.end = end;
        l.keyframes.insert(0, Keyframe::with(vec![default_camera(&self.doc)]));
        self.doc.scene.layers.insert(0, l);
        if self.editing.is_none() {
            self.layer = (self.layer + 1).min(self.doc.scene.layers.len() - 1);
        }
        self.fix_layer_selection();
        0
    }

    pub fn remove_camera(&mut self) {
        if let Some(i) = self.camera_layer() {
            self.checkpoint("Remove Camera");
            self.doc.scene.layers.remove(i);
            self.fix_layer_selection();
        }
    }

    fn fix_layer_selection(&mut self) {
        let n = self.timeline().layers.len();
        self.layer = self.layer.min(n.saturating_sub(1));
        self.selection.clear();
        self.frame_sel = None;
    }

    /// The camera element's matrix at `frame` (centre, rotation, zoom).
    pub fn camera_at(&self, frame: u32) -> Option<Matrix> {
        let l = &self.doc.scene.layers[self.camera_layer()?];
        layer_elements_at(l, frame).and_then(|(e, _)| e.first().map(|c| c.matrix))
    }

    /// Set the camera at the playhead (makes a camera keyframe). No undo step:
    /// call `begin_drag` (or `checkpoint` via a command) first.
    pub fn set_camera(&mut self, cam: Matrix) {
        let f = self.frame;
        let Some(i) = self.camera_layer() else { return };
        let doc_cam = default_camera(&self.doc);
        let l = &mut self.doc.scene.layers[i];
        if !l.keyframes.contains_key(&f) {
            frames::convert_to_keyframes(l, f, f, false);
        }
        let kf = l.keyframes.get_mut(&f).unwrap();
        if kf.elements.is_empty() {
            kf.elements.push(doc_cam);
        }
        kf.elements[0].matrix = cam;
    }

    pub fn camera_params(cam: &Matrix) -> (Pt, f32, f32) {
        let d = cam.decompose();
        (pt(cam.tx, cam.ty), d.skew_y.to_degrees(), d.scale_x.abs())
    }
    pub fn camera_from(center: Pt, rotation_deg: f32, zoom: f32) -> Matrix {
        Matrix::translate(center.x, center.y).concat(&Matrix::rotate(rotation_deg.to_radians())).concat(&Matrix::scale(zoom.max(0.01), zoom.max(0.01)))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LayerFlag {
    Visible,
    Locked,
    Outline,
}
