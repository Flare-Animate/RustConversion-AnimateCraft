//! Tools panel. Same structure as PhotoCraft's toolbar: divider-separated
//! *sections* of *slots*; each slot is a flyout group whose button shows the
//! group's last-used tool (a small corner triangle marks groups). Right-click
//! or long-press a slot to pick another tool from its group. "…" opens Edit
//! Toolbar to hide/show tools. The tool set and shortcuts follow Animate.

use crate::{App, ACCENT};
use ac_core::text::{Align, FontFace};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, Modifiers, Rect, RichText, Sense, Stroke};
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Tool {
    Select,
    Subselect,
    FreeTransform,
    GradientTransform,
    Lasso,
    PolyLasso,
    Pen,
    AddAnchor,
    DeleteAnchor,
    ConvertAnchor,
    Text,
    Line,
    Rect,
    Oval,
    PolyStar,
    Pencil,
    Brush,
    FluidBrush,
    Bone,
    Bucket,
    InkBottle,
    Eyedropper,
    Eraser,
    Width,
    AssetWarp,
    Camera,
    Hand,
    RotateView,
    Zoom,
}

pub struct ToolInfo {
    pub name: &'static str,
    /// Shortcut shown in tooltips; parsed from `key`/`shift`.
    pub key: Option<Key>,
    pub shift: bool,
    /// `None` = implemented. `Some(why)` = shown dimmed, not usable yet.
    pub planned: Option<&'static str>,
    pub tip: &'static str,
}

const fn t(name: &'static str, key: Option<Key>, shift: bool, tip: &'static str) -> ToolInfo {
    ToolInfo { name, key, shift, planned: None, tip }
}
const fn planned(name: &'static str, key: Option<Key>, shift: bool, why: &'static str) -> ToolInfo {
    ToolInfo { name, key, shift, planned: Some(why), tip: "" }
}

impl Tool {
    pub fn info(self) -> ToolInfo {
        use Key::*;
        match self {
            Tool::Select => t("Selection", Some(V), false, "Click or box-select objects; drag to move, corners to scale"),
            Tool::Subselect => t("Subselection", Some(A), false, "Edit the anchor points and curve handles of a shape"),
            Tool::FreeTransform => t("Free Transform", Some(Q), false, "Scale with corners, rotate just outside a corner"),
            Tool::GradientTransform => planned("Gradient Transform", Some(F), false, "needs gradient fills (roadmap M2)"),
            Tool::Lasso => t("Lasso", Some(L), false, "Draw a freehand loop to select the objects inside it"),
            Tool::PolyLasso => t("Polygon Lasso", None, false, "Click corners of a selection polygon; double-click to finish"),
            Tool::Pen => t("Pen", Some(P), false, "Click for corners, drag for curves; click the first point to close; Enter finishes"),
            Tool::AddAnchor => t("Add Anchor Point", Some(Equals), false, "Click a shape's outline to add a point"),
            Tool::DeleteAnchor => t("Delete Anchor Point", Some(Minus), false, "Click a point to remove it"),
            Tool::ConvertAnchor => t("Convert Anchor Point", None, false, "Click a point to switch it between corner and smooth"),
            Tool::Text => t("Text", Some(T), false, "Click to type new text, or click text to edit it"),
            Tool::Line => t("Line", Some(N), false, "Drag a straight line (Shift: 45° steps)"),
            Tool::Rect => t("Rectangle", Some(R), false, "Drag a rectangle (Shift: square). Corner radius in Tool properties"),
            Tool::Oval => t("Oval", Some(O), false, "Drag an oval (Shift: circle)"),
            Tool::PolyStar => t("PolyStar", None, false, "Drag a polygon or star. Sides and style in Tool properties"),
            Tool::Pencil => t("Pencil", Some(Y), true, "Freehand line: Straighten, Smooth or Ink mode"),
            Tool::Brush => t("Paint Brush", Some(Y), false, "Freehand filled stroke using the fill colour"),
            Tool::FluidBrush => planned("Fluid Brush", Some(B), false, "pressure/velocity-sensitive brush (roadmap M2)"),
            Tool::Bone => planned("Bone", Some(M), false, "IK rigging (roadmap M6)"),
            Tool::Bucket => t("Paint Bucket", Some(K), false, "Fill a shape (or tint an instance) with the fill colour"),
            Tool::InkBottle => t("Ink Bottle", Some(S), false, "Give a shape's outline the stroke colour and size"),
            Tool::Eyedropper => t("Eyedropper", Some(I), false, "Pick a colour from an object (fill → Paint Bucket, stroke → Ink Bottle)"),
            Tool::Eraser => t("Eraser", Some(E), false, "Erase whole objects you drag over (cutting fills needs Merge Drawing)"),
            Tool::Width => planned("Width", Some(U), false, "variable-width strokes (roadmap M2)"),
            Tool::AssetWarp => planned("Asset Warp", Some(W), false, "mesh warping with pins (roadmap M6)"),
            Tool::Camera => planned("Camera", None, false, "camera layer (roadmap M4)"),
            Tool::Hand => t("Hand", Some(H), false, "Drag to pan (or hold Space with any tool)"),
            Tool::RotateView => planned("Rotation", Some(H), true, "rotating the view (roadmap M1)"),
            Tool::Zoom => t("Zoom", Some(Z), false, "Click to zoom in, Alt+click out, drag a box to zoom to it"),
        }
    }
    pub fn shortcut_label(self) -> String {
        let i = self.info();
        match i.key {
            Some(k) => format!("{}{}", if i.shift { "Shift+" } else { "" }, k.symbol_or_name()),
            None => String::new(),
        }
    }
    pub fn usable(self) -> bool {
        self.info().planned.is_none()
    }
    pub fn tooltip(self) -> String {
        let i = self.info();
        let sc = self.shortcut_label();
        let head = if sc.is_empty() { i.name.to_string() } else { format!("{} ({sc})", i.name) };
        match i.planned {
            Some(why) => format!("{head}\nPlanned: {why}"),
            None => format!("{head}\n{}", i.tip),
        }
    }
    pub fn is_drawing(self) -> bool {
        matches!(self, Tool::Rect | Tool::Oval | Tool::PolyStar | Tool::Line | Tool::Pencil | Tool::Brush | Tool::Pen | Tool::Text)
    }
}

