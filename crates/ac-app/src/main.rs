//! AnimateCraft desktop editor. Layout follows Adobe Animate's default
//! "Essentials" workspace: menu bar, Tools (left), Stage (centre),
//! Properties + Library (right), Timeline (bottom).

mod timeline;

use ac_core::{
    demo::demo_document, pt, Color, Document, Editor, Element, ElementKind, LoopMode, Matrix, Path, Pt, Shape, Stroke, SymbolKind, Tween,
};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke as EStroke, Vec2};
use tiny_skia as sk;

pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x2D, 0x8C, 0xEB);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Select,
    FreeTransform,
    Rect,
    Oval,
    Line,
    Pencil,
    Bucket,
    Hand,
}

impl Tool {
    const ALL: [Tool; 8] = [Tool::Select, Tool::FreeTransform, Tool::Rect, Tool::Oval, Tool::Line, Tool::Pencil, Tool::Bucket, Tool::Hand];
    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Selection Tool (V)",
            Tool::FreeTransform => "Free Transform Tool (Q)",
            Tool::Rect => "Rectangle Tool (R)",
            Tool::Oval => "Oval Tool (O)",
            Tool::Line => "Line Tool (N)",
            Tool::Pencil => "Pencil Tool (Y)",
            Tool::Bucket => "Paint Bucket Tool (K)",
            Tool::Hand => "Hand Tool (H)",
        }
    }
    fn key(self) -> Key {
        match self {
            Tool::Select => Key::V,
            Tool::FreeTransform => Key::Q,
            Tool::Rect => Key::R,
            Tool::Oval => Key::O,
            Tool::Line => Key::N,
            Tool::Pencil => Key::Y,
            Tool::Bucket => Key::K,
            Tool::Hand => Key::H,
        }
    }
}

enum Drag {
    None,
    Move { last: Pt },
    Scale { anchor: Pt, start: Pt, base: Matrix },
    Rotate { center: Pt, start_angle: f32, base: Matrix },
    Draw { start: Pt, cur: Pt },
    Pencil { pts: Vec<Pt> },
    Pan { last: Pos2 },
}

enum Dialog {
    None,
    Path { title: String, path: String, action: FileAction },
    ConvertSymbol { name: String, kind: SymbolKind },
    NewSymbol { name: String, kind: SymbolKind },
    About,
}

#[derive(Clone, Copy)]
enum FileAction {
    Open,
    Save,
    ExportGif,
    ExportPng,
    ExportSvg,
}

pub struct App {
    pub(crate) ed: Editor,
    tool: Tool,
    fill: Color32,
    fill_on: bool,
    stroke: Color32,
    stroke_on: bool,
    stroke_w: f32,
    zoom: f32,
    pan: Vec2,
    fit_pending: bool,
    pub(crate) playing: bool,
    pub(crate) loop_play: bool,
    play_accum: f64,
    pub(crate) onion: bool,
    pub(crate) onion_n: u32,
    stage_tex: Option<egui::TextureHandle>,
    lib_tex: Option<egui::TextureHandle>,
    lib_selected: Option<u32>,
    drag: Drag,
    prop_dragging: bool,
    dialog: Dialog,
    status: String,
    file_path: Option<String>,
    pub(crate) tl_state: timeline::TimelineState,
    screenshot: Option<(String, u32)>,
}

