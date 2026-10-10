//! Tools panel (left) and Properties + Library panels (right).

use crate::toolbar::{PencilMode, Tool};
use crate::{c32, from32, App, Dialog, PropsTab, ACCENT};
use ac_core::text::{Align, FontFace};
use ac_core::{pt, Element, ElementKind, LoopMode, Matrix, Stroke, SymbolKind, Tween};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke as EStroke};
use tiny_skia as sk;

pub fn right_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::right("props").default_width(290.0).min_width(240.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::CollapsingHeader::new(RichText::new("Properties").strong()).default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (t, name) in [(PropsTab::Tool, "Tool"), (PropsTab::Object, "Object"), (PropsTab::Frame, "Frame"), (PropsTab::Doc, "Doc")] {
                        ui.selectable_value(&mut app.props_tab, t, name);
                    }
                });
                ui.separator();
                match app.props_tab {
                    PropsTab::Tool => tool_properties(app, ui),
                    PropsTab::Object => {
                        if app.ed.selection.is_empty() {
                            ui.label(RichText::new("Nothing selected. Click an object on the stage.").color(Color32::from_gray(150)));
                        } else {
                            object_properties(app, ui);
                        }
                    }
                    PropsTab::Frame => frame_properties(app, ui),
                    PropsTab::Doc => document_properties(app, ui),
                }
            });
            ui.add_space(4.0);
            egui::CollapsingHeader::new(RichText::new("Library").strong()).default_open(true).show(ui, |ui| library(app, ui));
        });
    });
}

/// A real user edit. egui also reports `changed` when a DragValue clamps a
/// value that is outside its range (e.g. set by a script); that must never be
/// written back, or the panel would keep "fixing" the document every frame.
pub(crate) fn edited(r: &egui::Response) -> bool {
    r.changed() && (r.dragged() || r.has_focus() || r.lost_focus() || r.clicked())
}

/// Route a widget change to the editor: drags merge into one undo step,
/// typed/clicked changes are one step each.
fn apply(app: &mut App, resp: &egui::Response, label: &str, f: impl Fn(&mut Element)) {
    if resp.drag_stopped() {
        app.prop_dragging = false;
    }
    if !edited(resp) {
        return;
    }
    if resp.dragged() {
        if !app.prop_dragging {
            app.ed.begin_drag();
            app.prop_dragging = true;
        }
        app.ed.modify_selected_continuous(f);
    } else {
        app.ed.modify_selected(label, f);
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).strong().color(Color32::from_gray(200)));
}

