//! Timeline panel: layer column + frame grid, styled after Animate's timeline
//! (keyframe dots, purple classic-tween spans, green shape-tween spans, red
//! playhead, blue frame selection).

use crate::{c32, App, ACCENT};
use ac_core::{FrameSel, LayerFlag, Tween};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Sense, Stroke};

#[derive(Default)]
pub struct TimelineState {
    /// First visible frame column.
    pub scroll: f32,
    /// First visible layer row.
    pub vscroll: usize,
    pub renaming: Option<(usize, String)>,
    /// Cell where the current mouse gesture started.
    anchor: Option<(usize, u32)>,
    gesture: Gesture,
    /// Layer being dragged in the layer column.
    layer_drag: Option<usize>,
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
enum Gesture {
    #[default]
    None,
    Select,
    /// Dragging selected frames; value = current frame offset.
    Move(i64),
}

const LW: f32 = 230.0; // layer column width
const HH: f32 = 22.0; // header height
const RH: f32 = 24.0; // row height
const CW: f32 = 11.0; // frame cell width

const CLASSIC: Color32 = Color32::from_rgb(0x6E, 0x5B, 0xB0);
const SHAPE: Color32 = Color32::from_rgb(0x4E, 0x8F, 0x57);
const PLAYHEAD: Color32 = Color32::from_rgb(0xE5, 0x3B, 0x3B);

/// Frame commands (Insert menu, Edit ▸ Timeline, and the right-click menu).
pub fn frame_menu(app: &mut App, ui: &mut egui::Ui) {
    let item = |ui: &mut egui::Ui, label: &str, sc: &str| ui.add(egui::Button::new(label).shortcut_text(sc)).clicked();
    if item(ui, "Insert Frame", "F5") { app.ed.insert_frame(); ui.close_menu(); }
    if item(ui, "Remove Frames", "Shift+F5") { app.ed.remove_frame(); ui.close_menu(); }
    if item(ui, "Insert Keyframe", "F6") { app.ed.insert_keyframe(); ui.close_menu(); }
    if item(ui, "Insert Blank Keyframe", "F7") { app.ed.insert_blank_keyframe(); ui.close_menu(); }
    if item(ui, "Clear Keyframe", "Shift+F6") { app.ed.clear_keyframe(); ui.close_menu(); }
    if item(ui, "Clear Frames", "Del") { app.ed.clear_frames(); ui.close_menu(); }
    ui.separator();
    if item(ui, "Create Classic Tween", "") { app.ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 }); ui.close_menu(); }
    if item(ui, "Create Shape Tween", "") { app.ed.set_tween(Tween::Shape { ease: 0.0 }); ui.close_menu(); }
    if item(ui, "Remove Tween", "") { app.ed.set_tween(Tween::None); ui.close_menu(); }
    ui.separator();
    if item(ui, "Cut Frames", "") { app.ed.cut_frames(); ui.close_menu(); }
    if item(ui, "Copy Frames", "") { app.ed.copy_frames(); ui.close_menu(); }
    if ui.add_enabled(app.ed.can_paste_frames(), egui::Button::new("Paste Frames")).clicked() { app.ed.paste_frames(); ui.close_menu(); }
    if item(ui, "Select All Frames", "") {
        let n = app.ed.timeline().layers.len();
        let f = app.ed.frame_count();
        app.ed.select_frames(FrameSel { layer0: 0, layer1: n.saturating_sub(1), f0: 0, f1: f.saturating_sub(1) });
        ui.close_menu();
    }
}

pub fn panel(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("timeline").resizable(true).default_height(230.0).min_height(120.0).show(ctx, |ui| {
        toolbar(app, ui);
        grid(app, ui);
    });
}

fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Timeline").strong());
        ui.separator();
        if ui.button("⏮").on_hover_text("First frame").clicked() { app.step(-100_000); }
        if ui.button("◀").on_hover_text("Step back (,)").clicked() { app.step(-1); }
        if ui.button(if app.playing { "⏸" } else { "▶" }).on_hover_text("Play / stop (Enter)").clicked() { app.toggle_play(); }
        if ui.button("▶|").on_hover_text("Step forward (.)").clicked() { app.step(1); }
        if ui.button("⏭").on_hover_text("Last frame").clicked() { app.step(100_000); }
        ui.toggle_value(&mut app.loop_play, "🔁").on_hover_text("Loop playback");
        ui.separator();
        ui.toggle_value(&mut app.onion, "Onion skin").on_hover_text("Show previous (blue) and next (green) frames");
        if app.onion {
            ui.add(egui::DragValue::new(&mut app.onion_n).range(1..=10).prefix("±"));
        }
        ui.separator();
        let n = app.ed.frame_count();
        ui.label(egui::RichText::new(format!("{}", app.ed.frame + 1)).monospace().color(ACCENT).size(15.0));
        ui.label(egui::RichText::new(format!("/ {n}")).monospace());
        ui.label(egui::RichText::new(format!("{:.2} fps", app.ed.doc.fps)).monospace().color(Color32::from_gray(160)));
        ui.label(egui::RichText::new(format!("{:.1}s", app.ed.frame as f32 / app.ed.doc.fps)).monospace().color(Color32::from_gray(160)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("🗑").on_hover_text("Delete layer").clicked() { app.ed.delete_layer(); }
            if ui.button("➕ Layer").on_hover_text("New layer").clicked() { app.ed.add_layer(); }
            ui.separator();
            if ui.button("Clear").on_hover_text("Clear selected frames (Delete)").clicked() { app.ed.clear_frames(); }
            if ui.button("F7").on_hover_text("Insert blank keyframe").clicked() { app.ed.insert_blank_keyframe(); }
            if ui.button("F6").on_hover_text("Insert keyframe").clicked() { app.ed.insert_keyframe(); }
            if ui.button("F5").on_hover_text("Insert frame").clicked() { app.ed.insert_frame(); }
        });
    });
}

