//! Stage: rendering, tool input, selection overlays and the stage context menu.

use crate::toolbar::{PencilMode, Tool};
use crate::{from32, App, ACCENT};
use ac_core::{pt, Color, Element, ElementKind, Matrix, Path, PointRef, Pt, Seg, Selection, Shape, Stroke};
use eframe::egui::{self, pos2, vec2, Color32, CursorIcon, Key, Pos2, Rect, Sense, Stroke as EStroke, Vec2};
use tiny_skia as sk;

pub enum Drag {
    None,
    Move { last: Pt, moved: bool, raw: Pt, applied: Pt, start: Option<(Pt, Pt)>, targets: (Vec<f32>, Vec<f32>) },
    Transform { mode: TMode, applied: Matrix, started: bool },
    Marquee { start: Pt, cur: Pt, add: bool },
    Draw { start: Pt, cur: Pt },
    Freehand { pts: Vec<Pt> },
    Erase { mark: usize, removed: usize, last: Pt },
    Point { sel: Selection, r: PointRef, started: bool },
    ZoomBox { start: Pos2, cur: Pos2 },
    Pan { last: Pos2 },
}

#[derive(Clone, Copy)]
pub enum TMode {
    Scale { anchor: Pt, start: Pt },
    Rotate { center: Pt, start_angle: f32 },
}

/// Pen tool path in progress: anchors with their outgoing handle offsets.
#[derive(Default)]
pub struct PenState {
    pub pts: Vec<(Pt, Pt)>,
    pub dragging: bool,
}

/// Inline text editing (Text tool).
pub struct TextEdit {
    pub sel: Selection,
    pub buffer: String,
    pub mark: usize,
    pub focus_requested: bool,
}

impl App {
    fn content_offset(&self) -> Vec2 {
        if self.ed.editing.is_some() {
            vec2(self.ed.doc.width / 2.0, self.ed.doc.height / 2.0)
        } else {
            Vec2::ZERO
        }
    }
    pub(crate) fn to_screen(&self, rect: Rect, p: Pt) -> Pos2 {
        let o = self.content_offset();
        rect.min + self.pan + vec2(p.x + o.x, p.y + o.y) * self.zoom
    }
    pub(crate) fn to_doc(&self, rect: Rect, s: Pos2) -> Pt {
        let o = self.content_offset();
        let v = (s - rect.min - self.pan) / self.zoom - o;
        pt(v.x, v.y)
    }
    fn fit(&mut self, rect: Rect) {
        let d = &self.ed.doc;
        self.zoom = ((rect.width() - 60.0) / d.width).min((rect.height() - 60.0) / d.height).clamp(0.1, 8.0);
        self.pan = (rect.size() - vec2(d.width, d.height) * self.zoom) / 2.0;
    }
    pub(crate) fn zoom_at(&mut self, rect: Rect, anchor: Pos2, factor: f32) {
        let new = (self.zoom * factor).clamp(0.05, 32.0);
        let a = anchor - rect.min;
        self.pan = a - (a - self.pan) * (new / self.zoom);
        self.zoom = new;
    }
    fn cur_fill(&self) -> Option<Color> {
        self.fill_on.then(|| from32(self.fill))
    }
    fn cur_stroke(&self) -> Option<Stroke> {
        self.stroke_on.then(|| Stroke { color: from32(self.stroke), width: self.stroke_w })
    }
    fn locked_msg(&mut self) {
        self.status = "The current layer is locked or hidden. Pick another layer in the timeline.".into();
    }

    fn handle_hit(&self, rect: Rect, pos: Pos2, tool: Tool) -> Option<(usize, bool)> {
        let (lo, hi) = self.ed.selection_bounds()?;
        let corners = [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(lo.x, hi.y), pt(hi.x, hi.y)];
        let mut best: Option<(usize, bool, f32)> = None;
        for (i, c) in corners.iter().enumerate() {
            let d = self.to_screen(rect, *c).distance(pos);
            if d < 7.0 && best.map(|b| d < b.2).unwrap_or(true) {
                best = Some((i, false, d));
            } else if tool == Tool::FreeTransform && d < 24.0 && best.is_none() {
                best = Some((i, true, d));
            }
        }
        best.map(|(i, r, _)| (i, r))
    }

    /// A selected (or clicked) shape and the doc→local transform for point editing.
    fn shape_for_points(&mut self, p: Pt, select_hit: bool) -> Option<(Selection, Matrix, Path)> {
        let candidates: Vec<Selection> = self.ed.selection.clone();
        for s in candidates {
            if let Some(el) = self.ed.element_at(s) {
                if let ElementKind::Shape(sh) = &el.kind {
                    let inv = el.matrix.invert()?;
                    let tol = 8.0 / self.zoom;
                    let lp = inv.apply(p);
                    if sh.path.pick_point(lp, tol / el.matrix.decompose().scale_x.abs().max(0.01), true).is_some() || sh.path.hit(lp, true, tol) {
                        return Some((s, inv, sh.path.clone()));
                    }
                }
            }
        }
        if !select_hit {
            return None;
        }
        let hit = self.ed.hit_test(p)?;
        let el = self.ed.element_at(hit)?;
        match &el.kind {
            ElementKind::Shape(sh) => {
                self.ed.select(Some(hit), false);
                Some((hit, el.matrix.invert()?, sh.path.clone()))
            }
            _ => {
                self.ed.select(Some(hit), false);
                self.status = "Point editing works on shapes. Break apart symbols/text first (Ctrl+B).".into();
                None
            }
        }
    }

