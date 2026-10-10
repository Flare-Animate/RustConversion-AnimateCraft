//! Timeline panel: layer column + frame grid, styled after Animate's timeline
//! (keyframe dots, purple classic-tween spans, green shape-tween spans, red playhead).

use crate::{c32, App, ACCENT};
use ac_core::{LayerFlag, Tween};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Sense, Stroke};

#[derive(Default)]
pub struct TimelineState {
    /// First visible frame column.
    pub scroll: f32,
    pub renaming: Option<(usize, String)>,
}

const LW: f32 = 230.0; // layer column width
const HH: f32 = 22.0; // header height
const RH: f32 = 24.0; // row height
const CW: f32 = 11.0; // frame cell width

const CLASSIC: Color32 = Color32::from_rgb(0x6E, 0x5B, 0xB0);
const SHAPE: Color32 = Color32::from_rgb(0x4E, 0x8F, 0x57);
const PLAYHEAD: Color32 = Color32::from_rgb(0xE5, 0x3B, 0x3B);

pub fn frame_menu(app: &mut App, ui: &mut egui::Ui) {
    let ed = &mut app.ed;
    let item = |ui: &mut egui::Ui, label: &str, sc: &str| ui.add(egui::Button::new(label).shortcut_text(sc)).clicked();
    if item(ui, "Insert Frame", "F5") { ed.insert_frame(); ui.close_menu(); }
    if item(ui, "Remove Frames", "Shift+F5") { ed.remove_frame(); ui.close_menu(); }
    if item(ui, "Insert Keyframe", "F6") { ed.insert_keyframe(); ui.close_menu(); }
    if item(ui, "Insert Blank Keyframe", "F7") { ed.insert_blank_keyframe(); ui.close_menu(); }
    if item(ui, "Clear Keyframe", "Shift+F6") { ed.clear_keyframe(); ui.close_menu(); }
    ui.separator();
    if item(ui, "Create Classic Tween", "") { ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 }); ui.close_menu(); }
    if item(ui, "Create Shape Tween", "") { ed.set_tween(Tween::Shape { ease: 0.0 }); ui.close_menu(); }
    if item(ui, "Remove Tween", "") { ed.set_tween(Tween::None); ui.close_menu(); }
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
        let n = app.ed.frame_count();
        if ui.button("⏮").on_hover_text("Go to first frame").clicked() { app.playing = false; app.ed.set_frame(0); }
        if ui.button("◀").on_hover_text("Step back (,)").clicked() { app.playing = false; app.ed.set_frame(app.ed.frame.saturating_sub(1)); }
        if ui.button(if app.playing { "⏸" } else { "▶" }).on_hover_text("Play / stop (Enter)").clicked() { app.toggle_play(); }
        if ui.button("▶|").on_hover_text("Step forward (.)").clicked() { app.playing = false; app.ed.set_frame((app.ed.frame + 1).min(n - 1)); }
        if ui.button("⏭").on_hover_text("Go to last frame").clicked() { app.playing = false; app.ed.set_frame(n - 1); }
        ui.toggle_value(&mut app.loop_play, "🔁").on_hover_text("Loop playback");
        ui.separator();
        ui.toggle_value(&mut app.onion, "Onion skin").on_hover_text("Show previous (blue) and next (green) frames");
        if app.onion {
            ui.add(egui::DragValue::new(&mut app.onion_n).range(1..=10).prefix("±"));
        }
        ui.separator();
        ui.label(egui::RichText::new(format!("{}", app.ed.frame + 1)).monospace().color(ACCENT).size(15.0));
        ui.label(egui::RichText::new(format!("/ {n}")).monospace());
        ui.label(egui::RichText::new(format!("{:.2} fps", app.ed.doc.fps)).monospace().color(Color32::from_gray(160)));
        ui.label(egui::RichText::new(format!("{:.1}s", app.ed.frame as f32 / app.ed.doc.fps)).monospace().color(Color32::from_gray(160)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("🗑").on_hover_text("Delete layer").clicked() { app.ed.delete_layer(); }
            if ui.button("➕ Layer").on_hover_text("New layer").clicked() { app.ed.add_layer(); }
            if ui.button("F7").on_hover_text("Insert blank keyframe").clicked() { app.ed.insert_blank_keyframe(); }
            if ui.button("F6").on_hover_text("Insert keyframe").clicked() { app.ed.insert_keyframe(); }
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
    let n = app.ed.frame_count();

    // Follow the playhead while playing; wheel scrolls horizontally.
    let st = &mut app.tl_state;
    if app.playing {
        let f = app.ed.frame as f32;
        if f < st.scroll || f >= st.scroll + visible_cols as f32 - 2.0 {
            st.scroll = (f - 2.0).max(0.0);
        }
    }
    let grid_rect = Rect::from_min_max(pos2(grid_x0, rect.top()), rect.max);
    if ui.rect_contains_pointer(grid_rect) {
        let d = ui.input(|i| i.raw_scroll_delta);
        let delta = if d.x.abs() > d.y.abs() { -d.x } else { -d.y };
        st.scroll = (st.scroll + delta / CW).clamp(0.0, (n as f32 + 200.0).max(0.0));
    }
    let scroll = st.scroll;
    let first = scroll.floor() as u32;
    let x_of = |f: u32| grid_x0 + (f as f32 - scroll) * CW;

    // --- header
    let header = Rect::from_min_size(pos2(grid_x0, rect.top()), vec2(grid_w, HH));
    p.rect_filled(header, 0.0, Color32::from_gray(0x33));
    p.rect_filled(Rect::from_min_size(rect.min, vec2(LW, HH)), 0.0, Color32::from_gray(0x33));
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
            let f = (((pos.x - grid_x0) / CW) + app.tl_state.scroll).floor().max(0.0) as u32;
            app.ed.set_frame(f.min(n + 200));
        }
    }

    // --- rows
    let layers = app.ed.timeline().layers.clone();
    let rows_top = rect.top() + HH;
    for (i, layer) in layers.iter().enumerate() {
        let y = rows_top + i as f32 * RH;
        if y > rect.bottom() {
            break;
        }
        let row_left = Rect::from_min_size(pos2(rect.left(), y), vec2(LW, RH));
        let selected = i == app.ed.layer;
        p.rect_filled(row_left, 0.0, if selected { Color32::from_rgb(0x2B, 0x4F, 0x7A) } else { Color32::from_gray(0x30) });
        p.line_segment([pos2(rect.left(), y + RH), pos2(rect.right(), y + RH)], Stroke::new(1.0_f32, Color32::from_gray(0x22)));

        // Name (click = select, double-click = rename)
        let name_rect = Rect::from_min_size(pos2(rect.left() + 8.0, y), vec2(LW - 90.0, RH));
        let renaming = matches!(&app.tl_state.renaming, Some((ri, _)) if *ri == i);
        if renaming {
            let (_, buf) = app.tl_state.renaming.as_mut().unwrap();
            let r = ui.put(name_rect.shrink(2.0), egui::TextEdit::singleline(buf));
            r.request_focus();
            if r.lost_focus() {
                let (_, name) = app.tl_state.renaming.take().unwrap();
                if !name.trim().is_empty() {
                    app.ed.rename_layer(i, name.trim());
                }
            }
        } else {
            p.text(name_rect.left_center(), Align2::LEFT_CENTER, &layer.name, FontId::proportional(13.0), if layer.visible { Color32::from_gray(225) } else { Color32::from_gray(120) });
            let r = ui.interact(name_rect, ui.id().with(("ln", i)), Sense::click());
            if r.clicked() {
                app.ed.layer = i;
                app.ed.selection = None;
            }
            if r.double_clicked() {
                app.tl_state.renaming = Some((i, layer.name.clone()));
            }
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
        for (k, flag) in [(0, LayerFlag::Visible), (1, LayerFlag::Locked), (2, LayerFlag::Outline)] {
            if ui.interact(icon(k), ui.id().with(("lf", i, k)), Sense::click()).clicked() {
                app.ed.toggle_layer_flag(i, flag);
            }
        }

        // Frame cells
        let row = Rect::from_min_max(pos2(grid_x0, y), pos2(rect.right(), y + RH));
        let rp = ui.painter_at(row);
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
        // Spans, tweens and keyframes
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
                        // broken tween (no end keyframe): dashed line like Animate
                        let mut x = x0 + CW;
                        while x < x1 - 4.0 {
                            rp.line_segment([pos2(x, ym), pos2((x + 3.0).min(x1), ym)], stroke);
                            x += 6.0;
                        }
                    }
                }
                Tween::None => {
                    if end > start + 1 {
                        // end-of-span marker
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
        // Row interaction: click selects frame + layer, right-click opens frame menu.
        let r = ui.interact(row, ui.id().with(("row", i)), Sense::click());
        if let Some(pos) = r.interact_pointer_pos() {
            if r.clicked() || r.secondary_clicked() {
                app.playing = false;
                let f = (((pos.x - grid_x0) / CW) + app.tl_state.scroll).floor().max(0.0) as u32;
                app.ed.set_frame(f);
                app.ed.layer = i;
            }
        }
        r.context_menu(|ui| frame_menu(app, ui));
    }

    // Playhead
    let x = x_of(app.ed.frame);
    if x >= grid_x0 - CW && x <= rect.right() {
        let gp = ui.painter_at(grid_rect);
        let bottom = (rows_top + layers.len() as f32 * RH).min(rect.bottom());
        gp.line_segment([pos2(x + CW / 2.0, rect.top()), pos2(x + CW / 2.0, bottom)], Stroke::new(1.5_f32, PLAYHEAD));
        let tag = Rect::from_center_size(pos2(x + CW / 2.0, rect.top() + HH / 2.0), vec2(26.0, HH - 4.0));
        gp.rect_filled(tag, 3.0, PLAYHEAD);
        gp.text(tag.center(), Align2::CENTER_CENTER, format!("{}", app.ed.frame + 1), FontId::monospace(10.5), Color32::WHITE);
    }
    p.line_segment([pos2(grid_x0, rect.top()), pos2(grid_x0, rect.bottom())], Stroke::new(1.0_f32, Color32::from_gray(0x1E)));
}