/// Toolbar layout: sections → slots (flyout groups) → tools. Animate order.
pub const SECTIONS: &[&[&[Tool]]] = &[
    &[&[Tool::Select], &[Tool::Subselect], &[Tool::FreeTransform, Tool::GradientTransform], &[Tool::Lasso, Tool::PolyLasso]],
    &[&[Tool::Pen, Tool::AddAnchor, Tool::DeleteAnchor, Tool::ConvertAnchor], &[Tool::Text], &[Tool::Line], &[Tool::Rect, Tool::Oval, Tool::PolyStar]],
    &[&[Tool::Brush, Tool::Pencil, Tool::FluidBrush], &[Tool::Bone], &[Tool::Bucket, Tool::InkBottle, Tool::Eyedropper], &[Tool::Eraser], &[Tool::Width], &[Tool::AssetWarp]],
    &[&[Tool::Camera], &[Tool::Hand, Tool::RotateView], &[Tool::Zoom]],
];

pub fn all_tools() -> impl Iterator<Item = Tool> {
    SECTIONS.iter().flat_map(|s| s.iter()).flat_map(|slot| slot.iter().copied())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PencilMode {
    Straighten,
    Smooth,
    Ink,
}

/// Per-tool settings shown in the Tool tab of the Properties panel.
pub struct ToolOpts {
    pub rect_radius: f32,
    pub poly_sides: usize,
    pub poly_star: bool,
    pub poly_inner: f32,
    pub pencil_mode: PencilMode,
    pub brush_size: f32,
    pub brush_smooth: f32,
    pub eraser_size: f32,
    pub text_size: f32,
    pub text_font: FontFace,
    pub text_align: Align,
    pub snap: bool,
    /// Remembered tool per slot (slot index → tool).
    pub slot_memory: Vec<Tool>,
    pub hidden: BTreeSet<Tool>,
    pub flyout: Option<(usize, Rect)>,
    pub edit_toolbar_open: bool,
}

impl Default for ToolOpts {
    fn default() -> Self {
        ToolOpts {
            rect_radius: 0.0,
            poly_sides: 5,
            poly_star: true,
            poly_inner: 0.5,
            pencil_mode: PencilMode::Smooth,
            brush_size: 10.0,
            brush_smooth: 50.0,
            eraser_size: 12.0,
            text_size: 28.0,
            text_font: FontFace::Sans,
            text_align: Align::Left,
            snap: true,
            slot_memory: SECTIONS.iter().flat_map(|s| s.iter()).map(|slot| slot[0]).collect(),
            hidden: BTreeSet::new(),
            flyout: None,
            edit_toolbar_open: false,
        }
    }
}

/// Keyboard shortcuts for tools (call when no text field has focus).
pub fn tool_shortcuts(app: &mut App, ctx: &egui::Context) {
    // egui ignores extra Shift when matching, so Shift+key tools must be checked first.
    let mut tools: Vec<Tool> = all_tools().collect();
    tools.sort_by_key(|t| !t.info().shift);
    for tool in tools {
        let i = tool.info();
        let Some(k) = i.key else { continue };
        let m = if i.shift { Modifiers::SHIFT } else { Modifiers::NONE };
        if ctx.input_mut(|inp| inp.consume_key(m, k)) {
            app.set_tool(tool);
        }
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::X)) {
        std::mem::swap(&mut app.fill, &mut app.stroke);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::D)) {
        app.stroke = Color32::BLACK;
        app.fill = Color32::WHITE;
        app.stroke_on = true;
        app.fill_on = true;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::J)) {
        app.status = "Object Drawing is always on in this version; Merge Drawing is on the roadmap (M2).".into();
    }
}