    pub(crate) fn finish_pen(&mut self, close: bool) {
        let Some(pen) = self.pen.take() else { return };
        if pen.pts.len() < 2 {
            return;
        }
        let mut segs = vec![Seg::M(pen.pts[0].0)];
        let seg = |(a, ha): (Pt, Pt), (b, hb): (Pt, Pt)| {
            if ha.x.abs() + ha.y.abs() + hb.x.abs() + hb.y.abs() < 0.01 {
                Seg::L(b)
            } else {
                Seg::C(pt(a.x + ha.x, a.y + ha.y), pt(b.x - hb.x, b.y - hb.y), b)
            }
        };
        for w in pen.pts.windows(2) {
            segs.push(seg(w[0], w[1]));
        }
        if close {
            segs.push(seg(*pen.pts.last().unwrap(), pen.pts[0]));
            segs.push(Seg::Z);
        }
        let fill = if close { self.cur_fill() } else { None };
        let stroke = self.cur_stroke().or(if fill.is_none() { Some(Stroke { color: from32(self.stroke), width: self.stroke_w }) } else { None });
        if self.ed.add_element(Element::shape("Pen Path", Shape { path: Path { segs }, fill, stroke })).is_none() {
            self.locked_msg();
        }
    }

    pub(crate) fn finish_text(&mut self) {
        let Some(te) = self.text_edit.take() else { return };
        if te.buffer.trim().is_empty() {
            self.ed.selection = vec![te.sel];
            self.ed.delete_selected();
        }
        self.ed.squash_undo(te.mark, "Text");
    }

    fn snap_targets(&self) -> (Vec<f32>, Vec<f32>) {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        if self.ed.editing.is_none() {
            let (w, h) = (self.ed.doc.width, self.ed.doc.height);
            xs.extend([0.0, w / 2.0, w]);
            ys.extend([0.0, h / 2.0, h]);
        } else {
            xs.push(0.0);
            ys.push(0.0);
        }
        for it in self.ed.evaluate_current(self.ed.frame) {
            let Some((l, e)) = it.src else { continue };
            if self.ed.is_selected(Selection { layer: l, element: e }) {
                continue;
            }
            if let Some((lo, hi)) = it.path.transformed(&it.matrix).bounds() {
                xs.extend([lo.x, (lo.x + hi.x) / 2.0, hi.x]);
                ys.extend([lo.y, (lo.y + hi.y) / 2.0, hi.y]);
            }
        }
        (xs, ys)
    }
}

fn snap_axis(vals: [f32; 3], targets: &[f32], thr: f32) -> (f32, Option<f32>) {
    let mut best: Option<(f32, f32)> = None; // (adjust, target)
    for v in vals {
        for t in targets {
            let d = t - v;
            if d.abs() <= thr && best.map(|b| d.abs() < b.0.abs()).unwrap_or(true) {
                best = Some((d, *t));
            }
        }
    }
    best.map(|(d, t)| (d, Some(t))).unwrap_or((0.0, None))
}

fn point_in_poly(poly: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Select editable objects whose bounds lie entirely inside a lasso polygon.
fn lasso_select(app: &mut App, poly: &[Pt], add: bool) {
    let mut found = Vec::new();
    for it in app.ed.evaluate_current(app.ed.frame) {
        let Some((l, e)) = it.src else { continue };
        if !app.ed.layer_editable(l) {
            continue;
        }
        let s = Selection { layer: l, element: e };
        if found.contains(&s) {
            continue;
        }
        if let Some((lo, hi)) = app.ed.bounds_of(&[s]) {
            if [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(lo.x, hi.y), pt(hi.x, hi.y)].iter().all(|c| point_in_poly(poly, *c)) {
                found.push(s);
            }
        }
    }
    if !add {
        app.ed.selection.clear();
    }
    for s in found {
        if !app.ed.is_selected(s) {
            app.ed.selection.push(s);
        }
    }
    app.ed.selection.sort();
    app.ed.frame_sel = None;
    app.status = format!("{} object(s) selected", app.ed.selection.len());
}

/// Paint Brush: turn a smoothed centre line into a filled outline.
fn brush_outline(pts: &[Pt], size: f32, smooth: f32) -> Option<Path> {
    if pts.len() == 1 {
        return Some(Path::ellipse(pts[0].x, pts[0].y, size / 2.0, size / 2.0));
    }
    let center = Path::smooth_polyline(pts, 0.3 + smooth / 100.0 * 3.0);
    let mut b = sk::PathBuilder::new();
    for s in &center.segs {
        match *s {
            Seg::M(p) => b.move_to(p.x, p.y),
            Seg::L(p) => b.line_to(p.x, p.y),
            Seg::Q(c, p) => b.quad_to(c.x, c.y, p.x, p.y),
            Seg::C(c1, c2, p) => b.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
            Seg::Z => b.close(),
        }
    }
    let path = b.finish()?;
    let st = sk::Stroke { width: size.max(0.5), line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() };
    let outline = path.stroke(&st, 1.0)?;
    let mut segs = Vec::new();
    for seg in outline.segments() {
        segs.push(match seg {
            sk::PathSegment::MoveTo(p) => Seg::M(pt(p.x, p.y)),
            sk::PathSegment::LineTo(p) => Seg::L(pt(p.x, p.y)),
            sk::PathSegment::QuadTo(c, p) => Seg::Q(pt(c.x, c.y), pt(p.x, p.y)),
            sk::PathSegment::CubicTo(c1, c2, p) => Seg::C(pt(c1.x, c1.y), pt(c2.x, c2.y), pt(p.x, p.y)),
            sk::PathSegment::Close => Seg::Z,
        });
    }
    Some(Path { segs })
}

pub fn stage(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_gray(0x3C))).show(ctx, |ui| {
        egui::Frame::none().fill(Color32::from_gray(0x2E)).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let crumbs = app.ed.breadcrumb();
                for (i, c) in crumbs.iter().enumerate() {
                    if i > 0 {
                        ui.label("›");
                    }
                    let icon = if i == 0 { "🎬" } else { "🔷" };
                    if ui.link(format!("{icon} {c}")).clicked() && i == 0 && app.ed.editing.is_some() {
                        app.ed.exit_to_scene();
                        app.fit_pending = true;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Fit").clicked() {
                        app.fit_pending = true;
                    }
                    ui.label(format!("{:.0}%", app.zoom * 100.0));
                });
            });
        });
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        app.stage_rect = rect;
        if app.fit_pending {
            app.fit(rect);
            app.fit_pending = false;
        }
        if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
            if app.text_edit.is_some() && !matches!(app.tool, Tool::Text) {
                app.finish_text();
            }
            if let Some(id) = ui.ctx().memory(|m| m.focused()) {
                if app.text_edit.is_none() {
                    ui.ctx().memory_mut(|m| m.surrender_focus(id));
                }
            }
        }
        if resp.hovered() {
            let space = ui.input(|i| i.key_down(Key::Space));
            let icon = match if space { Tool::Hand } else { app.tool } {
                Tool::Hand => if resp.dragged() { CursorIcon::Grabbing } else { CursorIcon::Grab },
                Tool::Zoom => if ui.input(|i| i.modifiers.alt) { CursorIcon::ZoomOut } else { CursorIcon::ZoomIn },
                Tool::Text => CursorIcon::Text,
                Tool::Select | Tool::FreeTransform => CursorIcon::Default,
                t if !t.usable() => CursorIcon::NotAllowed,
                _ => CursorIcon::Crosshair,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        stage_input(app, ui, rect, &resp);
        resp.context_menu(|ui| app.object_menu(ui));
        render_stage(app, ui, rect);
        overlays(app, ui, rect);
        text_editor(app, ui, rect);
    });
}