pub(crate) fn c32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}
fn from32(c: Color32) -> Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color { r, g, b, a }
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let mut v = egui::Visuals::dark();
        v.panel_fill = Color32::from_gray(0x32);
        v.window_fill = Color32::from_gray(0x2B);
        v.extreme_bg_color = Color32::from_gray(0x1E);
        v.selection.bg_fill = ACCENT;
        v.widgets.noninteractive.bg_stroke = EStroke::new(1.0_f32, Color32::from_gray(0x24));
        cc.egui_ctx.set_visuals(v);
        cc.egui_ctx.style_mut(|s| s.spacing.item_spacing = vec2(6.0, 5.0));

        let mut app = App {
            ed: Editor::new(demo_document()),
            tool: Tool::Select,
            fill: Color32::from_rgb(0x2D, 0x8C, 0xEB),
            fill_on: true,
            stroke: Color32::from_rgb(0x10, 0x10, 0x10),
            stroke_on: true,
            stroke_w: 2.0,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            playing: false,
            loop_play: true,
            play_accum: 0.0,
            onion: false,
            onion_n: 2,
            stage_tex: None,
            lib_tex: None,
            lib_selected: None,
            drag: Drag::None,
            prop_dragging: false,
            dialog: Dialog::None,
            status: "Demo document loaded. Press Enter to play, F6 for a keyframe, F8 to convert to symbol.".into(),
            file_path: None,
            tl_state: Default::default(),
            screenshot: None,
        };
        // Headless screenshot hook used by CI/docs: AC_SCREENSHOT=out.png [AC_SCREENSHOT_FRAME=n]
        if let Ok(p) = std::env::var("AC_SCREENSHOT") {
            let frame: u32 = std::env::var("AC_SCREENSHOT_FRAME").ok().and_then(|s| s.parse().ok()).unwrap_or(14);
            app.ed.set_frame(frame.saturating_sub(1));
            app.onion = true;
            app.ed.selection = Some(ac_core::Selection { layer: 0, element: 0 });
            app.lib_selected = Some(1);
            app.screenshot = Some((p, 0));
        }
        app
    }

    // ------------------------------------------------------------ coordinates
    fn content_offset(&self) -> Vec2 {
        // Inside a symbol, its registration point sits at the stage centre.
        if self.ed.editing.is_some() {
            vec2(self.ed.doc.width / 2.0, self.ed.doc.height / 2.0)
        } else {
            Vec2::ZERO
        }
    }
    fn to_screen(&self, rect: Rect, p: Pt) -> Pos2 {
        let o = self.content_offset();
        rect.min + self.pan + vec2(p.x + o.x, p.y + o.y) * self.zoom
    }
    fn to_doc(&self, rect: Rect, s: Pos2) -> Pt {
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

    // ------------------------------------------------------------ actions
    pub(crate) fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.play_accum = 0.0;
        if self.playing && self.ed.frame + 1 >= self.ed.frame_count() {
            self.ed.set_frame(0);
        }
    }

    fn do_file(&mut self, action: FileAction, path: &str) {
        let p = std::path::Path::new(path);
        let res: Result<String, String> = match action {
            FileAction::Open => std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|s| {
                Document::from_json(&s).map_err(|e| e.to_string()).map(|d| {
                    self.ed = Editor::new(d);
                    self.fit_pending = true;
                    self.file_path = Some(path.into());
                    format!("Opened {path}")
                })
            }),
            FileAction::Save => std::fs::write(p, self.ed.doc.to_json()).map_err(|e| e.to_string()).map(|_| {
                self.file_path = Some(path.into());
                format!("Saved {path}")
            }),
            FileAction::ExportGif => ac_render::export_gif(&self.ed.doc, 1.0, p).map(|_| format!("Exported animated GIF to {path}")),
            FileAction::ExportPng => ac_render::export_png(&self.ed.doc, self.ed.frame, 2.0, p).map(|_| format!("Exported frame {} to {path}", self.ed.frame + 1)),
            FileAction::ExportSvg => std::fs::write(p, ac_render::export_svg(&self.ed.doc, self.ed.frame)).map_err(|e| e.to_string()).map(|_| format!("Exported SVG to {path}")),
        };
        self.status = res.unwrap_or_else(|e| format!("Error: {e}"));
    }

    fn open_path_dialog(&mut self, title: &str, default: &str, action: FileAction) {
        self.dialog = Dialog::Path { title: title.into(), path: default.into(), action };
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) || !matches!(self.dialog, Dialog::None) {
            return;
        }
        let cmd = Modifiers::COMMAND;
        let sh = Modifiers::SHIFT;
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        if k(cmd | sh, Key::Z) || k(cmd, Key::Y) {
            if let Some(l) = self.ed.redo() { self.status = format!("Redo {l}"); }
        }
        if k(cmd, Key::Z) {
            if let Some(l) = self.ed.undo() { self.status = format!("Undo {l}"); }
        }
        if k(cmd, Key::S) {
            let p = self.file_path.clone().unwrap_or_else(|| "untitled.acx.json".into());
            self.do_file(FileAction::Save, &p);
        }
        if k(cmd, Key::Equals) || k(cmd, Key::Plus) { self.zoom *= 1.25; }
        if k(cmd, Key::Minus) { self.zoom /= 1.25; }
        if k(cmd, Key::Num1) { self.zoom = 1.0; }
        if k(cmd, Key::Num2) { self.fit_pending = true; }
        if k(sh, Key::F5) { self.ed.remove_frame(); }
        if k(Modifiers::NONE, Key::F5) { self.ed.insert_frame(); }
        if k(sh, Key::F6) { self.ed.clear_keyframe(); }
        if k(Modifiers::NONE, Key::F6) { self.ed.insert_keyframe(); }
        if k(Modifiers::NONE, Key::F7) { self.ed.insert_blank_keyframe(); }
        if k(cmd, Key::F8) { self.dialog = Dialog::NewSymbol { name: format!("Symbol {}", self.ed.doc.next_symbol_id), kind: SymbolKind::Graphic }; }
        if k(Modifiers::NONE, Key::F8) && self.ed.selection.is_some() {
            self.dialog = Dialog::ConvertSymbol { name: format!("Symbol {}", self.ed.doc.next_symbol_id), kind: SymbolKind::Graphic };
        }
        if k(Modifiers::NONE, Key::Enter) { self.toggle_play(); }
        if k(Modifiers::NONE, Key::Period) { self.playing = false; let n = self.ed.frame_count(); self.ed.set_frame((self.ed.frame + 1).min(n - 1)); }
        if k(Modifiers::NONE, Key::Comma) { self.playing = false; self.ed.set_frame(self.ed.frame.saturating_sub(1)); }
        if k(Modifiers::NONE, Key::Delete) || k(Modifiers::NONE, Key::Backspace) { self.ed.delete_selected(); }
        if k(Modifiers::NONE, Key::Escape) {
            if self.ed.selection.is_some() { self.ed.selection = None; } else if self.ed.editing.is_some() { self.ed.exit_to_scene(); self.fit_pending = true; }
        }
        for t in Tool::ALL {
            if k(Modifiers::NONE, t.key()) { self.tool = t; }
        }
    }

    // ------------------------------------------------------------ panels
    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() { self.ed = Editor::new(Document::default()); self.fit_pending = true; self.file_path = None; ui.close_menu(); }
                    if ui.button("Open Demo Scene").clicked() { self.ed = Editor::new(demo_document()); self.fit_pending = true; ui.close_menu(); }
                    if ui.add(egui::Button::new("Open…").shortcut_text("Ctrl+O")).clicked() { self.open_path_dialog("Open document", "untitled.acx.json", FileAction::Open); ui.close_menu(); }
                    ui.separator();
                    if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() {
                        let p = self.file_path.clone().unwrap_or_else(|| "untitled.acx.json".into());
                        self.do_file(FileAction::Save, &p);
                        ui.close_menu();
                    }
                    if ui.button("Save As…").clicked() { self.open_path_dialog("Save document as", "untitled.acx.json", FileAction::Save); ui.close_menu(); }
                    ui.menu_button("Export", |ui| {
                        if ui.button("Export Animated GIF…").clicked() { self.open_path_dialog("Export animated GIF", "animation.gif", FileAction::ExportGif); ui.close_menu(); }
                        if ui.button("Export Image (PNG)…").clicked() { self.open_path_dialog("Export current frame as PNG", "frame.png", FileAction::ExportPng); ui.close_menu(); }
                        if ui.button("Export SVG…").clicked() { self.open_path_dialog("Export current frame as SVG", "frame.svg", FileAction::ExportSvg); ui.close_menu(); }
                    });
                    ui.separator();
                    if ui.button("Quit").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                });
                ui.menu_button("Edit", |ui| {
                    let u = self.ed.undo_label().map(|l| format!("Undo {l}")).unwrap_or("Undo".into());
                    if ui.add_enabled(self.ed.undo_label().is_some(), egui::Button::new(u).shortcut_text("Ctrl+Z")).clicked() { self.ed.undo(); ui.close_menu(); }
                    let r = self.ed.redo_label().map(|l| format!("Redo {l}")).unwrap_or("Redo".into());
                    if ui.add_enabled(self.ed.redo_label().is_some(), egui::Button::new(r).shortcut_text("Ctrl+Shift+Z")).clicked() { self.ed.redo(); ui.close_menu(); }
                    ui.separator();
                    if ui.add_enabled(self.ed.selection.is_some(), egui::Button::new("Delete").shortcut_text("Del")).clicked() { self.ed.delete_selected(); ui.close_menu(); }
                    if ui.add(egui::Button::new("Deselect All").shortcut_text("Esc")).clicked() { self.ed.selection = None; ui.close_menu(); }
                    if ui.add_enabled(self.ed.editing.is_some(), egui::Button::new("Edit Document")).clicked() { self.ed.exit_to_scene(); self.fit_pending = true; ui.close_menu(); }
                });
                ui.menu_button("View", |ui| {
                    if ui.add(egui::Button::new("Zoom In").shortcut_text("Ctrl+=")).clicked() { self.zoom *= 1.25; ui.close_menu(); }
                    if ui.add(egui::Button::new("Zoom Out").shortcut_text("Ctrl+-")).clicked() { self.zoom /= 1.25; ui.close_menu(); }
                    if ui.add(egui::Button::new("100%").shortcut_text("Ctrl+1")).clicked() { self.zoom = 1.0; ui.close_menu(); }
                    if ui.add(egui::Button::new("Fit in Window").shortcut_text("Ctrl+2")).clicked() { self.fit_pending = true; ui.close_menu(); }
                });
                ui.menu_button("Insert", |ui| {
                    if ui.add(egui::Button::new("New Symbol…").shortcut_text("Ctrl+F8")).clicked() {
                        self.dialog = Dialog::NewSymbol { name: format!("Symbol {}", self.ed.doc.next_symbol_id), kind: SymbolKind::Graphic };
                        ui.close_menu();
                    }
                    if ui.button("New Layer").clicked() { self.ed.add_layer(); ui.close_menu(); }
                    ui.separator();
                    timeline::frame_menu(self, ui);
                });
                ui.menu_button("Modify", |ui| {
                    if ui.add_enabled(self.ed.selection.is_some(), egui::Button::new("Convert to Symbol…").shortcut_text("F8")).clicked() {
                        self.dialog = Dialog::ConvertSymbol { name: format!("Symbol {}", self.ed.doc.next_symbol_id), kind: SymbolKind::Graphic };
                        ui.close_menu();
                    }
                    ui.menu_button("Arrange", |ui| {
                        if ui.button("Bring to Front").clicked() { self.ed.arrange(true); ui.close_menu(); }
                        if ui.button("Send to Back").clicked() { self.ed.arrange(false); ui.close_menu(); }
                    });
                    ui.menu_button("Transform", |ui| {
                        if ui.button("Flip Horizontal").clicked() { self.ed.modify_selected("Flip Horizontal", |e| e.matrix = e.matrix.concat(&Matrix::scale(-1.0, 1.0))); ui.close_menu(); }
                        if ui.button("Flip Vertical").clicked() { self.ed.modify_selected("Flip Vertical", |e| e.matrix = e.matrix.concat(&Matrix::scale(1.0, -1.0))); ui.close_menu(); }
                        if ui.button("Rotate 90° CW").clicked() { self.ed.modify_selected("Rotate", |e| e.matrix = e.matrix.concat(&Matrix::rotate(std::f32::consts::FRAC_PI_2))); ui.close_menu(); }
                        if ui.button("Remove Transform").clicked() { self.ed.modify_selected("Remove Transform", |e| { let (x, y) = (e.matrix.tx, e.matrix.ty); e.matrix = Matrix::translate(x, y); }); ui.close_menu(); }
                    });
                });
                ui.menu_button("Control", |ui| {
                    if ui.add(egui::Button::new(if self.playing { "Stop" } else { "Play" }).shortcut_text("Enter")).clicked() { self.toggle_play(); ui.close_menu(); }
                    if ui.button("Rewind").clicked() { self.ed.set_frame(0); ui.close_menu(); }
                    if ui.button("Go to End").clicked() { let n = self.ed.frame_count(); self.ed.set_frame(n - 1); ui.close_menu(); }
                    if ui.add(egui::Button::new("Step Forward One Frame").shortcut_text(".")).clicked() { let n = self.ed.frame_count(); self.ed.set_frame((self.ed.frame + 1).min(n - 1)); ui.close_menu(); }
                    if ui.add(egui::Button::new("Step Backward One Frame").shortcut_text(",")).clicked() { self.ed.set_frame(self.ed.frame.saturating_sub(1)); ui.close_menu(); }
                    ui.separator();
                    ui.checkbox(&mut self.loop_play, "Loop Playback");
                    ui.checkbox(&mut self.onion, "Onion Skin");
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About AnimateCraft").clicked() { self.dialog = Dialog::About; ui.close_menu(); }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Essentials").color(Color32::from_gray(150)));
                });
            });
        });
    }

    fn tools_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools").exact_width(64.0).resizable(false).show(ctx, |ui| {
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                for t in Tool::ALL {
                    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 34.0), Sense::click());
                    let active = self.tool == t;
                    let bg = if active { Color32::from_gray(0x1E) } else if resp.hovered() { Color32::from_gray(0x44) } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(rect, 4.0, bg);
                    draw_tool_icon(ui.painter(), rect, t, if active { ACCENT } else { Color32::from_gray(210) });
                    if resp.on_hover_text(t.name()).clicked() {
                        self.tool = t;
                    }
                }
                ui.separator();
                ui.label(egui::RichText::new("Stroke").small());
                ui.color_edit_button_srgba(&mut self.stroke);
                ui.checkbox(&mut self.stroke_on, "").on_hover_text("Stroke on/off");
                ui.add(egui::DragValue::new(&mut self.stroke_w).range(0.1..=50.0).speed(0.1)).on_hover_text("Stroke width");
                ui.label(egui::RichText::new("Fill").small());
                ui.color_edit_button_srgba(&mut self.fill);
                ui.checkbox(&mut self.fill_on, "").on_hover_text("Fill on/off");
                if ui.small_button("⇄").on_hover_text("Swap colors (X)").clicked() {
                    std::mem::swap(&mut self.fill, &mut self.stroke);
                }
            });
        });
    }

    fn right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("props").default_width(270.0).min_width(230.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new(egui::RichText::new("Properties").strong()).default_open(true).show(ui, |ui| self.properties(ui));
                ui.add_space(4.0);
                egui::CollapsingHeader::new(egui::RichText::new("Library").strong()).default_open(true).show(ui, |ui| self.library(ui));
            });
        });
    }

    fn prop<F: FnOnce(&mut Element)>(&mut self, resp: &egui::Response, label: &str, f: F) {
        if !resp.changed() {
            if resp.drag_stopped() { self.prop_dragging = false; }
            return;
        }
        if resp.dragged() {
            if !self.prop_dragging { self.ed.begin_drag(); self.prop_dragging = true; }
            self.ed.modify_selected_continuous(f);
        } else {
            self.ed.modify_selected(label, f);
        }
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        if let Some(el) = self.ed.selected_element() {
            let kind = match &el.kind {
                ElementKind::Shape(_) => "Shape (Drawing Object)".to_string(),
                ElementKind::Instance(i) => {
                    let s = self.ed.doc.symbol(i.symbol);
                    format!("{} instance of “{}”", s.map(|s| format!("{:?}", s.kind)).unwrap_or_default(), s.map(|s| s.name.as_str()).unwrap_or("?"))
                }
            };
            ui.label(egui::RichText::new(kind).color(Color32::from_gray(170)));
            let d = el.matrix.decompose();
            egui::Grid::new("xform").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                // Like Animate, X/Y are the top-left of the bounding box.
                let (lo, _) = self.ed.selection_bounds().unwrap_or((pt(d.tx, d.ty), pt(d.tx, d.ty)));
                let (mut x, mut y) = (lo.x, lo.y);
                ui.label("X");
                let r = ui.add(egui::DragValue::new(&mut x).speed(1.0).max_decimals(1));
                self.prop(&r, "Move", |e| e.matrix.tx += x - lo.x);
                ui.label("Y");
                let r = ui.add(egui::DragValue::new(&mut y).speed(1.0).max_decimals(1));
                self.prop(&r, "Move", |e| e.matrix.ty += y - lo.y);
                ui.end_row();
                let (mut sx, mut sy) = (d.scale_x * 100.0, d.scale_y * 100.0);
                ui.label("W%");
                let r = ui.add(egui::DragValue::new(&mut sx).speed(0.5).suffix("%").max_decimals(1));
                self.prop(&r, "Scale", |e| { let mut dd = e.matrix.decompose(); dd.scale_x = sx / 100.0; e.matrix = Matrix::compose(&dd); });
                ui.label("H%");
                let r = ui.add(egui::DragValue::new(&mut sy).speed(0.5).suffix("%").max_decimals(1));
                self.prop(&r, "Scale", |e| { let mut dd = e.matrix.decompose(); dd.scale_y = sy / 100.0; e.matrix = Matrix::compose(&dd); });
                ui.end_row();
                let mut rot = d.skew_y.to_degrees();
                ui.label("Rot");
                let r = ui.add(egui::DragValue::new(&mut rot).speed(0.5).suffix("°").max_decimals(1));
                self.prop(&r, "Rotate", |e| {
                    let mut dd = e.matrix.decompose();
                    let delta = rot.to_radians() - dd.skew_y;
                    dd.skew_y += delta;
                    dd.skew_x += delta;
                    e.matrix = Matrix::compose(&dd);
                });
                let mut alpha = el.alpha * 100.0;
                ui.label("Alpha");
                let r = ui.add(egui::DragValue::new(&mut alpha).range(0.0..=100.0).speed(0.5).suffix("%").max_decimals(0));
                self.prop(&r, "Color Effect", |e| e.alpha = alpha / 100.0);
                ui.end_row();
            });
            match &el.kind {
                ElementKind::Shape(s) => {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Fill");
                        let mut c = s.fill.map(c32).unwrap_or(Color32::TRANSPARENT);
                        if ui.color_edit_button_srgba(&mut c).changed() { let col = from32(c); self.ed.modify_selected("Fill Color", |e| if let ElementKind::Shape(s) = &mut e.kind { s.fill = Some(col) }); }
                        ui.label("Stroke");
                        let mut sc = s.stroke.map(|s| c32(s.color)).unwrap_or(Color32::TRANSPARENT);
                        if ui.color_edit_button_srgba(&mut sc).changed() {
                            let col = from32(sc);
                            self.ed.modify_selected("Stroke Color", |e| if let ElementKind::Shape(s) = &mut e.kind { s.stroke = Some(Stroke { color: col, width: s.stroke.map(|x| x.width).unwrap_or(1.0) }) });
                        }
                    });
                    if let Some(st) = s.stroke {
                        let mut w = st.width;
                        ui.horizontal(|ui| {
                            ui.label("Stroke size");
                            let r = ui.add(egui::DragValue::new(&mut w).range(0.1..=100.0).speed(0.1));
                            self.prop(&r, "Stroke Size", |e| if let ElementKind::Shape(s) = &mut e.kind { if let Some(st) = &mut s.stroke { st.width = w } });
                        });
                    }
                }
                ElementKind::Instance(inst) => {
                    ui.separator();
                    let id = inst.symbol;
                    let is_graphic = self.ed.doc.symbol(id).map(|s| s.kind == SymbolKind::Graphic).unwrap_or(false);
                    if is_graphic {
                        ui.label(egui::RichText::new("Looping").strong());
                        let mut mode = inst.loop_mode;
                        ui.horizontal(|ui| {
                            for (m, n) in [(LoopMode::Loop, "Loop"), (LoopMode::PlayOnce, "Play Once"), (LoopMode::SingleFrame, "Single Frame")] {
                                if ui.selectable_value(&mut mode, m, n).changed() {
                                    self.ed.modify_selected("Looping", |e| if let ElementKind::Instance(i) = &mut e.kind { i.loop_mode = mode });
                                }
                            }
                        });
                        let mut ff = inst.first_frame + 1;
                        ui.horizontal(|ui| {
                            ui.label("First frame");
                            let r = ui.add(egui::DragValue::new(&mut ff).range(1..=999));
                            self.prop(&r, "First Frame", |e| if let ElementKind::Instance(i) = &mut e.kind { i.first_frame = ff - 1 });
                        });
                    }
                    let mut tint = el.tint.map(|(c, _)| c32(c)).unwrap_or(Color32::WHITE);
                    let mut amt = el.tint.map(|(_, a)| a * 100.0).unwrap_or(0.0);
                    ui.horizontal(|ui| {
                        ui.label("Tint");
                        if ui.color_edit_button_srgba(&mut tint).changed() { let c = from32(tint); let a = amt / 100.0; self.ed.modify_selected("Tint", |e| e.tint = Some((c, a.max(0.5)))); }
                        let r = ui.add(egui::DragValue::new(&mut amt).range(0.0..=100.0).suffix("%"));
                        let c = from32(tint);
                        self.prop(&r, "Tint", |e| e.tint = if amt <= 0.0 { None } else { Some((c, amt / 100.0)) });
                    });
                    if ui.button("Edit Symbol").clicked() { self.ed.enter_symbol(id); self.fit_pending = true; }
                }
            }
            return;
        }

        // Frame properties
        if let Some((start, kf)) = self.ed.current_keyframe() {
            let kf = kf.clone();
            ui.label(egui::RichText::new(format!("Frame {} (keyframe at {})", self.ed.frame + 1, start + 1)).color(Color32::from_gray(170)));
            let mut label = kf.label.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Label");
                if ui.text_edit_singleline(&mut label).lost_focus() && label != kf.label.clone().unwrap_or_default() {
                    self.ed.set_frame_label(&label);
                }
            });
            let mut kind = match kf.tween { Tween::None => 0, Tween::Classic { .. } => 1, Tween::Shape { .. } => 2 };
            let before = kind;
            ui.horizontal(|ui| {
                ui.label("Tween");
                egui::ComboBox::from_id_salt("tween").selected_text(["None", "Classic", "Shape"][kind]).show_ui(ui, |ui| {
                    ui.selectable_value(&mut kind, 0, "None");
                    ui.selectable_value(&mut kind, 1, "Classic");
                    ui.selectable_value(&mut kind, 2, "Shape");
                });
            });
            let mut ease = kf.tween.ease();
            let mut turns = if let Tween::Classic { rotate_turns, .. } = kf.tween { rotate_turns } else { 0 };
            let mut changed = kind != before;
            if kind != 0 {
                ui.horizontal(|ui| {
                    ui.label("Ease");
                    changed |= ui.add(egui::Slider::new(&mut ease, -100.0..=100.0).step_by(1.0)).changed();
                });
                ui.label(egui::RichText::new("−100 ease in · +100 ease out").small().color(Color32::from_gray(140)));
            }
            if kind == 1 {
                ui.horizontal(|ui| {
                    ui.label("Rotate");
                    changed |= ui.add(egui::DragValue::new(&mut turns).range(-10..=10).suffix(" × CW")).changed();
                });
            }
            if changed {
                self.ed.set_tween(match kind {
                    1 => Tween::Classic { ease, rotate_turns: turns },
                    2 => Tween::Shape { ease },
                    _ => Tween::None,
                });
            }
            ui.separator();
        }

        // Document properties
        let d = &self.ed.doc;
        let (mut w, mut h, mut fps, mut bg) = (d.width, d.height, d.fps, c32(d.background));
        ui.label(egui::RichText::new("Document").strong());
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
            self.ed.set_doc_props(w, h, fps, from32(bg));
        }
    }

    fn library(&mut self, ui: &mut egui::Ui) {
        // Preview of the selected symbol (first frame), like Animate's library preview pane.
        let (prev_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 110.0), Sense::hover());
        ui.painter().rect_filled(prev_rect, 3.0, Color32::from_gray(0x26));
        if let Some(sym) = self.lib_selected.and_then(|id| self.ed.doc.symbol(id)) {
            let ppp = ui.ctx().pixels_per_point();
            let (pw, ph) = ((prev_rect.width() * ppp) as u32, (prev_rect.height() * ppp) as u32);
            if let Some(mut pix) = sk::Pixmap::new(pw.max(1), ph.max(1)) {
                let ctx = ac_core::EvalCtx { doc: &self.ed.doc, global_frame: 0 };
                let mut items = Vec::new();
                ac_core::evaluate(&ctx, &sym.timeline, 0, &Matrix::IDENTITY, 1.0, 1, &mut items);
                let bounds = items.iter().filter_map(|it| it.path.transformed(&it.matrix).bounds()).reduce(|a, b| (pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)), pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y))));
                if let Some((lo, hi)) = bounds {
                    let s = ((pw as f32 - 16.0) / (hi.x - lo.x).max(1.0)).min((ph as f32 - 16.0) / (hi.y - lo.y).max(1.0)).min(4.0 * ppp);
                    let view = Matrix::translate(pw as f32 / 2.0, ph as f32 / 2.0).concat(&Matrix::scale(s, s)).concat(&Matrix::translate(-(lo.x + hi.x) / 2.0, -(lo.y + hi.y) / 2.0));
                    ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view, ..Default::default() });
                }
                let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pix.data());
                let tex = match &mut self.lib_tex {
                    Some(t) => { t.set(img, egui::TextureOptions::LINEAR); t.clone() }
                    None => { let t = ui.ctx().load_texture("lib", img, egui::TextureOptions::LINEAR); self.lib_tex = Some(t.clone()); t }
                };
                ui.painter().image(tex.id(), prev_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        } else {
            ui.painter().text(prev_rect.center(), Align2::CENTER_CENTER, "No symbol selected", FontId::proportional(12.0), Color32::from_gray(120));
        }
        ui.label(egui::RichText::new(format!("{} items", self.ed.doc.library.len())).small().color(Color32::from_gray(150)));
        let syms: Vec<(u32, String, SymbolKind, u32)> = self.ed.doc.library.iter().map(|s| (s.id, s.name.clone(), s.kind, s.timeline.frame_count())).collect();
        for (id, name, kind, frames) in syms {
            let icon = match kind { SymbolKind::Graphic => "🔷", SymbolKind::MovieClip => "🎞", SymbolKind::Button => "🔘" };
            let resp = ui.selectable_label(self.lib_selected == Some(id), format!("{icon}  {name}   ({frames}f)"));
            if resp.clicked() { self.lib_selected = Some(id); }
            if resp.double_clicked() { self.ed.enter_symbol(id); self.fit_pending = true; }
            resp.context_menu(|ui| {
                if ui.button("Edit").clicked() { self.ed.enter_symbol(id); self.fit_pending = true; ui.close_menu(); }
                if ui.button("Place on Stage").clicked() {
                    let c = if self.ed.editing.is_some() { pt(0.0, 0.0) } else { pt(self.ed.doc.width / 2.0, self.ed.doc.height / 2.0) };
                    self.ed.place_symbol(id, c);
                    ui.close_menu();
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(self.lib_selected.is_some(), egui::Button::new("Place on Stage")).clicked() {
                let c = if self.ed.editing.is_some() { pt(0.0, 0.0) } else { pt(self.ed.doc.width / 2.0, self.ed.doc.height / 2.0) };
                self.ed.place_symbol(self.lib_selected.unwrap(), c);
            }
            if ui.button("New Symbol…").clicked() {
                self.dialog = Dialog::NewSymbol { name: format!("Symbol {}", self.ed.doc.next_symbol_id), kind: SymbolKind::Graphic };
            }
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").exact_height(22.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(&self.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{:.0}%", self.zoom * 100.0)).small());
                    ui.label(egui::RichText::new(self.tool.name()).small().color(Color32::from_gray(150)));
                });
            });
        });
    }

    // ------------------------------------------------------------ stage
    fn stage(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_gray(0x3C))).show(ctx, |ui| {
            // Edit bar (breadcrumb), like Animate's scene/symbol bar.
            egui::Frame::none().fill(Color32::from_gray(0x2E)).inner_margin(egui::Margin::symmetric(8.0, 4.0)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let crumbs = self.ed.breadcrumb();
                    for (i, c) in crumbs.iter().enumerate() {
                        if i > 0 { ui.label("›"); }
                        let icon = if i == 0 { "🎬" } else { "🔷" };
                        if ui.link(format!("{icon} {c}")).clicked() && i == 0 && self.ed.editing.is_some() {
                            self.ed.exit_to_scene();
                            self.fit_pending = true;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Fit").clicked() { self.fit_pending = true; }
                        ui.label(format!("{:.0}%", self.zoom * 100.0));
                    });
                });
            });
            let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            if self.fit_pending {
                self.fit(rect);
                self.fit_pending = false;
            }
            self.stage_input(ui, rect, &resp);
            self.render_stage(ui, rect);
            self.stage_overlays(ui, rect);
        });
    }

    fn stage_input(&mut self, ui: &egui::Ui, rect: Rect, resp: &egui::Response) {
        // Zoom with wheel (Ctrl or plain), anchored at cursor.
        if let Some(hover) = resp.hover_pos() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                self.zoom_at(rect, hover, if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 });
            }
        }
        let space = ui.input(|i| i.key_down(Key::Space));
        let tool = if space { Tool::Hand } else { self.tool };
        let Some(pos) = resp.interact_pointer_pos() else { return };
        let p = self.to_doc(rect, pos);
        if resp.secondary_clicked() {
            self.ed.selection = self.ed.hit_test(p);
            return;
        }
        if resp.drag_started() || (resp.clicked() && matches!(self.drag, Drag::None)) {
            self.playing = false;
            match tool {
                Tool::Hand => self.drag = Drag::Pan { last: pos },
                Tool::Select | Tool::FreeTransform => {
                    if let Some(h) = self.handle_hit(rect, pos) {
                        let sel = self.ed.selected_element().unwrap();
                        let (lo, hi) = self.ed.selection_bounds().unwrap();
                        self.ed.begin_drag();
                        if tool == Tool::FreeTransform && h.1 {
                            let c = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
                            self.drag = Drag::Rotate { center: c, start_angle: (p.y - c.y).atan2(p.x - c.x), base: sel.matrix };
                        } else {
                            let corners = [pt(hi.x, hi.y), pt(lo.x, hi.y), pt(hi.x, lo.y), pt(lo.x, lo.y)];
                            self.drag = Drag::Scale { anchor: corners[h.0], start: p, base: sel.matrix };
                        }
                    } else {
                        self.ed.selection = self.ed.hit_test(p);
                        if self.ed.selection.is_some() {
                            self.ed.begin_drag();
                            self.drag = Drag::Move { last: p };
                        }
                    }
                }
                Tool::Rect | Tool::Oval | Tool::Line => self.drag = Drag::Draw { start: p, cur: p },
                Tool::Pencil => self.drag = Drag::Pencil { pts: vec![p] },
                Tool::Bucket => {
                    if let Some(s) = self.ed.hit_test(p) {
                        self.ed.paint_bucket(s, from32(self.fill));
                    }
                }
            }
        }
        if resp.dragged() {
            let shift = ui.input(|i| i.modifiers.shift);
            match &mut self.drag {
                Drag::Pan { last } => {
                    self.pan += pos - *last;
                    *last = pos;
                }
                Drag::Move { last } => {
                    let d = pt(p.x - last.x, p.y - last.y);
                    *last = p;
                    self.ed.modify_selected_continuous(|e| { e.matrix.tx += d.x; e.matrix.ty += d.y; });
                }
                Drag::Scale { anchor, start, base } => {
                    let (a, s, b) = (*anchor, *start, *base);
                    let mut sx = if (s.x - a.x).abs() > 0.5 { (p.x - a.x) / (s.x - a.x) } else { 1.0 };
                    let mut sy = if (s.y - a.y).abs() > 0.5 { (p.y - a.y) / (s.y - a.y) } else { 1.0 };
                    if shift { let u = sx.abs().max(sy.abs()); sx = u * sx.signum(); sy = u * sy.signum(); }
                    let m = Matrix::translate(a.x, a.y).concat(&Matrix::scale(sx, sy)).concat(&Matrix::translate(-a.x, -a.y)).concat(&b);
                    self.ed.modify_selected_continuous(|e| e.matrix = m);
                }
                Drag::Rotate { center, start_angle, base } => {
                    let (c, sa, b) = (*center, *start_angle, *base);
                    let mut ang = (p.y - c.y).atan2(p.x - c.x) - sa;
                    if shift { let step = std::f32::consts::PI / 4.0; ang = (ang / step).round() * step; }
                    let m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(ang)).concat(&Matrix::translate(-c.x, -c.y)).concat(&b);
                    self.ed.modify_selected_continuous(|e| e.matrix = m);
                }
                Drag::Draw { start, cur } => {
                    *cur = p;
                    if shift && self.tool != Tool::Line {
                        let s = (p.x - start.x).abs().max((p.y - start.y).abs());
                        *cur = pt(start.x + s * (p.x - start.x).signum(), start.y + s * (p.y - start.y).signum());
                    }
                }
                Drag::Pencil { pts } => {
                    if pts.last().map(|l| l.dist(p) > 1.5 / self.zoom).unwrap_or(true) {
                        pts.push(p);
                    }
                }
                Drag::None => {}
            }
        }
        if resp.drag_stopped() || (resp.clicked() && !matches!(self.drag, Drag::Pan { .. })) {
            let drag = std::mem::replace(&mut self.drag, Drag::None);
            match drag {
                Drag::Draw { start, cur } if start.dist(cur) > 2.0 => {
                    let (x0, y0, x1, y1) = (start.x.min(cur.x), start.y.min(cur.y), start.x.max(cur.x), start.y.max(cur.y));
                    let (name, path, fill) = match self.tool {
                        Tool::Rect => ("Rectangle", Path::rect(x0, y0, x1 - x0, y1 - y0), self.cur_fill()),
                        Tool::Oval => ("Oval", Path::ellipse((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0), self.cur_fill()),
                        _ => ("Line", Path { segs: vec![ac_core::Seg::M(start), ac_core::Seg::L(cur)] }, None),
                    };
                    let stroke = if self.tool == Tool::Line { Some(Stroke { color: from32(self.stroke), width: self.stroke_w }) } else { self.cur_stroke() };
                    if fill.is_some() || stroke.is_some() {
                        if self.ed.add_element(Element::shape(name, Shape { path, fill, stroke })).is_none() {
                            self.status = "Layer is locked or hidden.".into();
                        }
                    }
                }
                Drag::Pencil { pts } if pts.len() > 1 => {
                    let path = Path::smooth_polyline(&pts, 1.2 / self.zoom);
                    let stroke = Some(Stroke { color: from32(self.stroke), width: self.stroke_w });
                    if self.ed.add_element(Element::shape("Pencil", Shape { path, fill: None, stroke })).is_none() {
                        self.status = "Layer is locked or hidden.".into();
                    }
                }
                _ => {}
            }
        }
        if resp.drag_stopped() {
            self.drag = Drag::None;
        }
    }

    /// Which selection corner handle is under `pos`: (corner index, outside-corner rotate zone).
    fn handle_hit(&self, rect: Rect, pos: Pos2) -> Option<(usize, bool)> {
        let (lo, hi) = self.ed.selection_bounds()?;
        let corners = [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(lo.x, hi.y), pt(hi.x, hi.y)];
        for (i, c) in corners.iter().enumerate() {
            let d = self.to_screen(rect, *c).distance(pos);
            if d < 6.0 {
                return Some((i, false));
            }
            if self.tool == Tool::FreeTransform && d < 22.0 {
                return Some((i, true));
            }
        }
        None
    }

    fn render_stage(&mut self, ui: &egui::Ui, rect: Rect) {
        let ppp = ui.ctx().pixels_per_point();
        let (pw, ph) = ((rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32);
        let Some(mut pix) = sk::Pixmap::new(pw.max(1), ph.max(1)) else { return };
        pix.fill(sk::Color::from_rgba8(0x3C, 0x3C, 0x3C, 255));
        let o = self.content_offset();
        let view = Matrix::scale(ppp, ppp).concat(&Matrix::translate(self.pan.x, self.pan.y)).concat(&Matrix::scale(self.zoom, self.zoom));
        // Stage rectangle with a soft shadow.
        let d = &self.ed.doc;
        let stage_dim = self.ed.editing.is_some();
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
        let n = self.ed.frame_count();
        if self.onion && !self.playing {
            for k in (1..=self.onion_n).rev() {
                let fade = 0.45 / k as f32;
                if self.ed.frame >= k {
                    let items = self.ed.evaluate_current(self.ed.frame - k);
                    ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: content_view, alpha: fade, tint: Some((Color::rgb(0x30, 0x60, 0xFF), 0.85)) });
                }
                if self.ed.frame + k < n {
                    let items = self.ed.evaluate_current(self.ed.frame + k);
                    ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: content_view, alpha: fade, tint: Some((Color::rgb(0x20, 0xC0, 0x40), 0.85)) });
                }
            }
        }
        let items = self.ed.evaluate_current(self.ed.frame);
        ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: content_view, ..Default::default() });

        let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pix.data());
        let tex = match &mut self.stage_tex {
            Some(t) => {
                t.set(img, egui::TextureOptions::NEAREST);
                t.clone()
            }
            None => {
                let t = ui.ctx().load_texture("stage", img, egui::TextureOptions::NEAREST);
                self.stage_tex = Some(t.clone());
                t
            }
        };
        ui.painter_at(rect).image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }

    fn stage_overlays(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        // Symbol-edit registration crosshair.
        if self.ed.editing.is_some() {
            let c = self.to_screen(rect, pt(0.0, 0.0));
            let s = EStroke::new(1.0_f32, Color32::from_gray(20));
            painter.line_segment([c - vec2(9.0, 0.0), c + vec2(9.0, 0.0)], s);
            painter.line_segment([c - vec2(0.0, 9.0), c + vec2(0.0, 9.0)], s);
        }
        // Selection box + handles + registration point.
        if let Some((lo, hi)) = self.ed.selection_bounds() {
            let r = Rect::from_two_pos(self.to_screen(rect, lo), self.to_screen(rect, hi)).expand(2.0);
            painter.rect_stroke(r, 0.0, EStroke::new(1.0_f32, ACCENT));
            for c in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
                painter.rect_filled(Rect::from_center_size(c, vec2(7.0, 7.0)), 0.0, Color32::WHITE);
                painter.rect_stroke(Rect::from_center_size(c, vec2(7.0, 7.0)), 0.0, EStroke::new(1.0_f32, ACCENT));
            }
            if self.tool == Tool::FreeTransform {
                painter.circle_stroke(r.center(), 4.0, EStroke::new(1.5_f32, Color32::WHITE));
            }
            if let Some(el) = self.ed.selected_element() {
                if matches!(el.kind, ElementKind::Instance(_)) {
                    let reg = self.to_screen(rect, pt(el.matrix.tx, el.matrix.ty));
                    painter.circle_filled(reg, 3.5, Color32::WHITE);
                    painter.circle_stroke(reg, 3.5, EStroke::new(1.0_f32, Color32::BLACK));
                }
            }
        }
        // Live drawing previews.
        let preview = EStroke::new(1.0_f32, ACCENT);
        match &self.drag {
            Drag::Draw { start, cur } => {
                let (a, b) = (self.to_screen(rect, *start), self.to_screen(rect, *cur));
                match self.tool {
                    Tool::Rect => { painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, preview); }
                    Tool::Oval => {
                        let r = Rect::from_two_pos(a, b);
                        let pts: Vec<Pos2> = (0..=48).map(|i| { let t = i as f32 / 48.0 * std::f32::consts::TAU; r.center() + vec2(t.cos() * r.width() / 2.0, t.sin() * r.height() / 2.0) }).collect();
                        painter.add(egui::Shape::line(pts, preview));
                    }
                    _ => { painter.line_segment([a, b], EStroke::new(self.stroke_w * self.zoom, self.stroke)); }
                }
            }
            Drag::Pencil { pts } => {
                let s: Vec<Pos2> = pts.iter().map(|p| self.to_screen(rect, *p)).collect();
                painter.add(egui::Shape::line(s, EStroke::new(self.stroke_w * self.zoom, self.stroke)));
            }
            _ => {}
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut d = std::mem::replace(&mut self.dialog, Dialog::None);
        let mut close = false;
        let mut run: Option<(FileAction, String)> = None;
        let mut symbol_ok: Option<(bool, String, SymbolKind)> = None;
        match &mut d {
            Dialog::None => {}
            Dialog::Path { title, path, action } => {
                egui::Window::new(title.as_str()).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                    ui.label("File path:");
                    let r = ui.add(egui::TextEdit::singleline(path).desired_width(360.0));
                    r.request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                            run = Some((*action, path.clone()));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() { close = true; }
                    });
                });
            }
            Dialog::ConvertSymbol { .. } | Dialog::NewSymbol { .. } => {
                let convert = matches!(d, Dialog::ConvertSymbol { .. });
                let (Dialog::ConvertSymbol { name, kind } | Dialog::NewSymbol { name, kind }) = &mut d else { unreachable!() };
                egui::Window::new(if convert { "Convert to Symbol" } else { "Create New Symbol" }).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                    egui::Grid::new("sym").show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label("Type");
                        ui.horizontal(|ui| {
                            ui.selectable_value(kind, SymbolKind::Graphic, "Graphic");
                            ui.selectable_value(kind, SymbolKind::MovieClip, "Movie Clip");
                            ui.selectable_value(kind, SymbolKind::Button, "Button");
                        });
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() { symbol_ok = Some((convert, name.clone(), *kind)); close = true; }
                        if ui.button("Cancel").clicked() { close = true; }
                    });
                });
            }
            Dialog::About => {
                egui::Window::new("About AnimateCraft").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                    ui.heading("AnimateCraft 0.1 (demo)");
                    ui.label("Open-source 2D animation authoring in Rust, modelled on the Adobe Animate workflow.");
                    ui.label("Not affiliated with Adobe. Licensed MIT OR Apache-2.0.");
                    if ui.button("Close").clicked() { close = true; }
                });
            }
        }
        if let Some((convert, n, k)) = symbol_ok {
            if convert {
                if let Some(id) = self.ed.convert_to_symbol(&n, k) { self.lib_selected = Some(id); self.status = format!("Converted to symbol “{n}”"); }
            } else {
                let id = self.ed.new_symbol(&n, k);
                self.lib_selected = Some(id);
                self.fit_pending = true;
                self.status = format!("Editing new symbol “{n}”. Press Esc to return to the scene.");
            }
        }
        if !close { self.dialog = d; }
        if let Some((a, p)) = run { self.do_file(a, &p); }
    }
}