fn object_properties(app: &mut App, ui: &mut egui::Ui) {
    let n = app.ed.selection.len();
    let Some(el) = app.ed.selected_element() else {
        ui.label("Selection is not visible on this frame.");
        return;
    };
    let editable = app.ed.selection.iter().all(|s| app.ed.layer_editable(s.layer));
    let title = if n > 1 {
        format!("{n} objects")
    } else {
        match &el.kind {
            ElementKind::Shape(_) => "Shape (drawing object)".to_string(),
            ElementKind::Text(_) => "Static text".to_string(),
            ElementKind::Instance(i) => {
                let s = app.ed.doc.symbol(i.symbol);
                let kind = s.map(|s| match s.kind { SymbolKind::Graphic => "Graphic", SymbolKind::MovieClip => "Movie Clip", SymbolKind::Button => "Button" }).unwrap_or("?");
                format!("{kind} instance of “{}”", s.map(|s| s.name.as_str()).unwrap_or("?"))
            }
        }
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).color(Color32::from_gray(200)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(editable, egui::Button::new("🗑 Delete")).on_hover_text("Delete (Del)").clicked() {
                app.delete();
            }
        });
    });
    if !editable {
        ui.label(RichText::new("On a locked or hidden layer: unlock it in the timeline to edit.").small().color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
    }
    ui.add_enabled_ui(editable, |ui| {
        section(ui, "Position and size");
        let (lo, hi) = app.ed.selection_bounds().unwrap_or((pt(0.0, 0.0), pt(0.0, 0.0)));
        let (w0, h0) = (hi.x - lo.x, hi.y - lo.y);
        egui::Grid::new("xform").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
            let (mut x, mut y) = (lo.x, lo.y);
            ui.label("X");
            let r = ui.add(egui::DragValue::new(&mut x).speed(1.0).max_decimals(1));
            apply(app, &r, "Move", |e| e.matrix.tx += x - lo.x);
            ui.label("Y");
            let r = ui.add(egui::DragValue::new(&mut y).speed(1.0).max_decimals(1));
            apply(app, &r, "Move", |e| e.matrix.ty += y - lo.y);
            ui.end_row();
            let (mut w, mut h) = (w0, h0);
            ui.label("W");
            // No range clamp here: egui reports a clamped value as a change,
            // which would rescale a zero-width object every frame.
            let r = ui.add_enabled(w0 > 0.05, egui::DragValue::new(&mut w).speed(1.0).max_decimals(1));
            if w > 0.05 && w0 > 0.05 && (w - w0).abs() > 1e-3 {
                let m = Matrix::translate(lo.x, lo.y).concat(&Matrix::scale(w / w0, 1.0)).concat(&Matrix::translate(-lo.x, -lo.y));
                apply(app, &r, "Resize", |e| e.matrix = m.concat(&e.matrix));
            }
            ui.label("H");
            let r = ui.add_enabled(h0 > 0.05, egui::DragValue::new(&mut h).speed(1.0).max_decimals(1));
            if h > 0.05 && h0 > 0.05 && (h - h0).abs() > 1e-3 {
                let m = Matrix::translate(lo.x, lo.y).concat(&Matrix::scale(1.0, h / h0)).concat(&Matrix::translate(-lo.x, -lo.y));
                apply(app, &r, "Resize", |e| e.matrix = m.concat(&e.matrix));
            }
            ui.end_row();
        });

        section(ui, "Transform");
        let d = el.matrix.decompose();
        egui::Grid::new("xform2").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
            let mut rot = d.skew_y.to_degrees();
            ui.label("Rotate");
            let r = ui.add(egui::DragValue::new(&mut rot).speed(0.5).suffix("°").max_decimals(1));
            apply(app, &r, "Rotate", move |e| {
                let mut dd = e.matrix.decompose();
                let delta = rot.to_radians() - d.skew_y;
                dd.skew_y += delta;
                dd.skew_x += delta;
                e.matrix = Matrix::compose(&dd);
            });
            let mut alpha = el.alpha * 100.0;
            ui.label("Alpha");
            let r = ui.add(egui::DragValue::new(&mut alpha).range(0.0..=100.0).speed(0.5).suffix("%").max_decimals(0));
            apply(app, &r, "Color Effect", |e| e.alpha = alpha / 100.0);
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("⇋ Flip H").clicked() { app.flip(true); }
            if ui.button("⇵ Flip V").clicked() { app.flip(false); }
            if ui.button("↻ 90°").clicked() { app.rotate_selection(std::f32::consts::FRAC_PI_2); }
            if ui.button("Reset").on_hover_text("Remove Transform").clicked() {
                app.ed.modify_selected("Remove Transform", |e| { let (x, y) = (e.matrix.tx, e.matrix.ty); e.matrix = Matrix::translate(x, y); });
            }
        });

        match &el.kind {
            ElementKind::Shape(s) if n == 1 => {
                section(ui, "Fill and stroke");
                ui.horizontal(|ui| {
                    let mut has_fill = s.fill.is_some();
                    let mut c = s.fill.map(c32).unwrap_or(app.fill);
                    if ui.checkbox(&mut has_fill, "Fill").changed() {
                        let col = from32(c);
                        app.ed.modify_selected("Fill", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.fill = has_fill.then_some(col) });
                    }
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        let col = from32(c);
                        app.ed.modify_selected("Fill Color", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.fill = Some(col) });
                    }
                    let mut has_stroke = s.stroke.is_some();
                    let mut sc = s.stroke.map(|s| c32(s.color)).unwrap_or(app.stroke);
                    let w = s.stroke.map(|s| s.width).unwrap_or(app.stroke_w);
                    if ui.checkbox(&mut has_stroke, "Stroke").changed() {
                        let col = from32(sc);
                        app.ed.modify_selected("Stroke", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.stroke = has_stroke.then_some(Stroke { color: col, width: w }) });
                    }
                    if ui.color_edit_button_srgba(&mut sc).changed() {
                        let col = from32(sc);
                        app.ed.modify_selected("Stroke Color", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.stroke = Some(Stroke { color: col, width: w }) });
                    }
                });
                if let Some(st) = s.stroke {
                    let mut w = st.width;
                    ui.horizontal(|ui| {
                        ui.label("Stroke size");
                        let r = ui.add(egui::DragValue::new(&mut w).range(0.1..=100.0).speed(0.1));
                        apply(app, &r, "Stroke Size", |e| if let ElementKind::Shape(s) = &mut e.kind { if let Some(st) = &mut s.stroke { st.width = w } });
                    });
                }
            }
            ElementKind::Text(t) if n == 1 => {
                section(ui, "Text");
                let mut text = t.text.clone();
                let r = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(2).desired_width(f32::INFINITY));
                if r.changed() {
                    let v = text.clone();
                    app.ed.modify_selected("Edit Text", move |e| if let ElementKind::Text(t) = &mut e.kind { t.text = v.clone() });
                }
                let mut size = t.size;
                let mut col = c32(t.color);
                let mut font = t.font;
                let mut align = t.align;
                let mut spacing = t.letter_spacing;
                egui::Grid::new("textprops").num_columns(4).show(ui, |ui| {
                    ui.label("Size");
                    let r = ui.add(egui::DragValue::new(&mut size).range(1.0..=500.0).speed(0.5));
                    apply(app, &r, "Text Size", |e| if let ElementKind::Text(t) = &mut e.kind { t.size = size });
                    ui.label("Colour");
                    if ui.color_edit_button_srgba(&mut col).changed() {
                        let c = from32(col);
                        app.ed.modify_selected("Text Colour", move |e| if let ElementKind::Text(t) = &mut e.kind { t.color = c });
                    }
                    ui.end_row();
                    ui.label("Spacing");
                    let r = ui.add(egui::DragValue::new(&mut spacing).range(-20.0..=100.0).speed(0.1));
                    apply(app, &r, "Letter Spacing", |e| if let ElementKind::Text(t) = &mut e.kind { t.letter_spacing = spacing });
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    for (f, l) in [(FontFace::Sans, "Sans"), (FontFace::Mono, "Mono")] {
                        if ui.selectable_value(&mut font, f, l).changed() {
                            app.ed.modify_selected("Font", move |e| if let ElementKind::Text(t) = &mut e.kind { t.font = font });
                        }
                    }
                    ui.separator();
                    for (a, l) in [(Align::Left, "⬅"), (Align::Center, "↔"), (Align::Right, "➡")] {
                        if ui.selectable_value(&mut align, a, l).changed() {
                            app.ed.modify_selected("Align", move |e| if let ElementKind::Text(t) = &mut e.kind { t.align = align });
                        }
                    }
                });
            }
            ElementKind::Instance(inst) if n == 1 => {
                let id = inst.symbol;
                let kind = app.ed.doc.symbol(id).map(|s| s.kind);
                if kind == Some(SymbolKind::Graphic) {
                    section(ui, "Looping");
                    let mut mode = inst.loop_mode;
                    ui.horizontal(|ui| {
                        for (m, label) in [(LoopMode::Loop, "Loop"), (LoopMode::PlayOnce, "Play Once"), (LoopMode::SingleFrame, "Single Frame")] {
                            if ui.selectable_value(&mut mode, m, label).changed() {
                                app.ed.modify_selected("Looping", move |e| if let ElementKind::Instance(i) = &mut e.kind { i.loop_mode = mode });
                            }
                        }
                    });
                    let mut ff = inst.first_frame + 1;
                    ui.horizontal(|ui| {
                        ui.label("First frame");
                        let r = ui.add(egui::DragValue::new(&mut ff).range(1..=999));
                        apply(app, &r, "First Frame", |e| if let ElementKind::Instance(i) = &mut e.kind { i.first_frame = ff - 1 });
                    });
                }
                section(ui, "Color effect: tint");
                let mut tint = el.tint.map(|(c, _)| c32(c)).unwrap_or(Color32::WHITE);
                let mut amt = el.tint.map(|(_, a)| a * 100.0).unwrap_or(0.0);
                ui.horizontal(|ui| {
                    if ui.color_edit_button_srgba(&mut tint).changed() {
                        let c = from32(tint);
                        let a = (amt / 100.0).max(0.5);
                        app.ed.modify_selected("Tint", move |e| e.tint = Some((c, a)));
                    }
                    let r = ui.add(egui::DragValue::new(&mut amt).range(0.0..=100.0).suffix("%"));
                    let c = from32(tint);
                    apply(app, &r, "Tint", |e| e.tint = if amt <= 0.0 { None } else { Some((c, amt / 100.0)) });
                });
                ui.horizontal(|ui| {
                    if ui.button("Edit Symbol").clicked() { app.edit_symbol(id); }
                    if ui.button("Break Apart").on_hover_text("Ctrl+B").clicked() { app.ed.break_apart(); }
                });
            }
            _ => {}
        }
        ui.horizontal(|ui| {
            if ui.button("Convert to Symbol… (F8)").clicked() { app.open_symbol_dialog(true); }
            if ui.button("Duplicate").clicked() { app.ed.duplicate(); }
        });
    });
}

