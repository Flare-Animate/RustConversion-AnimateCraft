//! Stage: rendering, tool input, selection overlays and the stage context menu.

use crate::{from32, App, Tool, ACCENT};
use ac_core::{pt, Color, Element, ElementKind, Matrix, Path, Pt, Seg, Shape, Stroke};
use eframe::egui::{self, pos2, vec2, Color32, Key, Pos2, Rect, Sense, Stroke as EStroke, Vec2};
use tiny_skia as sk;

pub enum Drag {
    None,
    /// Moving the selection; `last` in doc coords.
    Move { last: Pt, moved: bool },
    /// Scale/rotate the selection: `total` is the transform applied so far.
    Transform { mode: TMode, applied: Matrix, started: bool },
    Marquee { start: Pt, cur: Pt, add: bool },
    Draw { start: Pt, cur: Pt },
    Pencil { pts: Vec<Pt> },
    Pan { last: Pos2 },
}

#[derive(Clone, Copy)]
pub enum TMode {
    Scale { anchor: Pt, start: Pt },
    Rotate { center: Pt, start_angle: f32 },
}

impl App {
    fn content_offset(&self) -> Vec2 {
        // Inside a symbol, its registration point sits at the stage centre.
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
    fn zoom_at(&mut self, rect: Rect, anchor: Pos2, factor: f32) {
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

    /// Which selection handle is under `pos`: (corner index, rotate zone).
    /// Corners: 0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right.
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
}

pub fn stage(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_gray(0x3C))).show(ctx, |ui| {
        // Edit bar (breadcrumb), like Animate's scene/symbol bar.
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
        // Clicking the stage takes keyboard focus away from any text field,
        // so shortcuts (Delete, F6, …) work immediately.
        if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
            if let Some(id) = ui.ctx().memory(|m| m.focused()) {
                ui.ctx().memory_mut(|m| m.surrender_focus(id));
            }
        }
        stage_input(app, ui, rect, &resp);
        resp.context_menu(|ui| app.object_menu(ui));
        render_stage(app, ui, rect);
        overlays(app, ui, rect);
    });
}