fn to_ts(m: &Matrix) -> sk::Transform {
    sk::Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty)
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
        Tool::Rect => { p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0)), 0.0, s); }
        Tool::Oval => { p.circle_stroke(c, 8.0, s); }
        Tool::Line => { p.line_segment([c + vec2(-8.0, 8.0), c + vec2(8.0, -8.0)], s); }
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.shortcuts(ctx);
        if self.playing {
            let dt = ctx.input(|i| i.stable_dt) as f64;
            self.play_accum += dt;
            let spf = 1.0 / self.ed.doc.fps as f64;
            let n = self.ed.frame_count();
            while self.play_accum >= spf {
                self.play_accum -= spf;
                if self.ed.frame + 1 >= n {
                    if self.loop_play { self.ed.frame = 0; } else { self.playing = false; break; }
                } else {
                    self.ed.frame += 1;
                }
            }
            ctx.request_repaint();
        }
        self.menu_bar(ctx);
        self.status_bar(ctx);
        timeline::panel(self, ctx);
        self.tools_panel(ctx);
        self.right_panel(ctx);
        self.stage(ctx);
        self.dialogs(ctx);

        // Screenshot hook (docs/CI): wait a few frames for layout to settle.
        if let Some((path, n)) = &mut self.screenshot {
            *n += 1;
            if *n == 6 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            let shot = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
            if let Some(img) = shot {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(pm) = sk::Pixmap::from_vec(bytes, sk::IntSize::from_wh(w as u32, h as u32).unwrap()) {
                    let _ = pm.save_png(path.as_str());
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1440.0, 880.0]).with_min_inner_size([900.0, 600.0]).with_title("AnimateCraft"),
        ..Default::default()
    };
    eframe::run_native("AnimateCraft", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