fn frame_properties(app: &mut App, ui: &mut egui::Ui) {
    let target = app.ed.frame_target();
    let tl = app.ed.timeline();
    let Some(layer) = tl.layers.get(target.layer0) else { return };
    let layer_name = layer.name.clone();
    let what = if target.is_single() {
        format!("Frame {} · {}", target.f0 + 1, layer_name)
    } else if target.layer0 == target.layer1 {
        format!("Frames {}–{} · {}", target.f0 + 1, target.f1 + 1, layer_name)
    } else {
        format!("Frames {}–{} · {} layers", target.f0 + 1, target.f1 + 1, target.layer1 - target.layer0 + 1)
    };
    section(ui, if app.ed.frame_sel.is_some() { "Frame selection" } else { "Frame" });
    ui.label(RichText::new(what).color(Color32::from_gray(200)));

    let span = layer.span_at(target.f0).map(|(s, k, _)| (s, k.clone()));
    let Some((start, kf)) = span else {
        ui.label(RichText::new("Empty frame. Press F6/F7 or draw here to add a keyframe.").small().color(Color32::from_gray(150)));
        frame_buttons(app, ui);
        return;
    };
    ui.label(RichText::new(format!("Keyframe at {} · {} object(s)", start + 1, kf.elements.len())).small().color(Color32::from_gray(150)));

    if target.is_single() {
        let mut label = kf.label.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Label");
            let r = ui.text_edit_singleline(&mut label);
            if r.lost_focus() && label != kf.label.clone().unwrap_or_default() {
                app.ed.set_frame_label(&label);
            }
        });
    }
    // Tween
    let cur = match kf.tween { Tween::None => 0, Tween::Classic { .. } => 1, Tween::Shape { .. } => 2 };
    let mut kind = cur;
    ui.horizontal(|ui| {
        ui.label("Tween");
        egui::ComboBox::from_id_salt("tween").selected_text(["None", "Classic", "Shape"][kind]).show_ui(ui, |ui| {
            ui.selectable_value(&mut kind, 0, "None");
            ui.selectable_value(&mut kind, 1, "Classic");
            ui.selectable_value(&mut kind, 2, "Shape");
        });
    });
    let ease = kf.tween.ease();
    let turns = if let Tween::Classic { rotate_turns, .. } = kf.tween { rotate_turns } else { 0 };
    if kind != cur {
        app.ed.set_tween(match kind {
            1 => Tween::Classic { ease, rotate_turns: turns },
            2 => Tween::Shape { ease },
            _ => Tween::None,
        });
    } else if kind != 0 {
        let (mut e, mut t) = (ease, turns);
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Ease");
            changed |= ui.add(egui::Slider::new(&mut e, -100.0..=100.0).step_by(1.0)).changed();
        });
        ui.label(RichText::new("−100 = ease in (slow start) · +100 = ease out (slow end)").small().color(Color32::from_gray(140)));
        if kind == 1 {
            ui.horizontal(|ui| {
                ui.label("Rotate");
                changed |= edited(&ui.add(egui::DragValue::new(&mut t).range(-10..=10).suffix(" × CW")));
            });
        }
        if changed {
            app.ed.set_tween_params(e, t);
        }
    }
    frame_buttons(app, ui);
    if target.is_single() {
        section(ui, "Frame script (JavaScript)");
        let code = kf.script.clone().unwrap_or_default();
        let lines = code.lines().count();
        ui.label(RichText::new(if code.is_empty() { "No script on this keyframe.".to_string() } else { format!("{lines} line(s) on keyframe {}", start + 1) }).small().color(Color32::from_gray(150)));
        if ui.button("Edit in Actions panel (F9)").clicked() {
            app.scripting.actions_open = true;
        }
    }
}

