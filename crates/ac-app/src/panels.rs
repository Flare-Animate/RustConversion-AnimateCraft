//! Tools panel (left) and Properties + Library panels (right).

use crate::{c32, from32, App, Dialog, Tool, ACCENT};
use ac_core::{pt, Element, ElementKind, LoopMode, Matrix, Stroke, SymbolKind, Tween};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke as EStroke};
use tiny_skia as sk;

pub fn tools_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::left("tools").exact_width(64.0).resizable(false).show(ctx, |ui| {
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            for t in Tool::ALL {
                let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 34.0), Sense::click());
                let active = app.tool == t;
                let bg = if active { Color32::from_gray(0x1E) } else if resp.hovered() { Color32::from_gray(0x44) } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, 4.0, bg);
                draw_tool_icon(ui.painter(), rect, t, if active { ACCENT } else { Color32::from_gray(210) });
                if resp.on_hover_text(t.name()).clicked() {
                    app.tool = t;
                }
            }
            ui.separator();
            ui.label(RichText::new("Stroke").small());
            ui.color_edit_button_srgba(&mut app.stroke);
            ui.checkbox(&mut app.stroke_on, "").on_hover_text("Stroke on/off");
            ui.add(egui::DragValue::new(&mut app.stroke_w).range(0.1..=50.0).speed(0.1)).on_hover_text("Stroke width");
            ui.label(RichText::new("Fill").small());
            ui.color_edit_button_srgba(&mut app.fill);
            ui.checkbox(&mut app.fill_on, "").on_hover_text("Fill on/off");
            if ui.small_button("⇄").on_hover_text("Swap stroke and fill colors").clicked() {
                std::mem::swap(&mut app.fill, &mut app.stroke);
            }
        });
    });
}

pub fn right_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::right("props").default_width(290.0).min_width(240.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::CollapsingHeader::new(RichText::new("Properties").strong()).default_open(true).show(ui, |ui| properties(app, ui));
            ui.add_space(4.0);
            egui::CollapsingHeader::new(RichText::new("Library").strong()).default_open(true).show(ui, |ui| library(app, ui));
        });
    });
}

/// Route a widget change to the editor: drags merge into one undo step,
/// typed/clicked changes are one step each.
fn apply(app: &mut App, resp: &egui::Response, label: &str, f: impl Fn(&mut Element)) {
    if resp.drag_stopped() {
        app.prop_dragging = false;
    }
    if !resp.changed() {
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

fn properties(app: &mut App, ui: &mut egui::Ui) {
    if !app.ed.selection.is_empty() {
        object_properties(app, ui);
    } else {
        frame_properties(app, ui);
        ui.separator();
        document_properties(app, ui);
    }
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
        let (w0, h0) = ((hi.x - lo.x).max(0.01), (hi.y - lo.y).max(0.01));
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
            let r = ui.add(egui::DragValue::new(&mut w).speed(1.0).range(1.0..=10000.0).max_decimals(1));
            let m = Matrix::translate(lo.x, lo.y).concat(&Matrix::scale(w / w0, 1.0)).concat(&Matrix::translate(-lo.x, -lo.y));
            apply(app, &r, "Resize", |e| e.matrix = m.concat(&e.matrix));
            ui.label("H");
            let r = ui.add(egui::DragValue::new(&mut h).speed(1.0).range(1.0..=10000.0).max_decimals(1));
            let m = Matrix::translate(lo.x, lo.y).concat(&Matrix::scale(1.0, h / h0)).concat(&Matrix::translate(-lo.x, -lo.y));
            apply(app, &r, "Resize", |e| e.matrix = m.concat(&e.matrix));
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
                changed |= ui.add(egui::DragValue::new(&mut t).range(-10..=10).suffix(" × CW")).changed();
            });
        }
        if changed {
            app.ed.set_tween_params(e, t);
        }
    }
    frame_buttons(app, ui);
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
        changed |= ui.add(egui::DragValue::new(&mut w).range(16.0..=8000.0)).changed();
        ui.label("H");
        changed |= ui.add(egui::DragValue::new(&mut h).range(16.0..=8000.0)).changed();
        ui.end_row();
        ui.label("FPS");
        changed |= ui.add(egui::DragValue::new(&mut fps).range(1.0..=120.0).speed(0.1)).changed();
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

fn draw_tool_icon(p: &egui::Painter, r: Rect, t: Tool, col: Color32) {
    let c = r.center();
    let s = EStroke::new(1.6_f32, col);
    match t {
        Tool::Select => {
            let pts = vec![c + vec2(-5.0, -9.0), c + vec2(6.0, 2.0), c + vec2(1.0, 2.5), c + vec2(4.0, 9.0), c + vec2(1.5, 10.0), c + vec2(-1.5, 3.5), c + vec2(-5.0, 7.0)];
            p.add(egui::Shape::convex_polygon(pts, col, EStroke::NONE));
        }
        Tool::FreeTransform => {
            let rr = Rect::from_center_size(c, vec2(16.0, 14.0));
            p.rect_stroke(rr, 0.0, EStroke::new(1.0_f32, col));
            for k in [rr.left_top(), rr.right_top(), rr.left_bottom(), rr.right_bottom()] {
                p.rect_filled(Rect::from_center_size(k, vec2(4.0, 4.0)), 0.0, col);
            }
        }
        Tool::Rect => {
            p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0)), 0.0, s);
        }
        Tool::Oval => {
            p.circle_stroke(c, 8.0, s);
        }
        Tool::Line => {
            p.line_segment([c + vec2(-8.0, 8.0), c + vec2(8.0, -8.0)], s);
        }
        Tool::Pencil => {
            p.line_segment([c + vec2(-6.0, 6.0), c + vec2(7.0, -7.0)], EStroke::new(4.0_f32, col));
            p.add(egui::Shape::convex_polygon(vec![c + vec2(-6.5, 3.5), c + vec2(-3.5, 6.5), c + vec2(-9.0, 9.0)], col, EStroke::NONE));
        }
        Tool::Bucket => {
            let pts = vec![c + vec2(-7.0, -1.0), c + vec2(0.0, -8.0), c + vec2(7.0, -1.0), c + vec2(0.0, 6.0)];
            p.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, s));
            p.circle_filled(c + vec2(8.0, 6.0), 2.5, col);
        }
        Tool::Hand => {
            for (i, h) in [6.0, 8.0, 8.0, 6.5].iter().enumerate() {
                let x = -4.5 + i as f32 * 3.0;
                p.line_segment([c + vec2(x, 2.0), c + vec2(x, 2.0 - h)], EStroke::new(2.0_f32, col));
            }
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 4.0), vec2(12.0, 8.0)), 3.0, col);
        }
    }
}