pub fn panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::left("tools").exact_width(92.0).resizable(false).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                let mut slot_index = 0usize;
                for (si, section) in SECTIONS.iter().enumerate() {
                    // Two columns, like Animate's expanded toolbar.
                    let mut slots: Vec<(usize, Vec<Tool>)> = Vec::new();
                    for slot in section.iter() {
                        let visible: Vec<Tool> = slot.iter().copied().filter(|t| !app.topts.hidden.contains(t)).collect();
                        if !visible.is_empty() {
                            slots.push((slot_index, visible));
                        }
                        slot_index += 1;
                    }
                    for pair in slots.chunks(2) {
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            for (idx, tools) in pair {
                                slot_button(app, ui, *idx, tools);
                            }
                        });
                    }
                    if !slots.is_empty() && si + 1 < SECTIONS.len() {
                        ui.add_space(3.0);
                        let (r, _) = ui.allocate_exact_size(vec2(76.0, 1.0), Sense::hover());
                        ui.painter().rect_filled(r, 0.0, Color32::from_gray(0x48));
                        ui.add_space(3.0);
                    }
                }
                // Edit Toolbar
                if ui.add(egui::Button::new("…").min_size(vec2(76.0, 18.0))).on_hover_text("Edit Toolbar: show or hide tools").clicked() {
                    app.topts.edit_toolbar_open = true;
                }
                ui.add_space(6.0);
                color_controls(app, ui);
            });
        });
    });
    flyout(app, ctx);
    edit_toolbar(app, ctx);
}

fn slot_button(app: &mut App, ui: &mut egui::Ui, idx: usize, tools: &[Tool]) {
    let remembered = app.topts.slot_memory.get(idx).copied().unwrap_or(tools[0]);
    let shown = if tools.contains(&app.tool) { app.tool } else if tools.contains(&remembered) { remembered } else { tools[0] };
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 32.0), Sense::click_and_drag());
    let active = app.tool == shown;
    let bg = if active { Color32::from_gray(0x1E) } else if resp.hovered() { Color32::from_gray(0x44) } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 4.0, bg);
    let col = if !shown.usable() { Color32::from_gray(95) } else if active { ACCENT } else { Color32::from_gray(210) };
    draw_icon(ui.painter(), rect, shown, col);
    if tools.len() > 1 {
        // flyout indicator (small corner triangle)
        let c = rect.right_bottom() - vec2(4.0, 4.0);
        ui.painter().add(egui::Shape::convex_polygon(vec![c, c - vec2(5.0, 0.0), c - vec2(0.0, 5.0)], Color32::from_gray(150), Stroke::NONE));
    }
    let held = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.press_start_time().map(|t| i.time - t > 0.45).unwrap_or(false));
    let resp = resp.on_hover_text(shown.tooltip());
    if (resp.secondary_clicked() || held) && tools.len() > 1 {
        app.topts.flyout = Some((idx, rect));
    } else if resp.clicked() {
        app.set_tool(shown);
    }
}