fn frame_buttons(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("Keyframe (F6)").clicked() { app.ed.insert_keyframe(); }
        if ui.button("Blank (F7)").clicked() { app.ed.insert_blank_keyframe(); }
        if ui.button("+ Frame (F5)").clicked() { app.ed.insert_frame(); }
        if ui.button("− Frames").on_hover_text("Remove Frames (Shift+F5)").clicked() { app.ed.remove_frame(); }
        if ui.button("Clear").on_hover_text("Clear Frames (Delete)").clicked() { app.ed.clear_frames(); }
    });
}

fn document_properties(app: &mut App, ui: &mut egui::Ui) {
    let d = &app.ed.doc;
    let (mut w, mut h, mut fps, mut bg) = (d.width, d.height, d.fps, c32(d.background));
    section(ui, "Document");
    let mut changed = false;
    egui::Grid::new("docprops").num_columns(4).show(ui, |ui| {
        ui.label("W");
        changed |= edited(&ui.add(egui::DragValue::new(&mut w).range(16.0..=8000.0)));
        ui.label("H");
        changed |= edited(&ui.add(egui::DragValue::new(&mut h).range(16.0..=8000.0)));
        ui.end_row();
        ui.label("FPS");
        changed |= edited(&ui.add(egui::DragValue::new(&mut fps).range(0.1..=240.0).speed(0.1)));
        ui.label("Stage");
        changed |= ui.color_edit_button_srgba(&mut bg).changed();
        ui.end_row();
    });
    if changed {
        app.ed.set_doc_props(w, h, fps, from32(bg));
        app.fit_pending = true;
    }
}