fn stage_input(app: &mut App, ui: &egui::Ui, rect: Rect, resp: &egui::Response) {
    if let Some(hover) = resp.hover_pos() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            app.zoom_at(rect, hover, if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 });
        }
    }
    let (space, shift) = ui.input(|i| (i.key_down(Key::Space), i.modifiers.shift));
    let tool = if space { Tool::Hand } else { app.tool };
    let Some(pos) = resp.interact_pointer_pos() else { return };
    let p = app.to_doc(rect, pos);
    // A drag is only recognised after the pointer has moved a few pixels;
    // start every gesture where the button actually went down.
    let origin = if resp.drag_started() { ui.input(|i| i.pointer.press_origin()).unwrap_or(pos) } else { pos };
    let p0 = app.to_doc(rect, origin);

    // Right-click selects what's under the cursor (unless it's already selected) before the menu opens.
    if resp.secondary_clicked() {
        let hit = app.ed.hit_test(p);
        if hit.map(|h| !app.ed.is_selected(h)).unwrap_or(false) {
            app.ed.select(hit, false);
        }
        return;
    }
    // Double-click a symbol instance with a selection tool: edit it (Animate behaviour).
    if resp.double_clicked() && matches!(tool, Tool::Select | Tool::FreeTransform) {
        if let Some(h) = app.ed.hit_test(p) {
            if let Some(ElementKind::Instance(i)) = app.ed.element_at(h).map(|e| e.kind) {
                app.edit_symbol(i.symbol);
                return;
            }
        }
    }

    let pressed = resp.drag_started() || (resp.clicked() && matches!(app.drag, Drag::None));
    if pressed {
        app.playing = false;
        app.ed.frame_sel = None; // working on the stage: Delete etc. now target objects
        match tool {
            Tool::Hand => app.drag = Drag::Pan { last: origin },
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
                        app.drag = Drag::Move { last: p0, moved: false };
                    }
                } else {
                    if !shift {
                        app.ed.select(None, false);
                    }
                    app.drag = Drag::Marquee { start: p0, cur: p0, add: shift };
                }
            }
            Tool::Rect | Tool::Oval | Tool::Line => app.drag = Drag::Draw { start: p0, cur: p0 },
            Tool::Pencil => app.drag = Drag::Pencil { pts: vec![p0] },
            Tool::Bucket => {
                match app.ed.hit_test(p0) {
                    Some(s) => app.ed.paint_bucket(s, from32(app.fill)),
                    None => app.status = "Paint Bucket: click inside a shape.".into(),
                }
            }
        }
    }

    if resp.dragged() {
        let zoom = app.zoom;
        let tool_now = app.tool;
        match &mut app.drag {
            Drag::Pan { last } => {
                app.pan += pos - *last;
                *last = pos;
            }
            Drag::Move { last, moved } => {
                let d = pt(p.x - last.x, p.y - last.y);
                *last = p;
                if !*moved {
                    app.ed.begin_drag(); // one undo step per drag
                    *moved = true;
                }
                app.ed.modify_selected_continuous(|e| {
                    e.matrix.tx += d.x;
                    e.matrix.ty += d.y;
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
            Drag::Marquee { cur, .. } => *cur = p,
            Drag::Draw { start, cur } => {
                *cur = p;
                if shift && tool_now != Tool::Line {
                    let s = (p.x - start.x).abs().max((p.y - start.y).abs());
                    *cur = pt(start.x + s * (p.x - start.x).signum(), start.y + s * (p.y - start.y).signum());
                }
            }
            Drag::Pencil { pts } => {
                if pts.last().map(|l| l.dist(p) > 1.5 / zoom).unwrap_or(true) {
                    pts.push(p);
                }
            }
            Drag::None => {}
        }
    }

    let released = resp.drag_stopped() || (resp.clicked() && !matches!(app.drag, Drag::Pan { .. }));
    if released {
        let drag = std::mem::replace(&mut app.drag, Drag::None);
        match drag {
            Drag::Move { moved: false, .. } => {
                // A plain click inside a multi-selection narrows it to that object.
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
            Drag::Draw { start, cur } if start.dist(cur) > 2.0 => {
                let (x0, y0, x1, y1) = (start.x.min(cur.x), start.y.min(cur.y), start.x.max(cur.x), start.y.max(cur.y));
                let (name, path, fill) = match app.tool {
                    Tool::Rect => ("Rectangle", Path::rect(x0, y0, x1 - x0, y1 - y0), app.cur_fill()),
                    Tool::Oval => ("Oval", Path::ellipse((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0), app.cur_fill()),
                    _ => ("Line", Path { segs: vec![Seg::M(start), Seg::L(cur)] }, None),
                };
                let stroke = if app.tool == Tool::Line { Some(Stroke { color: from32(app.stroke), width: app.stroke_w }) } else { app.cur_stroke() };
                if fill.is_none() && stroke.is_none() {
                    app.status = "Both fill and stroke are off.".into();
                } else if app.ed.add_element(Element::shape(name, Shape { path, fill, stroke })).is_none() {
                    app.status = "The current layer is locked or hidden. Pick another layer in the timeline.".into();
                }
            }
            Drag::Pencil { pts } if pts.len() > 1 => {
                let path = Path::smooth_polyline(&pts, 1.2 / app.zoom);
                let stroke = Some(Stroke { color: from32(app.stroke), width: app.stroke_w });
                if app.ed.add_element(Element::shape("Pencil", Shape { path, fill: None, stroke })).is_none() {
                    app.status = "The current layer is locked or hidden. Pick another layer in the timeline.".into();
                }
            }
            _ => {}
        }
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
    if app.ed.editing.is_some() {
        let c = app.to_screen(rect, pt(0.0, 0.0));
        let s = EStroke::new(1.0_f32, Color32::from_gray(20));
        painter.line_segment([c - vec2(9.0, 0.0), c + vec2(9.0, 0.0)], s);
        painter.line_segment([c - vec2(0.0, 9.0), c + vec2(0.0, 9.0)], s);
    }
    // Each selected object gets a thin box; the whole selection gets handles.
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
        for c in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
            painter.rect_filled(Rect::from_center_size(c, vec2(7.0, 7.0)), 0.0, Color32::WHITE);
            painter.rect_stroke(Rect::from_center_size(c, vec2(7.0, 7.0)), 0.0, EStroke::new(1.0_f32, ACCENT));
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
    let preview = EStroke::new(1.0_f32, ACCENT);
    match &app.drag {
        Drag::Marquee { start, cur, .. } => {
            let r = Rect::from_two_pos(app.to_screen(rect, *start), app.to_screen(rect, *cur));
            painter.rect_filled(r, 0.0, ACCENT.gamma_multiply(0.12));
            painter.rect_stroke(r, 0.0, preview);
        }
        Drag::Draw { start, cur } => {
            let (a, b) = (app.to_screen(rect, *start), app.to_screen(rect, *cur));
            match app.tool {
                Tool::Rect => {
                    painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, preview);
                }
                Tool::Oval => {
                    let r = Rect::from_two_pos(a, b);
                    let pts: Vec<Pos2> = (0..=48)
                        .map(|i| {
                            let t = i as f32 / 48.0 * std::f32::consts::TAU;
                            r.center() + vec2(t.cos() * r.width() / 2.0, t.sin() * r.height() / 2.0)
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, preview));
                }
                _ => {
                    painter.line_segment([a, b], EStroke::new(app.stroke_w * app.zoom, app.stroke));
                }
            }
        }
        Drag::Pencil { pts } => {
            let s: Vec<Pos2> = pts.iter().map(|p| app.to_screen(rect, *p)).collect();
            painter.add(egui::Shape::line(s, EStroke::new(app.stroke_w * app.zoom, app.stroke)));
        }
        _ => {}
    }
    // Drop target highlight while dragging a library item.
    if app.lib_drag.is_some() && ui.rect_contains_pointer(rect) {
        painter.rect_stroke(rect.shrink(2.0), 0.0, EStroke::new(2.0_f32, ACCENT));
    }
}