fn grid(app: &mut App, ui: &mut egui::Ui) {
    let avail = ui.available_size();
    let (rect, _) = ui.allocate_exact_size(avail, Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_gray(0x28));

    let grid_x0 = rect.left() + LW;
    let grid_w = (rect.right() - grid_x0).max(0.0);
    let visible_cols = (grid_w / CW).ceil() as u32 + 1;
    let rows_top = rect.top() + HH;
    let visible_rows = (((rect.bottom() - rows_top) / RH).floor() as usize).max(1);
    let n = app.ed.frame_count();
    let layers = app.ed.timeline().layers.clone();
    let nl = layers.len();

    // Follow the playhead while playing; wheel scrolls (horizontal over frames, vertical over layers).
    if app.playing {
        let f = app.ed.frame as f32;
        if f < app.tl.scroll || f >= app.tl.scroll + visible_cols as f32 - 2.0 {
            app.tl.scroll = (f - 2.0).max(0.0);
        }
    }
    let grid_rect = Rect::from_min_max(pos2(grid_x0, rect.top()), rect.max);
    let layer_col = Rect::from_min_max(pos2(rect.left(), rows_top), pos2(grid_x0, rect.bottom()));
    let d = ui.input(|i| i.raw_scroll_delta);
    if ui.rect_contains_pointer(grid_rect) && (d.x != 0.0 || d.y != 0.0) {
        let delta = if d.x.abs() > d.y.abs() { -d.x } else { -d.y };
        app.tl.scroll = (app.tl.scroll + delta / CW).clamp(0.0, n as f32 + 200.0);
    }
    if ui.rect_contains_pointer(layer_col) && d.y != 0.0 {
        let max = nl.saturating_sub(visible_rows);
        app.tl.vscroll = if d.y < 0.0 { (app.tl.vscroll + 1).min(max) } else { app.tl.vscroll.saturating_sub(1) };
    }
    app.tl.vscroll = app.tl.vscroll.min(nl.saturating_sub(1));
    let scroll = app.tl.scroll;
    let vscroll = app.tl.vscroll;
    let first = scroll.floor() as u32;
    let x_of = |f: u32| grid_x0 + (f as f32 - scroll) * CW;
    let row_y = |i: usize| rows_top + (i as f32 - vscroll as f32) * RH;
    let cell_at = |pos: egui::Pos2| -> (usize, u32) {
        let f = (((pos.x - grid_x0) / CW) + scroll).floor().max(0.0) as u32;
        let r = (((pos.y - rows_top) / RH).floor().max(0.0) as usize + vscroll).min(nl.saturating_sub(1));
        (r, f)
    };

    // --- header (scrub)
    let header = Rect::from_min_size(pos2(grid_x0, rect.top()), vec2(grid_w, HH));
    p.rect_filled(header, 0.0, Color32::from_gray(0x33));
    p.rect_filled(Rect::from_min_size(rect.min, vec2(LW, HH)), 0.0, Color32::from_gray(0x33));
    p.text(rect.min + vec2(8.0, HH / 2.0), Align2::LEFT_CENTER, "Layers", FontId::proportional(12.0), Color32::from_gray(160));
    for (k, label) in ["👁", "🔒", "□"].iter().enumerate() {
        p.text(pos2(rect.left() + LW - 70.0 + k as f32 * 22.0, rect.top() + HH / 2.0), Align2::CENTER_CENTER, *label, FontId::proportional(11.0), Color32::from_gray(140));
    }
    for f in first..first + visible_cols {
        let x = x_of(f);
        let tick = if (f + 1) % 5 == 0 || f == 0 { 6.0 } else { 3.0 };
        p.line_segment([pos2(x, header.bottom() - tick), pos2(x, header.bottom())], Stroke::new(1.0_f32, Color32::from_gray(110)));
        if f == 0 || (f + 1) % 5 == 0 {
            p.text(pos2(x + 1.0, header.top() + 3.0), Align2::LEFT_TOP, format!("{}", f + 1), FontId::monospace(10.0), Color32::from_gray(175));
        }
    }
    let hresp = ui.interact(header, ui.id().with("tl-header"), Sense::click_and_drag());
    if let Some(pos) = hresp.interact_pointer_pos() {
        if hresp.clicked() || hresp.dragged() {
            app.playing = false;
            let (_, f) = cell_at(pos);
            app.ed.set_frame(f.min(n + 200));
            app.ed.frame_sel = None;
        }
    }

    // --- frame cells: one interaction for the whole area (selection, drag-move, context menu)
    let cells = Rect::from_min_max(pos2(grid_x0, rows_top), pos2(rect.right(), (rows_top + (nl - vscroll) as f32 * RH).min(rect.bottom())));
    let cresp = ui.interact(cells, ui.id().with("tl-cells"), Sense::click_and_drag());
    let shift = ui.input(|i| i.modifiers.shift);
    if cresp.hovered() || cresp.dragged() {
        // Clicking the timeline takes keyboard focus away from text fields.
        if cresp.clicked() || cresp.drag_started() || cresp.secondary_clicked() {
            if let Some(id) = ui.ctx().memory(|m| m.focused()) {
                ui.ctx().memory_mut(|m| m.surrender_focus(id));
            }
        }
    }
    if let Some(pos) = cresp.interact_pointer_pos() {
        let (l, f) = cell_at(pos);
        let pressed = cresp.drag_started() || cresp.clicked() || cresp.secondary_clicked();
        if std::env::var("AC_DEBUG").is_ok() && (pressed || cresp.dragged()) {
            eprintln!("tl: pressed={pressed} drag_started={} clicked={} dbl={} dragged={} gesture={:?} sel={:?} cell={:?} origin={:?}", cresp.drag_started(), cresp.clicked(), cresp.double_clicked(), cresp.dragged(), app.tl.gesture, app.ed.frame_sel, (l, f), ui.input(|i| i.pointer.press_origin()));
        }
        if pressed && app.tl.gesture == Gesture::None {
            // Gestures start where the button went down, not where the drag was detected.
            let (l, f) = ui.input(|i| i.pointer.press_origin()).filter(|_| cresp.drag_started()).map(cell_at).unwrap_or((l, f));
            app.playing = false;
            let inside = app.ed.frame_sel.map(|s| s.contains(l, f)).unwrap_or(false);
            if cresp.secondary_clicked() {
                if !inside {
                    app.ed.select_frames(FrameSel::cell(l, f));
                }
            } else if cresp.double_clicked() {
                app.ed.select_span(l, f);
            } else if shift {
                let a = app.tl.anchor.unwrap_or((app.ed.layer, app.ed.frame));
                app.ed.select_frames(FrameSel::between(a, (l, f)));
                app.ed.frame = f;
            } else if inside && cresp.drag_started() {
                app.tl.anchor = Some((l, f));
                app.tl.gesture = Gesture::Move(0);
            } else {
                app.tl.anchor = Some((l, f));
                app.ed.select_frames(FrameSel::cell(l, f));
                app.tl.gesture = if cresp.drag_started() { Gesture::Select } else { Gesture::None };
            }
        }
        if cresp.dragged() {
            match app.tl.gesture {
                Gesture::Select => {
                    if let Some(a) = app.tl.anchor {
                        let sel = FrameSel::between(a, (l, f));
                        app.ed.frame_sel = Some(sel);
                        app.ed.layer = sel.layer0;
                        app.ed.frame = f;
                    }
                }
                Gesture::Move(_) => {
                    if let Some(a) = app.tl.anchor {
                        app.tl.gesture = Gesture::Move(f as i64 - a.1 as i64);
                    }
                }
                Gesture::None => {}
            }
        }
    }
    if cresp.drag_stopped() {
        if let Gesture::Move(delta) = app.tl.gesture {
            if delta != 0 {
                app.ed.move_frames(delta);
                app.status = format!("Moved frames by {delta}");
            }
        }
        app.tl.gesture = Gesture::None;
    }
    cresp.context_menu(|ui| frame_menu(app, ui));

    // --- rows
    let move_delta = if let Gesture::Move(d) = app.tl.gesture { d } else { 0 };
    let sel = app.ed.frame_sel;
    for (i, layer) in layers.iter().enumerate().skip(vscroll) {
        let y = row_y(i);
        if y > rect.bottom() {
            break;
        }
        let row_left = Rect::from_min_size(pos2(rect.left(), y), vec2(LW, RH));
        let selected = i == app.ed.layer;
        p.rect_filled(row_left, 0.0, if selected { Color32::from_rgb(0x2B, 0x4F, 0x7A) } else { Color32::from_gray(0x30) });
        p.line_segment([pos2(rect.left(), y + RH), pos2(rect.right(), y + RH)], Stroke::new(1.0_f32, Color32::from_gray(0x22)));

        // Name: click = select layer (+ its frames), double-click = rename, drag = reorder.
        let name_rect = Rect::from_min_size(pos2(rect.left() + 8.0, y), vec2(LW - 90.0, RH));
        let renaming = matches!(&app.tl.renaming, Some((ri, _)) if *ri == i);
        if renaming {
            let (_, buf) = app.tl.renaming.as_mut().unwrap();
            let r = ui.put(name_rect.shrink(2.0), egui::TextEdit::singleline(buf));
            r.request_focus();
            if r.lost_focus() {
                let (_, name) = app.tl.renaming.take().unwrap();
                if !name.trim().is_empty() {
                    app.ed.rename_layer(i, name.trim());
                }
            }
        } else {
            let lock_note = if layer.locked { "  🔒" } else { "" };
            p.text(name_rect.left_center(), Align2::LEFT_CENTER, format!("{}{lock_note}", layer.name), FontId::proportional(13.0), if layer.visible { Color32::from_gray(225) } else { Color32::from_gray(120) });
            let r = ui.interact(name_rect, ui.id().with(("ln", i)), Sense::click_and_drag());
            if r.clicked() {
                app.ed.select_layer_frames(i);
            }
            if r.double_clicked() {
                app.tl.renaming = Some((i, layer.name.clone()));
            }
            if r.drag_started() {
                app.tl.layer_drag = Some(i);
            }
            if r.drag_stopped() {
                if let (Some(from), Some(pos)) = (app.tl.layer_drag.take(), ui.input(|inp| inp.pointer.latest_pos())) {
                    let to = (((pos.y - rows_top) / RH).floor().max(0.0) as usize + vscroll).min(nl - 1);
                    app.ed.move_layer(from, to);
                }
            }
            r.on_hover_text("Click: select layer · double-click: rename · drag: reorder");
        }
        // Eye / lock / outline toggles
        let icon = |k: usize| Rect::from_center_size(pos2(rect.left() + LW - 70.0 + k as f32 * 22.0, y + RH / 2.0), vec2(18.0, 18.0));
        let eye = icon(0);
        if layer.visible {
            p.circle_stroke(eye.center(), 5.0, Stroke::new(1.3_f32, Color32::from_gray(200)));
            p.circle_filled(eye.center(), 2.2, Color32::from_gray(200));
        } else {
            p.line_segment([eye.center() - vec2(5.0, 0.0), eye.center() + vec2(5.0, 0.0)], Stroke::new(1.6_f32, Color32::from_rgb(0xE5, 0x6B, 0x3B)));
        }
        let lock = icon(1);
        if layer.locked {
            p.rect_filled(Rect::from_center_size(lock.center() + vec2(0.0, 2.0), vec2(9.0, 7.0)), 1.0, Color32::from_gray(210));
            p.circle_stroke(lock.center() - vec2(0.0, 2.0), 3.0, Stroke::new(1.3_f32, Color32::from_gray(210)));
        } else {
            p.circle_filled(lock.center(), 1.6, Color32::from_gray(110));
        }
        let ol = icon(2);
        let sq = Rect::from_center_size(ol.center(), vec2(10.0, 10.0));
        if layer.outline {
            p.rect_stroke(sq, 0.0, Stroke::new(1.5_f32, c32(layer.color)));
        } else {
            p.rect_filled(sq, 0.0, c32(layer.color));
        }
        for (k, flag, tip) in [(0, LayerFlag::Visible, "Show / hide"), (1, LayerFlag::Locked, "Lock / unlock"), (2, LayerFlag::Outline, "Outline mode")] {
            if ui.interact(icon(k), ui.id().with(("lf", i, k)), Sense::click()).on_hover_text(tip).clicked() {
                app.ed.toggle_layer_flag(i, flag);
            }
        }

        // Frame cells
        let row = Rect::from_min_max(pos2(grid_x0, y), pos2(rect.right(), y + RH));
        let rp = ui.painter_at(row.intersect(rect));
        for f in first..first + visible_cols {
            let x = x_of(f);
            let cell = Rect::from_min_size(pos2(x, y), vec2(CW, RH));
            let in_span = f < layer.end;
            let fill = if in_span {
                if (f + 1) % 5 == 0 { Color32::from_gray(0x52) } else { Color32::from_gray(0x47) }
            } else if (f + 1) % 5 == 0 {
                Color32::from_gray(0x30)
            } else {
                Color32::from_gray(0x2A)
            };
            rp.rect_filled(cell, 0.0, fill);
            rp.line_segment([pos2(x + CW, y), pos2(x + CW, y + RH)], Stroke::new(1.0_f32, Color32::from_gray(0x24)));
        }
        for (&start, kf) in &layer.keyframes {
            let end = layer.span_end(start);
            let (x0, x1) = (x_of(start), x_of(end));
            let span = Rect::from_min_max(pos2(x0, y + 1.0), pos2(x1, y + RH - 1.0));
            let has_next = layer.keyframes.contains_key(&end);
            match kf.tween {
                Tween::Classic { .. } | Tween::Shape { .. } => {
                    let col = if matches!(kf.tween, Tween::Classic { .. }) { CLASSIC } else { SHAPE };
                    rp.rect_filled(span, 0.0, col);
                    let ym = y + RH / 2.0;
                    let stroke = Stroke::new(1.0_f32, Color32::from_gray(20));
                    if has_next {
                        rp.line_segment([pos2(x0 + CW, ym), pos2(x1 - 3.0, ym)], stroke);
                        rp.line_segment([pos2(x1 - 6.0, ym - 3.0), pos2(x1 - 2.0, ym)], stroke);
                        rp.line_segment([pos2(x1 - 6.0, ym + 3.0), pos2(x1 - 2.0, ym)], stroke);
                    } else {
                        let mut x = x0 + CW;
                        while x < x1 - 4.0 {
                            rp.line_segment([pos2(x, ym), pos2((x + 3.0).min(x1), ym)], stroke);
                            x += 6.0;
                        }
                    }
                }
                Tween::None => {
                    if end > start + 1 {
                        let r = Rect::from_center_size(pos2(x1 - CW / 2.0, y + RH - 7.0), vec2(5.0, 7.0));
                        rp.rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_gray(30)));
                    }
                }
            }
            let c = pos2(x0 + CW / 2.0, y + RH - 7.0);
            if kf.elements.is_empty() {
                rp.circle_stroke(c, 3.0, Stroke::new(1.2_f32, Color32::from_gray(20)));
            } else {
                rp.circle_filled(c, 3.3, Color32::from_gray(20));
            }
            if let Some(label) = &kf.label {
                rp.text(pos2(x0 + 2.0, y + 2.0), Align2::LEFT_TOP, format!("⚑{label}"), FontId::proportional(10.0), Color32::from_rgb(0xFF, 0x60, 0x60));
            }
        }
        // Frame selection highlight (and its ghost while dragging).
        if let Some(s) = sel {
            if (s.layer0..=s.layer1).contains(&i) {
                let r = Rect::from_min_max(pos2(x_of(s.f0), y), pos2(x_of(s.f1 + 1), y + RH));
                rp.rect_filled(r, 0.0, ACCENT.gamma_multiply(0.45));
                rp.rect_stroke(r, 0.0, Stroke::new(1.0_f32, ACCENT));
                if move_delta != 0 {
                    let g = r.translate(vec2(move_delta as f32 * CW, 0.0));
                    rp.rect_stroke(g, 0.0, Stroke::new(2.0_f32, Color32::WHITE));
                }
            }
        }
    }
    // Layer reorder indicator
    if let (Some(_), Some(pos)) = (app.tl.layer_drag, ui.input(|i| i.pointer.latest_pos())) {
        let to = (((pos.y - rows_top) / RH).floor().max(0.0) as usize + vscroll).min(nl - 1);
        let y = row_y(to);
        p.line_segment([pos2(rect.left(), y), pos2(grid_x0, y)], Stroke::new(2.0_f32, ACCENT));
    }

    // Playhead
    let x = x_of(app.ed.frame);
    if x >= grid_x0 - CW && x <= rect.right() {
        let gp = ui.painter_at(grid_rect);
        let bottom = (rows_top + (nl - vscroll) as f32 * RH).min(rect.bottom());
        gp.line_segment([pos2(x + CW / 2.0, rect.top()), pos2(x + CW / 2.0, bottom)], Stroke::new(1.5_f32, PLAYHEAD));
        let tag = Rect::from_center_size(pos2(x + CW / 2.0, rect.top() + HH / 2.0), vec2(26.0, HH - 4.0));
        gp.rect_filled(tag, 3.0, PLAYHEAD);
        gp.text(tag.center(), Align2::CENTER_CENTER, format!("{}", app.ed.frame + 1), FontId::monospace(10.5), Color32::WHITE);
    }
    p.line_segment([pos2(grid_x0, rect.top()), pos2(grid_x0, rect.bottom())], Stroke::new(1.0_f32, Color32::from_gray(0x1E)));
}