fn library(app: &mut App, ui: &mut egui::Ui) {
    // Preview of the selected symbol (first frame), like Animate's library preview pane.
    let (prev_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 110.0), Sense::hover());
    ui.painter().rect_filled(prev_rect, 3.0, Color32::from_gray(0x26));
    if let Some(sym) = app.lib_selected.and_then(|id| app.ed.doc.symbol(id)) {
        let ppp = ui.ctx().pixels_per_point();
        let (pw, ph) = ((prev_rect.width() * ppp) as u32, (prev_rect.height() * ppp) as u32);
        if let Some(mut pix) = sk::Pixmap::new(pw.max(1), ph.max(1)) {
            let ctx = ac_core::EvalCtx { doc: &app.ed.doc, global_frame: 0 };
            let mut items = Vec::new();
            ac_core::evaluate(&ctx, &sym.timeline, 0, &Matrix::IDENTITY, 1.0, 1, &mut items);
            let bounds = items
                .iter()
                .filter_map(|it| it.path.transformed(&it.matrix).bounds())
                .reduce(|a, b| (pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)), pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y))));
            if let Some((lo, hi)) = bounds {
                let s = ((pw as f32 - 16.0) / (hi.x - lo.x).max(1.0)).min((ph as f32 - 16.0) / (hi.y - lo.y).max(1.0)).min(4.0 * ppp);
                let view = Matrix::translate(pw as f32 / 2.0, ph as f32 / 2.0).concat(&Matrix::scale(s, s)).concat(&Matrix::translate(-(lo.x + hi.x) / 2.0, -(lo.y + hi.y) / 2.0));
                ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view, ..Default::default() });
            }
            let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pix.data());
            let tex = match &mut app.lib_tex {
                Some(t) => {
                    t.set(img, egui::TextureOptions::LINEAR);
                    t.clone()
                }
                None => {
                    let t = ui.ctx().load_texture("lib", img, egui::TextureOptions::LINEAR);
                    app.lib_tex = Some(t.clone());
                    t
                }
            };
            ui.painter().image(tex.id(), prev_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    } else {
        ui.painter().text(prev_rect.center(), Align2::CENTER_CENTER, "Select an item to preview", FontId::proportional(12.0), Color32::from_gray(120));
    }
    ui.label(RichText::new(format!("{} items · drag onto the stage to place", app.ed.doc.library.len())).small().color(Color32::from_gray(150)));

    let syms: Vec<(u32, String, SymbolKind, u32)> = app.ed.doc.library.iter().map(|s| (s.id, s.name.clone(), s.kind, s.timeline.frame_count())).collect();
    let stage_center = if app.ed.editing.is_some() { pt(0.0, 0.0) } else { pt(app.ed.doc.width / 2.0, app.ed.doc.height / 2.0) };
    for (id, name, kind, frames) in syms {
        let icon = match kind { SymbolKind::Graphic => "🔷", SymbolKind::MovieClip => "🎞", SymbolKind::Button => "🔘" };
        let resp = ui.add(egui::SelectableLabel::new(app.lib_selected == Some(id), format!("{icon}  {name}   ({frames}f)"))).interact(Sense::click_and_drag());
        if resp.clicked() || resp.drag_started() {
            app.lib_selected = Some(id);
        }
        if resp.drag_started() {
            app.lib_drag = Some(id);
        }
        if resp.double_clicked() {
            app.edit_symbol(id);
        }
        resp.context_menu(|ui| {
            if ui.button("Edit").clicked() { app.edit_symbol(id); ui.close_menu(); }
            if ui.button("Place on Stage").clicked() { app.ed.place_symbol(id, stage_center); ui.close_menu(); }
            if ui.button("Rename…").clicked() { app.dialog = Dialog::RenameSymbol { id, name: name.clone() }; ui.close_menu(); }
            if ui.button("Duplicate").clicked() { app.lib_selected = app.ed.duplicate_symbol(id); ui.close_menu(); }
            ui.menu_button("Type", |ui| {
                for (k, l) in [(SymbolKind::Graphic, "Graphic"), (SymbolKind::MovieClip, "Movie Clip"), (SymbolKind::Button, "Button")] {
                    if ui.radio(kind == k, l).clicked() { app.ed.set_symbol_kind(id, k); ui.close_menu(); }
                }
            });
            ui.separator();
            if ui.button("Delete").clicked() { app.dialog = Dialog::DeleteSymbol { id, uses: app.ed.symbol_usage(id) }; ui.close_menu(); }
        });
    }
    ui.horizontal_wrapped(|ui| {
        let sel = app.lib_selected.filter(|id| app.ed.doc.symbol(*id).is_some());
        if ui.add_enabled(sel.is_some(), egui::Button::new("Place")).clicked() {
            app.ed.place_symbol(sel.unwrap(), stage_center);
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Edit")).clicked() {
            app.edit_symbol(sel.unwrap());
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Rename")).clicked() {
            let id = sel.unwrap();
            app.dialog = Dialog::RenameSymbol { id, name: app.ed.doc.symbol(id).unwrap().name.clone() };
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("🗑")).on_hover_text("Delete library item").clicked() {
            let id = sel.unwrap();
            app.dialog = Dialog::DeleteSymbol { id, uses: app.ed.symbol_usage(id) };
        }
        if ui.button("New Symbol…").clicked() {
            app.open_symbol_dialog(false);
        }
    });
}