fn stage_input(app: &mut App, ui: &egui::Ui, rect: Rect, resp: &egui::Response) {
    if let Some(hover) = resp.hover_pos() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            app.zoom_at(rect, hover, if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 });
        }
    }
    let (space, shift, alt) = ui.input(|i| (i.key_down(Key::Space), i.modifiers.shift, i.modifiers.alt));
    let tool = if space { Tool::Hand } else { app.tool };

    // Pen: Enter finishes an open path, Escape too.
    if app.pen.is_some() && ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape)) {
        app.finish_pen(false);
    }
    if !app.poly_lasso.is_empty() && ui.input(|i| i.key_pressed(Key::Escape)) {
        app.poly_lasso.clear();
    }

    let Some(pos) = resp.interact_pointer_pos() else { return };
    let p = app.to_doc(rect, pos);
    let origin = if resp.drag_started() { ui.input(|i| i.pointer.press_origin()).unwrap_or(pos) } else { pos };
    let p0 = app.to_doc(rect, origin);

    if resp.secondary_clicked() {
        let hit = app.ed.hit_test(p);
        if hit.map(|h| !app.ed.is_selected(h)).unwrap_or(false) {
            app.ed.select(hit, false);
        }
        return;
    }
    if resp.double_clicked() {
        match tool {
            Tool::Select | Tool::FreeTransform => {
                if let Some(h) = app.ed.hit_test(p) {
                    match app.ed.element_at(h).map(|e| e.kind) {
                        Some(ElementKind::Instance(i)) => {
                            app.edit_symbol(i.symbol);
                            return;
                        }
                        Some(ElementKind::Text(t)) => {
                            app.set_tool(Tool::Text);
                            app.ed.select(Some(h), false);
                            app.text_edit = Some(TextEdit { sel: h, buffer: t.text, mark: app.ed.undo_depth(), focus_requested: false });
                            return;
                        }
                        _ => {}
                    }
                }
            }
            Tool::Pen => {
                // the double-click's second press added a duplicate point
                if let Some(pen) = &mut app.pen {
                    if pen.pts.len() >= 2 && pen.pts[pen.pts.len() - 1].0.dist(pen.pts[pen.pts.len() - 2].0) < 3.0 / app.zoom {
                        pen.pts.pop();
                    }
                }
                app.finish_pen(false);
                return;
            }
            Tool::PolyLasso => {
                let poly = std::mem::take(&mut app.poly_lasso);
                lasso_select(app, &poly, shift);
                return;
            }
            _ => {}
        }
    }

    let pressed = resp.drag_started() || (resp.clicked() && matches!(app.drag, crate::stage::Drag::None));
    if pressed {
        app.playing = false;
        app.ed.frame_sel = None;
        if !tool.usable() {
            app.status = format!("{} is planned: {}", tool.info().name, tool.info().planned.unwrap_or(""));
            return;
        }
        match tool {
            Tool::Hand => app.drag = Drag::Pan { last: origin },
            Tool::Zoom => app.drag = Drag::ZoomBox { start: origin, cur: origin },
            Tool::Select | Tool::FreeTransform => {
                if let Some((corner, rotate)) = app.handle_hit(rect, origin, tool) {
                    let (lo, hi) = app.ed.selection_bounds().unwrap();
                    app.drag = if rotate {
                        let c = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
                        Drag::Transform { mode: TMode::Rotate { center: c, start_angle: (p0.y - c.y).atan2(p0.x - c.x) }, applied: Matrix::IDENTITY, started: false }
                    } else {
                        let opposite = [pt(hi.x, hi.y), pt(lo.x, hi.y), pt(hi.x, lo.y), pt(lo.x, lo.y)][corner];
                        Drag::Transform { mode: TMode::Scale { anchor: opposite, start: p0 }, applied: Matrix::IDENTITY, started: false }
                    };
                } else if let Some(hit) = app.ed.hit_test(p0) {
                    if shift {
                        app.ed.select(Some(hit), true);
                    } else if !app.ed.is_selected(hit) {
                        app.ed.select(Some(hit), false);
                    }
                    if app.ed.is_selected(hit) {
                        let targets = if app.topts.snap { app.snap_targets() } else { (vec![], vec![]) };
                        app.drag = Drag::Move { last: p0, moved: false, raw: pt(0.0, 0.0), applied: pt(0.0, 0.0), start: app.ed.selection_bounds(), targets };
                    }
                } else {
                    if !shift {
                        app.ed.select(None, false);
                    }
                    app.drag = Drag::Marquee { start: p0, cur: p0, add: shift };
                }
            }
            Tool::Subselect => {
                if let Some((s, inv, path)) = app.shape_for_points(p0, true) {
                    let scale = app.ed.element_at(s).map(|e| e.matrix.decompose().scale_x.abs().max(0.01)).unwrap_or(1.0);
                    if let Some(r) = path.pick_point(inv.apply(p0), 8.0 / app.zoom / scale, true) {
                        app.drag = Drag::Point { sel: s, r, started: false };
                    } else {
                        let targets = (vec![], vec![]);
                        app.drag = Drag::Move { last: p0, moved: false, raw: pt(0.0, 0.0), applied: pt(0.0, 0.0), start: None, targets };
                    }
                } else if app.ed.hit_test(p0).is_none() {
                    app.ed.select(None, false);
                }
            }
            Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor => {
                if let Some((s, inv, path)) = app.shape_for_points(p0, true) {
                    let scale = app.ed.element_at(s).map(|e| e.matrix.decompose().scale_x.abs().max(0.01)).unwrap_or(1.0);
                    let lp = inv.apply(p0);
                    let tol = 8.0 / app.zoom / scale;
                    let mut path2 = path.clone();
                    let changed = match tool {
                        Tool::AddAnchor => path2.insert_anchor(lp, tol).is_some(),
                        Tool::DeleteAnchor => match path.pick_point(lp, tol, false) {
                            Some(PointRef::Anchor(i)) => path2.delete_anchor(i),
                            _ => false,
                        },
                        _ => match path.pick_point(lp, tol, false) {
                            Some(PointRef::Anchor(i)) => {
                                path2.convert_anchor(i);
                                true
                            }
                            _ => false,
                        },
                    };
                    if changed {
                        app.ed.selection = vec![s];
                        let label = tool.info().name;
                        app.ed.modify_selected(label, move |e| {
                            if let ElementKind::Shape(sh) = &mut e.kind {
                                sh.path = path2.clone();
                            }
                        });
                    } else {
                        app.status = match tool {
                            Tool::AddAnchor => "Click on the shape's outline to add a point.".into(),
                            _ => "Click on an anchor point.".into(),
                        };
                    }
                }
            }
            Tool::Pen => {
                if !app.ed.layer_editable(app.ed.layer) {
                    app.locked_msg();
                    return;
                }
                let first = app.pen.as_ref().and_then(|pn| pn.pts.first().map(|f| f.0));
                let n = app.pen.as_ref().map(|pn| pn.pts.len()).unwrap_or(0);
                if let Some(f) = first {
                    if n >= 2 && app.to_screen(rect, f).distance(origin) < 8.0 {
                        app.finish_pen(true);
                        return;
                    }
                }
                let pen = app.pen.get_or_insert_with(Default::default);
                pen.pts.push((p0, pt(0.0, 0.0)));
                pen.dragging = true;
            }
            Tool::Lasso => app.drag = Drag::Freehand { pts: vec![p0] },
            Tool::PolyLasso => {
                if app.poly_lasso.len() >= 3 && app.to_screen(rect, app.poly_lasso[0]).distance(origin) < 8.0 {
                    let poly = std::mem::take(&mut app.poly_lasso);
                    lasso_select(app, &poly, shift);
                } else {
                    app.poly_lasso.push(p0);
                }
            }
            Tool::Text => {
                if app.text_edit.is_some() {
                    app.finish_text();
                }
                if let Some(h) = app.ed.hit_test(p0) {
                    if let Some(ElementKind::Text(t)) = app.ed.element_at(h).map(|e| e.kind) {
                        app.ed.select(Some(h), false);
                        app.text_edit = Some(TextEdit { sel: h, buffer: t.text, mark: app.ed.undo_depth(), focus_requested: false });
                        return;
                    }
                }
                let mark = app.ed.undo_depth();
                let mut el = Element::text("", app.topts.text_size, from32(app.fill), p0);
                if let ElementKind::Text(t) = &mut el.kind {
                    t.font = app.topts.text_font;
                    t.align = app.topts.text_align;
                }
                match app.ed.add_element(el) {
                    Some(s) => app.text_edit = Some(TextEdit { sel: s, buffer: String::new(), mark, focus_requested: false }),
                    None => app.locked_msg(),
                }
            }
            Tool::Rect | Tool::Oval | Tool::Line | Tool::PolyStar => app.drag = Drag::Draw { start: p0, cur: p0 },
            Tool::Pencil | Tool::Brush => app.drag = Drag::Freehand { pts: vec![p0] },
            Tool::Eraser => {
                let mark = app.ed.undo_depth();
                let removed = erase_at(app, p0);
                app.drag = Drag::Erase { mark, removed, last: p0 };
            }
            Tool::Bucket => match app.ed.hit_test(p0) {
                Some(s) => app.ed.paint_bucket(s, from32(app.fill)),
                None => app.status = "Paint Bucket: click inside a shape.".into(),
            },
            Tool::InkBottle => match app.ed.hit_test(p0) {
                Some(s) => {
                    app.ed.selection = vec![s];
                    let st = Stroke { color: from32(app.stroke), width: app.stroke_w };
                    app.ed.modify_selected("Ink Bottle", move |e| {
                        if let ElementKind::Shape(sh) = &mut e.kind {
                            sh.stroke = Some(st);
                        }
                    });
                }
                None => app.status = "Ink Bottle: click a shape to give it an outline.".into(),
            },
            Tool::Eyedropper => eyedrop(app, p0),
            _ => {}
        }
    }

    if resp.dragged() {
        let zoom = app.zoom;
        let tool_now = app.tool;
        let snap_on = app.topts.snap && !alt;
        if let Some(pen) = app.pen.as_mut().filter(|pn| pn.dragging) {
            if let Some(last) = pen.pts.last_mut() {
                last.1 = pt(p.x - last.0.x, p.y - last.0.y);
            }
        }
        match &mut app.drag {
            Drag::Pan { last } => {
                app.pan += pos - *last;
                *last = pos;
            }
            Drag::ZoomBox { cur, .. } => *cur = pos,
            Drag::Move { last, moved, raw, applied, start, targets } => {
                let d = pt(p.x - last.x, p.y - last.y);
                *last = p;
                if !*moved {
                    app.ed.begin_drag();
                    *moved = true;
                }
                raw.x += d.x;
                raw.y += d.y;
                let mut want = *raw;
                app.snap_guides = (None, None);
                if let (true, Some((lo, hi))) = (snap_on, *start) {
                    let thr = 6.0 / zoom;
                    let (ax, gx) = snap_axis([lo.x + raw.x, (lo.x + hi.x) / 2.0 + raw.x, hi.x + raw.x], &targets.0, thr);
                    let (ay, gy) = snap_axis([lo.y + raw.y, (lo.y + hi.y) / 2.0 + raw.y, hi.y + raw.y], &targets.1, thr);
                    want = pt(raw.x + ax, raw.y + ay);
                    app.snap_guides = (gx, gy);
                }
                let inc = pt(want.x - applied.x, want.y - applied.y);
                *applied = want;
                app.ed.modify_selected_continuous(|e| {
                    e.matrix.tx += inc.x;
                    e.matrix.ty += inc.y;
                });
            }
            Drag::Transform { mode, applied, started } => {
                if !*started {
                    app.ed.begin_drag();
                    *started = true;
                }
                let total = match *mode {
                    TMode::Scale { anchor: a, start: s } => {
                        let mut sx = if (s.x - a.x).abs() > 0.5 { (p.x - a.x) / (s.x - a.x) } else { 1.0 };
                        let mut sy = if (s.y - a.y).abs() > 0.5 { (p.y - a.y) / (s.y - a.y) } else { 1.0 };
                        if shift {
                            let u = sx.abs().max(sy.abs());
                            sx = u * sx.signum();
                            sy = u * sy.signum();
                        }
                        let clamp = |v: f32| if v.abs() < 0.01 { 0.01_f32.copysign(if v == 0.0 { 1.0 } else { v }) } else { v };
                        Matrix::translate(a.x, a.y).concat(&Matrix::scale(clamp(sx), clamp(sy))).concat(&Matrix::translate(-a.x, -a.y))
                    }
                    TMode::Rotate { center: c, start_angle } => {
                        let mut ang = (p.y - c.y).atan2(p.x - c.x) - start_angle;
                        if shift {
                            let step = std::f32::consts::PI / 4.0;
                            ang = (ang / step).round() * step;
                        }
                        Matrix::translate(c.x, c.y).concat(&Matrix::rotate(ang)).concat(&Matrix::translate(-c.x, -c.y))
                    }
                };
                if let Some(inv) = applied.invert() {
                    let inc = total.concat(&inv);
                    app.ed.modify_selected_continuous(|e| e.matrix = inc.concat(&e.matrix));
                    *applied = total;
                }
            }
            Drag::Point { sel, r, started } => {
                if !*started {
                    app.ed.begin_drag();
                    *started = true;
                }
                let (sel, r) = (*sel, *r);
                if let Some(inv) = app.ed.element_at(sel).and_then(|e| e.matrix.invert()) {
                    let lp = inv.apply(p);
                    app.ed.selection = vec![sel];
                    app.ed.modify_selected_continuous(|e| {
                        if let ElementKind::Shape(sh) = &mut e.kind {
                            sh.path.set_point(r, lp);
                        }
                    });
                }
            }
            Drag::Marquee { cur, .. } => *cur = p,
            Drag::Draw { start, cur } => {
                *cur = p;
                if shift {
                    if tool_now == Tool::Line {
                        let (dx, dy) = (p.x - start.x, p.y - start.y);
                        let ang = (dy.atan2(dx) / (std::f32::consts::PI / 4.0)).round() * (std::f32::consts::PI / 4.0);
                        let len = (dx * dx + dy * dy).sqrt();
                        *cur = pt(start.x + len * ang.cos(), start.y + len * ang.sin());
                    } else {
                        let s = (p.x - start.x).abs().max((p.y - start.y).abs());
                        *cur = pt(start.x + s * (p.x - start.x).signum(), start.y + s * (p.y - start.y).signum());
                    }
                }
            }
            Drag::Freehand { pts } => {
                if pts.last().map(|l| l.dist(p) > 1.5 / zoom).unwrap_or(true) {
                    pts.push(p);
                }
            }
            Drag::Erase { .. } => {
                let removed = erase_at(app, p);
                if let Drag::Erase { removed: r, last, .. } = &mut app.drag {
                    *r += removed;
                    *last = p;
                }
            }
            Drag::None => {}
        }
    }

    let released = resp.drag_stopped() || (resp.clicked() && !matches!(app.drag, Drag::Pan { .. }));
    if released {
        if let Some(pen) = &mut app.pen {
            pen.dragging = false;
        }
        app.snap_guides = (None, None);
        let drag = std::mem::replace(&mut app.drag, Drag::None);
        match drag {
            Drag::Move { moved: false, .. } => {
                if !shift && app.ed.selection.len() > 1 {
                    if let Some(hit) = app.ed.hit_test(p) {
                        app.ed.select(Some(hit), false);
                    }
                }
            }
            Drag::Marquee { start, cur, add } => {
                if start.dist(cur) > 2.0 / app.zoom {
                    app.ed.select_in_rect(start, cur, add);
                    let n = app.ed.selection.len();
                    app.status = if n == 0 { "No objects fully inside the box (locked layers are skipped).".into() } else { format!("{n} object(s) selected") };
                }
            }
            Drag::ZoomBox { start, cur } => {
                if start.distance(cur) < 4.0 {
                    app.zoom_at(rect, cur, if alt { 0.5 } else { 2.0 });
                } else {
                    let r = Rect::from_two_pos(start, cur);
                    let factor = (rect.width() / r.width()).min(rect.height() / r.height()).clamp(0.1, 16.0);
                    let c = r.center();
                    app.zoom_at(rect, c, factor);
                    app.pan += rect.center() - (rect.min + (c - rect.min));
                }
            }
            Drag::Draw { start, cur } if start.dist(cur) > 2.0 => {
                let (x0, y0, x1, y1) = (start.x.min(cur.x), start.y.min(cur.y), start.x.max(cur.x), start.y.max(cur.y));
                let o = &app.topts;
                let (name, path, fill) = match app.tool {
                    Tool::Rect => ("Rectangle", Path::rounded_rect(x0, y0, x1 - x0, y1 - y0, o.rect_radius), app.cur_fill()),
                    Tool::Oval => ("Oval", Path::ellipse((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0), app.cur_fill()),
                    Tool::PolyStar => ("PolyStar", Path::polystar((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0, o.poly_sides, o.poly_star, o.poly_inner), app.cur_fill()),
                    _ => ("Line", Path { segs: vec![Seg::M(start), Seg::L(cur)] }, None),
                };
                let stroke = if app.tool == Tool::Line { Some(Stroke { color: from32(app.stroke), width: app.stroke_w }) } else { app.cur_stroke() };
                if fill.is_none() && stroke.is_none() {
                    app.status = "Both fill and stroke are set to none.".into();
                } else if app.ed.add_element(Element::shape(name, Shape { path, fill, stroke })).is_none() {
                    app.locked_msg();
                }
            }
            Drag::Freehand { pts } => match app.tool {
                Tool::Lasso => {
                    if pts.len() > 2 {
                        lasso_select(app, &pts, shift);
                    }
                }
                Tool::Pencil if pts.len() > 1 => {
                    let path = match app.topts.pencil_mode {
                        PencilMode::Straighten => {
                            let simple = ac_core::geom::simplify(&pts, 4.0 / app.zoom);
                            let mut p = Path::polygon(&simple);
                            p.segs.pop();
                            p
                        }
                        PencilMode::Smooth => Path::smooth_polyline(&pts, 1.5 / app.zoom),
                        PencilMode::Ink => Path::smooth_polyline(&pts, 0.3 / app.zoom),
                    };
                    let stroke = Some(Stroke { color: from32(app.stroke), width: app.stroke_w });
                    if app.ed.add_element(Element::shape("Pencil", Shape { path, fill: None, stroke })).is_none() {
                        app.locked_msg();
                    }
                }
                Tool::Brush => {
                    if let Some(path) = brush_outline(&pts, app.topts.brush_size, app.topts.brush_smooth) {
                        if app.ed.add_element(Element::shape("Brush", Shape { path, fill: Some(from32(app.fill)), stroke: None })).is_none() {
                            app.locked_msg();
                        }
                    }
                }
                _ => {}
            },
            Drag::Erase { mark, removed, .. } => {
                if removed > 0 {
                    app.ed.squash_undo(mark, "Erase");
                    app.status = format!("Erased {removed} object(s)");
                }
            }
            _ => {}
        }
    }
}

/// Eraser (object mode): delete the topmost editable object under `p`.
fn erase_at(app: &mut App, p: Pt) -> usize {
    let r = app.topts.eraser_size / 2.0;
    let probes = [p, pt(p.x + r, p.y), pt(p.x - r, p.y), pt(p.x, p.y + r), pt(p.x, p.y - r)];
    for q in probes {
        if let Some(h) = app.ed.hit_test(q) {
            if app.ed.layer_editable(h.layer) {
                app.ed.selection = vec![h];
                return app.ed.delete_selected();
            }
        }
    }
    0
}

/// Eyedropper: pick fill or stroke under the cursor and switch to the tool
/// that applies it (Animate behaviour).
fn eyedrop(app: &mut App, p: Pt) {
    let items = app.ed.evaluate_current(app.ed.frame);
    let Some(it) = items.iter().rev().find(|it| it.hit(p)) else {
        app.status = "Eyedropper: click a coloured object.".into();
        return;
    };
    let on_edge = it.stroke.is_some() && it.matrix.invert().map(|inv| it.path.hit(inv.apply(p), false, it.stroke.unwrap().width / 2.0 + 2.0 / app.zoom)).unwrap_or(false);
    if let (true, Some(st)) = (on_edge || it.fill.is_none(), it.stroke) {
        app.stroke = crate::c32(st.color);
        app.stroke_w = st.width;
        app.stroke_on = true;
        app.set_tool(Tool::InkBottle);
        app.status = format!("Picked stroke {} — Ink Bottle selected", st.color.to_hex());
    } else if let Some(f) = it.fill {
        app.fill = crate::c32(f);
        app.fill_on = true;
        app.set_tool(Tool::Bucket);
        app.status = format!("Picked fill {} — Paint Bucket selected", f.to_hex());
    }
}

fn to_ts(m: &Matrix) -> sk::Transform {
    sk::Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty)
}