fn flyout(app: &mut App, ctx: &egui::Context) {
    let Some((idx, anchor)) = app.topts.flyout else { return };
    let slot: Vec<Tool> = SECTIONS.iter().flat_map(|s| s.iter()).nth(idx).map(|s| s.iter().copied().filter(|t| !app.topts.hidden.contains(t)).collect()).unwrap_or_default();
    let area = egui::Area::new(egui::Id::new("tool-flyout")).order(egui::Order::Foreground).fixed_pos(anchor.right_top() + vec2(4.0, 0.0));
    let r = area.show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            for tool in slot {
                let label = format!("{:<24} {}", tool.info().name, tool.shortcut_label());
                let txt = if tool.usable() { RichText::new(label) } else { RichText::new(format!("{label}   (planned)")).color(Color32::from_gray(120)) };
                if ui.selectable_label(app.tool == tool, txt).on_hover_text(tool.tooltip()).clicked() {
                    app.set_tool(tool);
                    app.topts.flyout = None;
                }
            }
        });
    });
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed()) && !r.response.rect.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default()) && !anchor.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default());
    if clicked_outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.topts.flyout = None;
    }
}

fn edit_toolbar(app: &mut App, ctx: &egui::Context) {
    if !app.topts.edit_toolbar_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Edit Toolbar").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.label("Untick tools to hide them from the Tools panel.");
        ui.label(RichText::new("Dimmed tools are planned and not usable yet.").small().color(Color32::from_gray(150)));
        ui.separator();
        egui::Grid::new("edit-toolbar").num_columns(2).show(ui, |ui| {
            for (i, tool) in all_tools().enumerate() {
                let mut shown = !app.topts.hidden.contains(&tool);
                let txt = if tool.usable() { RichText::new(tool.info().name) } else { RichText::new(tool.info().name).color(Color32::from_gray(120)) };
                if ui.checkbox(&mut shown, txt).changed() {
                    if shown { app.topts.hidden.remove(&tool); } else { app.topts.hidden.insert(tool); }
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Show all").clicked() { app.topts.hidden.clear(); }
            if ui.button("Hide planned tools").clicked() {
                for tl in all_tools().filter(|t| !t.usable()) { app.topts.hidden.insert(tl); }
            }
        });
    });
    app.topts.edit_toolbar_open = open;
}

fn color_controls(app: &mut App, ui: &mut egui::Ui) {
    let sw = |ui: &mut egui::Ui, label: &str, c: &mut Color32, on: &mut bool, tip: &str| {
        ui.label(RichText::new(label).small());
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.add_enabled_ui(*on, |ui| ui.color_edit_button_srgba(c)).response.on_hover_text(tip);
        });
        let (r, resp) = ui.allocate_exact_size(vec2(36.0, 14.0), Sense::click());
        ui.painter().rect_stroke(r.shrink(1.0), 2.0, Stroke::new(1.0_f32, Color32::from_gray(120)));
        if !*on {
            ui.painter().line_segment([r.left_bottom(), r.right_top()], Stroke::new(1.5_f32, Color32::from_rgb(0xE5, 0x3B, 0x3B)));
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, if *on { "on" } else { "none" }, FontId::proportional(9.0), Color32::from_gray(170));
        if resp.on_hover_text("No colour on/off").clicked() {
            *on = !*on;
        }
    };
    sw(ui, "Stroke", &mut app.stroke, &mut app.stroke_on, "Stroke colour");
    sw(ui, "Fill", &mut app.fill, &mut app.fill_on, "Fill colour");
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        if ui.small_button("Swap").on_hover_text("Swap stroke and fill colours (X)").clicked() {
            std::mem::swap(&mut app.fill, &mut app.stroke);
        }
        if ui.small_button("B/W").on_hover_text("Default black stroke, white fill (D)").clicked() {
            app.stroke = Color32::BLACK;
            app.fill = Color32::WHITE;
            app.stroke_on = true;
            app.fill_on = true;
        }
    });
    ui.add(egui::DragValue::new(&mut app.stroke_w).range(0.1..=100.0).speed(0.1).suffix(" px")).on_hover_text("Stroke size");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.toggle_value(&mut app.topts.snap, "Snap").on_hover_text("Snap to objects and the stage (hold Alt to bypass)");
        if ui.selectable_label(true, "Obj").on_hover_text("Object Drawing (J): always on for now; Merge Drawing is planned").clicked() {
            app.status = "Object Drawing is always on in this version; Merge Drawing is on the roadmap (M2).".into();
        }
    });
}