/// Ghost label under the cursor while a library item is dragged, and the drop.
pub fn library_drag_ghost(app: &mut App, ctx: &egui::Context) {
    let Some(id) = app.lib_drag else { return };
    let (pos, released) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.any_released()));
    if let Some(p) = pos {
        let name = app.ed.doc.symbol(id).map(|s| s.name.clone()).unwrap_or_default();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("libdrag")));
        let r = painter.text(p + vec2(14.0, 8.0), Align2::LEFT_TOP, format!("🔷 {name}"), FontId::proportional(13.0), Color32::WHITE);
        painter.rect_stroke(r.expand(4.0), 3.0, EStroke::new(1.0_f32, ACCENT));
        if released {
            if app.stage_rect.contains(p) {
                let at = app.to_doc(app.stage_rect, p);
                if app.ed.layer_locked() {
                    app.status = "The current layer is locked or hidden.".into();
                } else {
                    app.ed.place_symbol(id, at);
                    app.status = format!("Placed “{name}”");
                }
            }
            app.lib_drag = None;
        }
    } else if released {
        app.lib_drag = None;
    }
    ctx.request_repaint();
}

fn tool_properties(app: &mut App, ui: &mut egui::Ui) {
    let tool = app.tool;
    let info = tool.info();
    ui.label(RichText::new(format!("{}  {}", info.name, tool.shortcut_label())).strong());
    ui.label(RichText::new(info.tip).small().color(Color32::from_gray(160)));
    ui.add_space(4.0);
    let o = &mut app.topts;
    match tool {
        Tool::Rect => {
            section(ui, "Rectangle options");
            ui.add(egui::Slider::new(&mut o.rect_radius, 0.0..=100.0).text("Corner radius"));
        }
        Tool::PolyStar => {
            section(ui, "PolyStar options");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut o.poly_star, false, "Polygon");
                ui.selectable_value(&mut o.poly_star, true, "Star");
            });
            ui.add(egui::Slider::new(&mut o.poly_sides, 3..=32).text("Sides"));
            ui.add_enabled(o.poly_star, egui::Slider::new(&mut o.poly_inner, 0.05..=1.0).text("Star point size"));
        }
        Tool::Pencil => {
            section(ui, "Pencil mode");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut o.pencil_mode, PencilMode::Straighten, "Straighten");
                ui.selectable_value(&mut o.pencil_mode, PencilMode::Smooth, "Smooth");
                ui.selectable_value(&mut o.pencil_mode, PencilMode::Ink, "Ink");
            });
        }
        Tool::Brush => {
            section(ui, "Paint Brush");
            ui.add(egui::Slider::new(&mut o.brush_size, 1.0..=200.0).text("Size"));
            ui.add(egui::Slider::new(&mut o.brush_smooth, 0.0..=100.0).text("Smoothing"));
            ui.label(RichText::new("Paints with the fill colour.").small().color(Color32::from_gray(150)));
        }
        Tool::Eraser => {
            section(ui, "Eraser");
            ui.add(egui::Slider::new(&mut o.eraser_size, 2.0..=100.0).text("Size"));
            ui.label(RichText::new("Mode: erase objects. Cutting into fills needs Merge Drawing (roadmap M2).").small().color(Color32::from_gray(150)));
        }
        Tool::Text => {
            section(ui, "Character");
            ui.add(egui::Slider::new(&mut o.text_size, 4.0..=200.0).text("Size"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut o.text_font, FontFace::Sans, "Sans");
                ui.selectable_value(&mut o.text_font, FontFace::Mono, "Mono");
                ui.separator();
                ui.selectable_value(&mut o.text_align, Align::Left, "⬅");
                ui.selectable_value(&mut o.text_align, Align::Center, "↔");
                ui.selectable_value(&mut o.text_align, Align::Right, "➡");
            });
            ui.label(RichText::new("Text uses the fill colour.").small().color(Color32::from_gray(150)));
        }
        Tool::Select | Tool::Subselect | Tool::FreeTransform => {
            section(ui, "Snapping");
            ui.checkbox(&mut o.snap, "Snap to objects and stage (hold Alt to bypass)");
        }
        _ => {}
    }
    if tool.is_drawing() || matches!(tool, Tool::InkBottle | Tool::Bucket) {
        section(ui, "Fill and stroke");
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.fill_on, "Fill");
            ui.color_edit_button_srgba(&mut app.fill);
            ui.checkbox(&mut app.stroke_on, "Stroke");
            ui.color_edit_button_srgba(&mut app.stroke);
        });
        ui.add(egui::Slider::new(&mut app.stroke_w, 0.1..=50.0).text("Stroke size"));
    }
}