fn render_stage(app: &mut App, ui: &egui::Ui, rect: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    let (pw, ph) = ((rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32);
    let Some(mut pix) = sk::Pixmap::new(pw.max(1), ph.max(1)) else { return };
    pix.fill(sk::Color::from_rgba8(0x3C, 0x3C, 0x3C, 255));
    let o = app.content_offset();
    let view = Matrix::scale(ppp, ppp).concat(&Matrix::translate(app.pan.x, app.pan.y)).concat(&Matrix::scale(app.zoom, app.zoom));
    let d = &app.ed.doc;
    let stage_dim = app.ed.editing.is_some();
    for (off, a) in [(6.0, 40u8), (3.0, 60)] {
        if let Some(r) = sk::Rect::from_xywh(off, off, d.width, d.height) {
            let mut paint = sk::Paint::default();
            paint.set_color(sk::Color::from_rgba8(0, 0, 0, a));
            pix.fill_rect(r, &paint, to_ts(&view), None);
        }
    }
    if let Some(r) = sk::Rect::from_xywh(0.0, 0.0, d.width, d.height) {
        let mut paint = sk::Paint::default();
        let bg = d.background;
        paint.set_color(sk::Color::from_rgba8(bg.r, bg.g, bg.b, if stage_dim { 90 } else { 255 }));
        pix.fill_rect(r, &paint, to_ts(&view), None);
    }
    let content_view = view.concat(&Matrix::translate(o.x, o.y));
    let n = app.ed.frame_count();
    if app.onion && !app.playing {
        for k in (1..=app.onion_n).rev() {
            let fade = 0.45 / k as f32;
            if app.ed.frame >= k {
                let items = app.ed.evaluate_current(app.ed.frame - k);
                ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: content_view, alpha: fade, tint: Some((Color::rgb(0x30, 0x60, 0xFF), 0.85)) });
            }
            if app.ed.frame + k < n {
                let items = app.ed.evaluate_current(app.ed.frame + k);
                ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: content_view, alpha: fade, tint: Some((Color::rgb(0x20, 0xC0, 0x40), 0.85)) });
            }
        }
    }
    let items = app.ed.evaluate_current(app.ed.frame);
    ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: content_view, ..Default::default() });

    let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pix.data());
    let tex = match &mut app.stage_tex {
        Some(t) => {
            t.set(img, egui::TextureOptions::NEAREST);
            t.clone()
        }
        None => {
            let t = ui.ctx().load_texture("stage", img, egui::TextureOptions::NEAREST);
            app.stage_tex = Some(t.clone());
            t
        }
    };
    ui.painter_at(rect).image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}