pub fn draw_icon(p: &egui::Painter, r: Rect, t: Tool, col: Color32) {
    let c = r.center();
    let s = Stroke::new(1.6_f32, col);
    let thin = Stroke::new(1.0_f32, col);
    let poly = |pts: Vec<egui::Pos2>, fill: Color32, st: Stroke| egui::Shape::convex_polygon(pts, fill, st);
    let arrow = |off: egui::Vec2, filled: bool| {
        let pts = vec![c + off + vec2(-5.0, -9.0), c + off + vec2(6.0, 2.0), c + off + vec2(1.0, 2.5), c + off + vec2(4.0, 9.0), c + off + vec2(1.5, 10.0), c + off + vec2(-1.5, 3.5), c + off + vec2(-5.0, 7.0)];
        if filled { poly(pts, col, Stroke::NONE) } else { egui::Shape::closed_line(pts, thin) }
    };
    match t {
        Tool::Select => { p.add(arrow(vec2(0.0, 0.0), true)); }
        Tool::Subselect => { p.add(arrow(vec2(0.0, 0.0), false)); }
        Tool::FreeTransform | Tool::GradientTransform => {
            let rr = Rect::from_center_size(c, vec2(16.0, 14.0));
            p.rect_stroke(rr, 0.0, thin);
            for k in [rr.left_top(), rr.right_top(), rr.left_bottom(), rr.right_bottom()] {
                p.rect_filled(Rect::from_center_size(k, vec2(4.0, 4.0)), 0.0, col);
            }
            if t == Tool::GradientTransform { p.circle_stroke(c, 3.0, thin); }
        }
        Tool::Lasso => {
            let pts: Vec<_> = (0..=20).map(|i| { let a = i as f32 / 20.0 * 5.6; c + vec2(a.cos() * 8.0, a.sin() * 5.0 - 2.0) }).collect();
            p.add(egui::Shape::line(pts, s));
            p.line_segment([c + vec2(6.0, 1.0), c + vec2(4.0, 9.0)], s);
        }
        Tool::PolyLasso => {
            p.add(egui::Shape::closed_line(vec![c + vec2(-8.0, -4.0), c + vec2(0.0, -8.0), c + vec2(8.0, -2.0), c + vec2(3.0, 6.0), c + vec2(-6.0, 4.0)], s));
        }
        Tool::Pen | Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor => {
            p.add(poly(vec![c + vec2(0.0, -9.0), c + vec2(6.0, 3.0), c + vec2(0.0, 6.0), c + vec2(-6.0, 3.0)], Color32::TRANSPARENT, s));
            p.line_segment([c + vec2(0.0, -9.0), c + vec2(0.0, 1.0)], thin);
            let badge = c + vec2(8.0, -7.0);
            match t {
                Tool::AddAnchor => { p.line_segment([badge - vec2(3.0, 0.0), badge + vec2(3.0, 0.0)], s); p.line_segment([badge - vec2(0.0, 3.0), badge + vec2(0.0, 3.0)], s); }
                Tool::DeleteAnchor => { p.line_segment([badge - vec2(3.0, 0.0), badge + vec2(3.0, 0.0)], s); }
                Tool::ConvertAnchor => { p.add(egui::Shape::line(vec![badge + vec2(-3.0, 3.0), badge + vec2(0.0, -3.0), badge + vec2(3.0, 3.0)], thin)); }
                _ => {}
            }
        }
        Tool::Text => { p.text(c, Align2::CENTER_CENTER, "T", FontId::proportional(20.0), col); }
        Tool::Line => { p.line_segment([c + vec2(-8.0, 8.0), c + vec2(8.0, -8.0)], s); }
        Tool::Rect => { p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0)), 0.0, s); }
        Tool::Oval => { p.circle_stroke(c, 8.0, s); }
        Tool::PolyStar => {
            let pts: Vec<_> = (0..10).map(|i| { let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0; let rr = if i % 2 == 0 { 9.0 } else { 4.0 }; c + vec2(a.cos() * rr, a.sin() * rr) }).collect();
            p.add(egui::Shape::closed_line(pts, s));
        }
        Tool::Pencil => {
            p.line_segment([c + vec2(-6.0, 6.0), c + vec2(7.0, -7.0)], Stroke::new(4.0_f32, col));
            p.add(poly(vec![c + vec2(-6.5, 3.5), c + vec2(-3.5, 6.5), c + vec2(-9.0, 9.0)], col, Stroke::NONE));
        }
        Tool::Brush | Tool::FluidBrush => {
            p.line_segment([c + vec2(-1.0, 1.0), c + vec2(8.0, -8.0)], Stroke::new(2.0_f32, col));
            p.circle_filled(c + vec2(-4.0, 4.0), 4.5, col);
            if t == Tool::FluidBrush { p.circle_stroke(c + vec2(-4.0, 4.0), 7.0, thin); }
        }
        Tool::Bone => {
            p.line_segment([c + vec2(-7.0, 7.0), c + vec2(7.0, -7.0)], Stroke::new(3.0_f32, col));
            p.circle_filled(c + vec2(-7.0, 7.0), 3.0, col);
            p.circle_filled(c + vec2(7.0, -7.0), 3.0, col);
        }
        Tool::Bucket => {
            p.add(poly(vec![c + vec2(-7.0, -1.0), c + vec2(0.0, -8.0), c + vec2(7.0, -1.0), c + vec2(0.0, 6.0)], Color32::TRANSPARENT, s));
            p.circle_filled(c + vec2(8.0, 6.0), 2.5, col);
        }
        Tool::InkBottle => {
            p.rect_stroke(Rect::from_center_size(c + vec2(0.0, 3.0), vec2(12.0, 11.0)), 2.0, s);
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, -6.0), vec2(6.0, 5.0)), 1.0, col);
        }
        Tool::Eyedropper => {
            p.line_segment([c + vec2(-7.0, 7.0), c + vec2(4.0, -4.0)], Stroke::new(2.5_f32, col));
            p.circle_filled(c + vec2(6.0, -6.0), 3.5, col);
        }
        Tool::Eraser => {
            p.add(poly(vec![c + vec2(-9.0, 4.0), c + vec2(1.0, -7.0), c + vec2(9.0, 0.0), c + vec2(0.0, 9.0)], Color32::TRANSPARENT, s));
            p.line_segment([c + vec2(-4.0, -1.5), c + vec2(4.0, 5.5)], thin);
        }
        Tool::Width => {
            p.add(egui::Shape::line(vec![c + vec2(-9.0, 2.0), c + vec2(-3.0, -4.0), c + vec2(3.0, 4.0), c + vec2(9.0, -2.0)], Stroke::new(3.0_f32, col)));
        }
        Tool::AssetWarp => {
            p.circle_filled(c + vec2(0.0, -3.0), 4.0, col);
            p.line_segment([c + vec2(0.0, 1.0), c + vec2(0.0, 9.0)], s);
        }
        Tool::Camera => {
            p.rect_stroke(Rect::from_center_size(c + vec2(-2.0, 1.0), vec2(14.0, 10.0)), 2.0, s);
            p.add(poly(vec![c + vec2(5.0, 1.0), c + vec2(10.0, -3.0), c + vec2(10.0, 5.0)], col, Stroke::NONE));
        }
        Tool::Hand => {
            for (i, h) in [6.0, 8.0, 8.0, 6.5].iter().enumerate() {
                let x = -4.5 + i as f32 * 3.0;
                p.line_segment([c + vec2(x, 2.0), c + vec2(x, 2.0 - h)], Stroke::new(2.0_f32, col));
            }
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 4.0), vec2(12.0, 8.0)), 3.0, col);
        }
        Tool::RotateView => {
            let pts: Vec<_> = (0..=16).map(|i| { let a = 0.6 + i as f32 / 16.0 * 4.8; c + vec2(a.cos() * 8.0, a.sin() * 8.0) }).collect();
            p.add(egui::Shape::line(pts, s));
            p.add(poly(vec![c + vec2(6.0, -9.0), c + vec2(10.0, -3.0), c + vec2(3.0, -3.0)], col, Stroke::NONE));
        }
        Tool::Zoom => {
            p.circle_stroke(c + vec2(-2.0, -2.0), 6.0, s);
            p.line_segment([c + vec2(2.5, 2.5), c + vec2(8.0, 8.0)], Stroke::new(2.5_f32, col));
        }
    }
    let _ = pos2;
}