fn overlays(app: &App, ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter_at(rect);
    let preview = EStroke::new(1.0_f32, ACCENT);
    if app.ed.editing.is_some() {
        let c = app.to_screen(rect, pt(0.0, 0.0));
        let s = EStroke::new(1.0_f32, Color32::from_gray(20));
        painter.line_segment([c - vec2(9.0, 0.0), c + vec2(9.0, 0.0)], s);
        painter.line_segment([c - vec2(0.0, 9.0), c + vec2(0.0, 9.0)], s);
    }
    let point_tools = matches!(app.tool, Tool::Subselect | Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor);
    if point_tools {
        // Anchors (squares) and handles (dots) of selected shapes.
        for s in &app.ed.selection {
            let Some(el) = app.ed.element_at(*s) else { continue };
            let ElementKind::Shape(sh) = &el.kind else { continue };
            let m = el.matrix;
            let outline = sh.path.transformed(&m);
            for (poly, closed) in outline.flatten(12) {
                let mut pts: Vec<Pos2> = poly.iter().map(|p| app.to_screen(rect, *p)).collect();
                if closed {
                    pts.push(pts[0]);
                }
                painter.add(egui::Shape::line(pts, EStroke::new(1.0_f32, ACCENT)));
            }
            if app.tool == Tool::Subselect {
                for (_, _, h, a) in sh.path.handles() {
                    let (hs, as_) = (app.to_screen(rect, m.apply(h)), app.to_screen(rect, m.apply(a)));
                    painter.line_segment([as_, hs], EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.7)));
                    painter.circle_filled(hs, 3.0, ACCENT);
                }
            }
            for (_, a) in sh.path.anchors() {
                let c = app.to_screen(rect, m.apply(a));
                painter.rect_filled(Rect::from_center_size(c, vec2(6.0, 6.0)), 0.0, Color32::WHITE);
                painter.rect_stroke(Rect::from_center_size(c, vec2(6.0, 6.0)), 0.0, EStroke::new(1.0_f32, ACCENT));
            }
        }
    } else {
        if app.ed.selection.len() > 1 {
            for s in &app.ed.selection {
                if let Some((lo, hi)) = app.ed.bounds_of(&[*s]) {
                    let r = Rect::from_two_pos(app.to_screen(rect, lo), app.to_screen(rect, hi)).expand(1.0);
                    painter.rect_stroke(r, 0.0, EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.7)));
                }
            }
        }
        if let Some((lo, hi)) = app.ed.selection_bounds() {
            let r = Rect::from_two_pos(app.to_screen(rect, lo), app.to_screen(rect, hi)).expand(2.0);
            painter.rect_stroke(r, 0.0, EStroke::new(1.0_f32, ACCENT));
            if matches!(app.tool, Tool::Select | Tool::FreeTransform) {
                for c in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
                    painter.rect_filled(Rect::from_center_size(c, vec2(7.0, 7.0)), 0.0, Color32::WHITE);
                    painter.rect_stroke(Rect::from_center_size(c, vec2(7.0, 7.0)), 0.0, EStroke::new(1.0_f32, ACCENT));
                }
            }
            if app.tool == Tool::FreeTransform {
                painter.circle_stroke(r.center(), 4.0, EStroke::new(1.5_f32, Color32::WHITE));
            }
            if app.ed.selection.len() == 1 {
                if let Some(el) = app.ed.selected_element() {
                    if matches!(el.kind, ElementKind::Instance(_)) {
                        let reg = app.to_screen(rect, pt(el.matrix.tx, el.matrix.ty));
                        painter.circle_filled(reg, 3.5, Color32::WHITE);
                        painter.circle_stroke(reg, 3.5, EStroke::new(1.0_f32, Color32::BLACK));
                    }
                }
            }
        }
    }
    // Snap guides
    let guide = EStroke::new(1.0_f32, Color32::from_rgb(0xFF, 0x3D, 0xB5));
    if let Some(x) = app.snap_guides.0 {
        let sx = app.to_screen(rect, pt(x, 0.0)).x;
        painter.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], guide);
    }
    if let Some(y) = app.snap_guides.1 {
        let sy = app.to_screen(rect, pt(0.0, y)).y;
        painter.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], guide);
    }
    let hover = ui.input(|i| i.pointer.hover_pos());
    match &app.drag {
        Drag::Marquee { start, cur, .. } => {
            let r = Rect::from_two_pos(app.to_screen(rect, *start), app.to_screen(rect, *cur));
            painter.rect_filled(r, 0.0, ACCENT.gamma_multiply(0.12));
            painter.rect_stroke(r, 0.0, preview);
        }
        Drag::ZoomBox { start, cur } => {
            painter.rect_stroke(Rect::from_two_pos(*start, *cur), 0.0, EStroke::new(1.0_f32, Color32::WHITE));
        }
        Drag::Draw { start, cur } => {
            let (a, b) = (*start, *cur);
            let o = &app.topts;
            let path = match app.tool {
                Tool::Rect => Some(Path::rounded_rect(a.x.min(b.x), a.y.min(b.y), (b.x - a.x).abs(), (b.y - a.y).abs(), o.rect_radius)),
                Tool::Oval => Some(Path::ellipse((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (b.x - a.x).abs() / 2.0, (b.y - a.y).abs() / 2.0)),
                Tool::PolyStar => Some(Path::polystar((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (b.x - a.x).abs() / 2.0, (b.y - a.y).abs() / 2.0, o.poly_sides, o.poly_star, o.poly_inner)),
                _ => None,
            };
            match path {
                Some(path) => {
                    for (poly, closed) in path.flatten(16) {
                        let mut pts: Vec<Pos2> = poly.iter().map(|p| app.to_screen(rect, *p)).collect();
                        if closed {
                            pts.push(pts[0]);
                        }
                        painter.add(egui::Shape::line(pts, preview));
                    }
                }
                None => {
                    painter.line_segment([app.to_screen(rect, a), app.to_screen(rect, b)], EStroke::new(app.stroke_w * app.zoom, app.stroke));
                }
            }
        }
        Drag::Freehand { pts } => {
            let s: Vec<Pos2> = pts.iter().map(|p| app.to_screen(rect, *p)).collect();
            let st = match app.tool {
                Tool::Brush => EStroke::new(app.topts.brush_size * app.zoom, app.fill),
                Tool::Lasso => EStroke::new(1.0_f32, ACCENT),
                _ => EStroke::new(app.stroke_w * app.zoom, app.stroke),
            };
            painter.add(egui::Shape::line(s, st));
        }
        _ => {}
    }
    // Pen path in progress
    if let Some(pen) = &app.pen {
        let mut prev: Option<(Pt, Pt)> = None;
        for &(a, h) in &pen.pts {
            if let Some((pa, ph)) = prev {
                let samples: Vec<Pos2> = (0..=16)
                    .map(|i| {
                        let t = i as f32 / 16.0;
                        let (c1, c2) = (pt(pa.x + ph.x, pa.y + ph.y), pt(a.x - h.x, a.y - h.y));
                        let u = 1.0 - t;
                        let q = pt(
                            u * u * u * pa.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * a.x,
                            u * u * u * pa.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * a.y,
                        );
                        app.to_screen(rect, q)
                    })
                    .collect();
                painter.add(egui::Shape::line(samples, EStroke::new(1.5_f32, ACCENT)));
            }
            let c = app.to_screen(rect, a);
            if h.x.abs() + h.y.abs() > 0.01 {
                let h1 = app.to_screen(rect, pt(a.x + h.x, a.y + h.y));
                let h2 = app.to_screen(rect, pt(a.x - h.x, a.y - h.y));
                painter.line_segment([h1, h2], EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.7)));
                painter.circle_filled(h1, 2.5, ACCENT);
                painter.circle_filled(h2, 2.5, ACCENT);
            }
            painter.rect_filled(Rect::from_center_size(c, vec2(6.0, 6.0)), 0.0, Color32::WHITE);
            painter.rect_stroke(Rect::from_center_size(c, vec2(6.0, 6.0)), 0.0, EStroke::new(1.0_f32, ACCENT));
            prev = Some((a, h));
        }
        if let (Some(&(last, _)), Some(hp), false) = (pen.pts.last(), hover, pen.dragging) {
            painter.line_segment([app.to_screen(rect, last), hp], EStroke::new(1.0_f32, ACCENT.gamma_multiply(0.5)));
            if pen.pts.len() >= 2 && app.to_screen(rect, pen.pts[0].0).distance(hp) < 8.0 {
                painter.circle_stroke(app.to_screen(rect, pen.pts[0].0), 6.0, EStroke::new(1.5_f32, Color32::WHITE));
            }
        }
    }
    // Polygon lasso in progress
    if !app.poly_lasso.is_empty() {
        let mut pts: Vec<Pos2> = app.poly_lasso.iter().map(|p| app.to_screen(rect, *p)).collect();
        if let Some(h) = hover {
            pts.push(h);
        }
        painter.add(egui::Shape::line(pts, EStroke::new(1.0_f32, ACCENT)));
    }
    // Eraser cursor
    if app.tool == Tool::Eraser {
        if let Some(h) = hover.filter(|h| rect.contains(*h)) {
            painter.circle_stroke(h, app.topts.eraser_size / 2.0 * app.zoom, EStroke::new(1.0_f32, Color32::WHITE));
        }
    }
    if app.lib_drag.is_some() && ui.rect_contains_pointer(rect) {
        painter.rect_stroke(rect.shrink(2.0), 0.0, EStroke::new(2.0_f32, ACCENT));
    }
}

/// Inline editor for the Text tool. The vector text updates live as you type.
fn text_editor(app: &mut App, ui: &mut egui::Ui, rect: Rect) {
    let Some(te) = &app.text_edit else { return };
    let sel = te.sel;
    let Some(el) = app.ed.element_at(sel) else {
        app.text_edit = None;
        return;
    };
    let anchor = app.to_screen(rect, pt(el.matrix.tx, el.matrix.ty));
    let (lo, hi) = app.ed.bounds_of(&[sel]).unwrap_or((pt(el.matrix.tx, el.matrix.ty), pt(el.matrix.tx + 10.0, el.matrix.ty + 10.0)));
    let box_r = Rect::from_two_pos(app.to_screen(rect, lo), app.to_screen(rect, hi)).expand(3.0);
    ui.painter_at(rect).rect_stroke(box_r, 0.0, EStroke::new(1.0_f32, ACCENT));
    let mut buffer = te.buffer.clone();
    let focus_requested = te.focus_requested;
    let mark = te.mark;
    let mut done = false;
    let mut got_focus = false;
    egui::Area::new(egui::Id::new("text-tool-editor")).order(egui::Order::Foreground).fixed_pos(pos2(anchor.x, box_r.bottom() + 6.0)).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new("Type text · Esc or click away to finish").small().color(Color32::from_gray(160)));
            let r = ui.add(egui::TextEdit::multiline(&mut buffer).desired_rows(2).desired_width(260.0));
            // Keep asking until the box really has focus (the click that
            // created it can steal focus back in the same frame).
            if !focus_requested && !r.has_focus() {
                r.request_focus();
            }
            got_focus = r.has_focus();
            if r.changed() {
                let text = buffer.clone();
                if app.ed.undo_depth() == mark {
                    app.ed.begin_drag(); // first keystroke when editing existing text
                }
                app.ed.selection = vec![sel];
                app.ed.modify_selected_continuous(|e| {
                    if let ElementKind::Text(t) = &mut e.kind {
                        t.text = text.clone();
                    }
                });
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) || ui.button("Done").clicked() {
                done = true;
            }
        });
    });
    if let Some(te) = &mut app.text_edit {
        te.buffer = buffer;
        te.focus_requested |= got_focus;
    }
    if done {
        app.finish_text();
    }
}
