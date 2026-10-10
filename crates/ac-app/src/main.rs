//! AnimateCraft desktop editor. Layout follows Adobe Animate's default
//! "Essentials" workspace: menu bar, Tools (left), Stage (centre),
//! Properties + Library (right), Timeline (bottom).

mod panels;
mod stage;
mod timeline;

use ac_core::{demo::demo_document, Color, Document, Editor, ElementKind, Matrix, Pt, SymbolId, SymbolKind};
use eframe::egui::{self, vec2, Align2, Color32, Key, Modifiers, Rect, Stroke as EStroke, Vec2};

pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x2D, 0x8C, 0xEB);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Tool {
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
    pub(crate) const ALL: [Tool; 8] = [Tool::Select, Tool::FreeTransform, Tool::Rect, Tool::Oval, Tool::Line, Tool::Pencil, Tool::Bucket, Tool::Hand];
    pub(crate) fn name(self) -> &'static str {
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

pub(crate) enum Dialog {
    None,
    Path { title: String, path: String, action: FileAction },
    Symbol { convert: bool, name: String, kind: SymbolKind },
    RenameSymbol { id: SymbolId, name: String },
    DeleteSymbol { id: SymbolId, uses: usize },
    About,
}

#[derive(Clone, Copy)]
pub(crate) enum FileAction {
    Open,
    Save,
    ExportGif,
    ExportPng,
    ExportSvg,
}

pub struct App {
    ctx: egui::Context,
    pub(crate) ed: Editor,
    pub(crate) tool: Tool,
    pub(crate) fill: Color32,
    pub(crate) fill_on: bool,
    pub(crate) stroke: Color32,
    pub(crate) stroke_on: bool,
    pub(crate) stroke_w: f32,
    pub(crate) zoom: f32,
    pub(crate) pan: Vec2,
    pub(crate) fit_pending: bool,
    pub(crate) playing: bool,
    pub(crate) loop_play: bool,
    play_accum: f64,
    pub(crate) onion: bool,
    pub(crate) onion_n: u32,
    pub(crate) stage_tex: Option<egui::TextureHandle>,
    pub(crate) lib_tex: Option<egui::TextureHandle>,
    pub(crate) lib_selected: Option<SymbolId>,
    /// Library item being dragged towards the stage.
    pub(crate) lib_drag: Option<SymbolId>,
    pub(crate) stage_rect: Rect,
    pub(crate) drag: stage::Drag,
    pub(crate) prop_dragging: bool,
    pub(crate) dialog: Dialog,
    pub(crate) status: String,
    file_path: Option<String>,
    pub(crate) tl: timeline::TimelineState,
    screenshot: Option<(String, u32)>,
}

pub(crate) fn c32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}
pub(crate) fn from32(c: Color32) -> Color {
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
            ctx: cc.egui_ctx.clone(),
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
            lib_drag: None,
            stage_rect: Rect::NOTHING,
            drag: stage::Drag::None,
            prop_dragging: false,
            dialog: Dialog::None,
            status: "Demo loaded. Enter = play · click frames to select · Delete removes the selection · right-click for menus".into(),
            file_path: None,
            tl: Default::default(),
            screenshot: None,
        };
        // Headless screenshot hook used by CI/docs: AC_SCREENSHOT=out.png [AC_SCREENSHOT_FRAME=n]
        if let Ok(p) = std::env::var("AC_SCREENSHOT") {
            let frame: u32 = std::env::var("AC_SCREENSHOT_FRAME").ok().and_then(|s| s.parse().ok()).unwrap_or(14);
            app.ed.set_frame(frame.saturating_sub(1));
            app.onion = true;
            app.ed.selection = vec![ac_core::Selection { layer: 0, element: 0 }];
            app.lib_selected = Some(1);
            app.screenshot = Some((p, 0));
        }
        app
    }

    // ------------------------------------------------------------ actions
    pub(crate) fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.play_accum = 0.0;
        if self.playing {
            self.ed.selection.clear();
            if self.ed.frame + 1 >= self.ed.frame_count() {
                self.ed.set_frame(0);
            }
        }
    }

    pub(crate) fn step(&mut self, delta: i64) {
        self.playing = false;
        let n = self.ed.frame_count() as i64;
        let f = (self.ed.frame as i64 + delta).clamp(0, (n - 1).max(0)) as u32;
        self.ed.set_frame(f);
        self.ed.frame_sel = None;
    }

    /// Delete key: objects first, otherwise the selected frames.
    pub(crate) fn delete(&mut self) {
        if !self.ed.selection.is_empty() {
            let n = self.ed.delete_selected();
            self.status = if n == 0 { "Selected objects are on a locked or hidden layer.".into() } else { format!("Deleted {n} object{}", if n == 1 { "" } else { "s" }) };
        } else if self.ed.frame_sel.is_some() {
            self.ed.clear_frames();
            self.status = "Cleared frames".into();
        } else {
            self.status = "Nothing selected. Click an object (or drag a box around it), or select frames in the timeline.".into();
        }
    }

    pub(crate) fn copy(&mut self) {
        if !self.ed.selection.is_empty() {
            let n = self.ed.copy_selected();
            self.status = format!("Copied {n} object(s)");
        } else if self.ed.frame_sel.is_some() {
            self.ed.copy_frames();
            self.status = "Copied frames".into();
        }
        self.mark_clipboard();
    }
    /// Put a marker on the system clipboard: platforms only deliver a Paste
    /// event when the system clipboard isn't empty.
    fn mark_clipboard(&self) {
        self.ctx.copy_text("AnimateCraft clipboard".into());
    }
    pub(crate) fn cut(&mut self) {
        if !self.ed.selection.is_empty() {
            self.ed.cut_selected();
            self.status = "Cut".into();
        } else if self.ed.frame_sel.is_some() {
            self.ed.cut_frames();
            self.status = "Cut frames".into();
        }
        self.mark_clipboard();
    }
    pub(crate) fn paste(&mut self, in_place: bool) {
        if self.ed.frame_sel.is_some() && self.ed.can_paste_frames() {
            self.ed.paste_frames();
            self.status = "Pasted frames".into();
        } else if self.ed.can_paste() {
            if self.ed.layer_locked() {
                self.status = "Current layer is locked or hidden.".into();
            } else {
                self.ed.paste(in_place);
                self.status = "Pasted".into();
            }
        } else if self.ed.can_paste_frames() {
            self.ed.paste_frames();
            self.status = "Pasted frames".into();
        }
    }

    pub(crate) fn open_symbol_dialog(&mut self, convert: bool) {
        if convert && self.ed.selection.is_empty() {
            self.status = "Select something on the stage first, then press F8.".into();
            return;
        }
        self.dialog = Dialog::Symbol { convert, name: format!("Symbol {}", self.ed.doc.next_symbol_id), kind: SymbolKind::Graphic };
    }

    pub(crate) fn edit_symbol(&mut self, id: SymbolId) {
        self.ed.enter_symbol(id);
        self.fit_pending = true;
        self.status = "Editing symbol. Press Esc or click the scene name to go back.".into();
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
                format!("Saved {}", std::fs::canonicalize(p).map(|c| c.display().to_string()).unwrap_or(path.into()))
            }),
            FileAction::ExportGif => ac_render::export_gif(&self.ed.doc, 1.0, p).map(|_| format!("Exported animated GIF to {path}")),
            FileAction::ExportPng => ac_render::export_png(&self.ed.doc, self.ed.frame, 2.0, p).map(|_| format!("Exported frame {} to {path}", self.ed.frame + 1)),
            FileAction::ExportSvg => std::fs::write(p, ac_render::export_svg(&self.ed.doc, self.ed.frame)).map_err(|e| e.to_string()).map(|_| format!("Exported SVG to {path}")),
        };
        self.status = res.unwrap_or_else(|e| format!("Error: {e}"));
    }

    pub(crate) fn open_path_dialog(&mut self, title: &str, default: &str, action: FileAction) {
        self.dialog = Dialog::Path { title: title.into(), path: default.into(), action };
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) || !matches!(self.dialog, Dialog::None) {
            return;
        }
        // Copy/cut/paste arrive as events, not key presses, on most platforms.
        let (copy, cut, paste, shift_held) = ctx.input(|i| {
            let mut c = (false, false, false);
            for e in &i.events {
                match e {
                    egui::Event::Copy => c.0 = true,
                    egui::Event::Cut => c.1 = true,
                    egui::Event::Paste(_) => c.2 = true,
                    _ => {}
                }
            }
            (c.0, c.1, c.2, i.modifiers.shift)
        });
        if copy { self.copy(); }
        if cut { self.cut(); }
        if paste { self.paste(shift_held); }

        let cmd = Modifiers::COMMAND;
        let sh = Modifiers::SHIFT;
        let none = Modifiers::NONE;
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        if k(cmd | sh, Key::Z) || k(cmd, Key::Y) {
            if let Some(l) = self.ed.redo() { self.status = format!("Redo {l}"); }
        }
        if k(cmd, Key::Z) {
            if let Some(l) = self.ed.undo() { self.status = format!("Undo {l}"); }
        }
        // Fallbacks in case the platform reports these as plain keys.
        if k(cmd, Key::C) { self.copy(); }
        if k(cmd, Key::X) { self.cut(); }
        if k(cmd | sh, Key::V) { self.paste(true); }
        if k(cmd, Key::V) { self.paste(false); }
        if k(cmd, Key::D) { self.ed.duplicate(); }
        if k(cmd, Key::A) { self.ed.select_all(); }
        if k(cmd, Key::B) { self.ed.break_apart(); }
        if k(cmd, Key::S) {
            let p = self.file_path.clone().unwrap_or_else(|| "untitled.acx.json".into());
            self.do_file(FileAction::Save, &p);
        }
        if k(cmd, Key::O) { self.open_path_dialog("Open document", "untitled.acx.json", FileAction::Open); }
        if k(cmd, Key::Equals) || k(cmd, Key::Plus) { self.zoom *= 1.25; }
        if k(cmd, Key::Minus) { self.zoom /= 1.25; }
        if k(cmd, Key::Num1) { self.zoom = 1.0; }
        if k(cmd, Key::Num2) { self.fit_pending = true; }
        if k(sh, Key::F5) { self.ed.remove_frame(); }
        if k(none, Key::F5) { self.ed.insert_frame(); }
        if k(sh, Key::F6) { self.ed.clear_keyframe(); }
        if k(none, Key::F6) { self.ed.insert_keyframe(); }
        if k(none, Key::F7) { self.ed.insert_blank_keyframe(); }
        if k(cmd, Key::F8) { self.open_symbol_dialog(false); }
        if k(none, Key::F8) { self.open_symbol_dialog(true); }
        if k(none, Key::Enter) { self.toggle_play(); }
        if k(none, Key::Period) { self.step(1); }
        if k(none, Key::Comma) { self.step(-1); }
        if k(none, Key::Delete) || k(none, Key::Backspace) { self.delete(); }
        // Arrow keys nudge the selection (Shift = 10 px).
        for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
            let big = k(sh, key);
            if big || k(none, key) {
                let s = if big { 10.0 } else { 1.0 };
                self.ed.modify_selected("Nudge", |e| { e.matrix.tx += dx * s; e.matrix.ty += dy * s; });
            }
        }
        if k(none, Key::Escape) {
            if !self.ed.selection.is_empty() || self.ed.frame_sel.is_some() {
                self.ed.selection.clear();
                self.ed.frame_sel = None;
            } else if self.ed.editing.is_some() {
                self.ed.exit_to_scene();
                self.fit_pending = true;
            }
        }
        for t in Tool::ALL {
            if k(none, t.key()) { self.tool = t; }
        }
    }

    // ------------------------------------------------------------ menus
    /// Object commands shared by the Edit/Modify menus and the stage context menu.
    pub(crate) fn object_menu(&mut self, ui: &mut egui::Ui) {
        let has_sel = !self.ed.selection.is_empty();
        if ui.add_enabled(has_sel, egui::Button::new("Cut").shortcut_text("Ctrl+X")).clicked() { self.cut(); ui.close_menu(); }
        if ui.add_enabled(has_sel, egui::Button::new("Copy").shortcut_text("Ctrl+C")).clicked() { self.copy(); ui.close_menu(); }
        if ui.add_enabled(self.ed.can_paste(), egui::Button::new("Paste in Center").shortcut_text("Ctrl+V")).clicked() { self.paste(false); ui.close_menu(); }
        if ui.add_enabled(self.ed.can_paste(), egui::Button::new("Paste in Place").shortcut_text("Ctrl+Shift+V")).clicked() { self.paste(true); ui.close_menu(); }
        if ui.add_enabled(has_sel, egui::Button::new("Duplicate").shortcut_text("Ctrl+D")).clicked() { self.ed.duplicate(); ui.close_menu(); }
        if ui.add_enabled(has_sel, egui::Button::new("Delete").shortcut_text("Del")).clicked() { self.delete(); ui.close_menu(); }
        ui.separator();
        if ui.add(egui::Button::new("Select All").shortcut_text("Ctrl+A")).clicked() { self.ed.select_all(); ui.close_menu(); }
        if ui.add_enabled(has_sel, egui::Button::new("Deselect All").shortcut_text("Esc")).clicked() { self.ed.selection.clear(); ui.close_menu(); }
        ui.separator();
        if ui.add_enabled(has_sel, egui::Button::new("Convert to Symbol…").shortcut_text("F8")).clicked() { self.open_symbol_dialog(true); ui.close_menu(); }
        let inst = self.ed.selected_element().and_then(|e| if let ElementKind::Instance(i) = e.kind { Some(i.symbol) } else { None });
        if ui.add_enabled(inst.is_some(), egui::Button::new("Break Apart").shortcut_text("Ctrl+B")).clicked() { self.ed.break_apart(); ui.close_menu(); }
        if let Some(id) = inst {
            if ui.button("Edit Symbol").clicked() { self.edit_symbol(id); ui.close_menu(); }
        }
        ui.menu_button("Arrange", |ui| {
            if ui.button("Bring to Front").clicked() { self.ed.arrange(true); ui.close_menu(); }
            if ui.button("Send to Back").clicked() { self.ed.arrange(false); ui.close_menu(); }
        });
        ui.menu_button("Transform", |ui| {
            if ui.button("Flip Horizontal").clicked() { self.flip(true); ui.close_menu(); }
            if ui.button("Flip Vertical").clicked() { self.flip(false); ui.close_menu(); }
            if ui.button("Rotate 90° CW").clicked() { self.rotate_selection(std::f32::consts::FRAC_PI_2); ui.close_menu(); }
            if ui.button("Rotate 90° CCW").clicked() { self.rotate_selection(-std::f32::consts::FRAC_PI_2); ui.close_menu(); }
            if ui.button("Remove Transform").clicked() {
                self.ed.modify_selected("Remove Transform", |e| { let (x, y) = (e.matrix.tx, e.matrix.ty); e.matrix = Matrix::translate(x, y); });
                ui.close_menu();
            }
        });
    }

    fn selection_center(&self) -> Option<Pt> {
        let (lo, hi) = self.ed.selection_bounds()?;
        Some(ac_core::pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
    }
    pub(crate) fn flip(&mut self, horizontal: bool) {
        let Some(c) = self.selection_center() else { return };
        let s = if horizontal { Matrix::scale(-1.0, 1.0) } else { Matrix::scale(1.0, -1.0) };
        let m = Matrix::translate(c.x, c.y).concat(&s).concat(&Matrix::translate(-c.x, -c.y));
        self.ed.modify_selected(if horizontal { "Flip Horizontal" } else { "Flip Vertical" }, |e| e.matrix = m.concat(&e.matrix));
    }
    pub(crate) fn rotate_selection(&mut self, rad: f32) {
        let Some(c) = self.selection_center() else { return };
        let m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(rad)).concat(&Matrix::translate(-c.x, -c.y));
        self.ed.modify_selected("Rotate", |e| e.matrix = m.concat(&e.matrix));
    }

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
                    self.object_menu(ui);
                    ui.separator();
                    ui.menu_button("Timeline", |ui| timeline::frame_menu(self, ui));
                    if ui.add_enabled(self.ed.editing.is_some(), egui::Button::new("Edit Document")).clicked() { self.ed.exit_to_scene(); self.fit_pending = true; ui.close_menu(); }
                });
                ui.menu_button("View", |ui| {
                    if ui.add(egui::Button::new("Zoom In").shortcut_text("Ctrl+=")).clicked() { self.zoom *= 1.25; ui.close_menu(); }
                    if ui.add(egui::Button::new("Zoom Out").shortcut_text("Ctrl+-")).clicked() { self.zoom /= 1.25; ui.close_menu(); }
                    if ui.add(egui::Button::new("100%").shortcut_text("Ctrl+1")).clicked() { self.zoom = 1.0; ui.close_menu(); }
                    if ui.add(egui::Button::new("Fit in Window").shortcut_text("Ctrl+2")).clicked() { self.fit_pending = true; ui.close_menu(); }
                });
                ui.menu_button("Insert", |ui| {
                    if ui.add(egui::Button::new("New Symbol…").shortcut_text("Ctrl+F8")).clicked() { self.open_symbol_dialog(false); ui.close_menu(); }
                    if ui.button("New Layer").clicked() { self.ed.add_layer(); ui.close_menu(); }
                    ui.separator();
                    timeline::frame_menu(self, ui);
                });
                ui.menu_button("Modify", |ui| {
                    if ui.button("Document…").clicked() { self.ed.selection.clear(); self.ed.frame_sel = None; self.status = "Document settings are in the Properties panel.".into(); ui.close_menu(); }
                    ui.separator();
                    self.object_menu(ui);
                });
                ui.menu_button("Control", |ui| {
                    if ui.add(egui::Button::new(if self.playing { "Stop" } else { "Play" }).shortcut_text("Enter")).clicked() { self.toggle_play(); ui.close_menu(); }
                    if ui.button("Rewind").clicked() { self.step(-100_000); ui.close_menu(); }
                    if ui.button("Go to End").clicked() { self.step(100_000); ui.close_menu(); }
                    if ui.add(egui::Button::new("Step Forward One Frame").shortcut_text(".")).clicked() { self.step(1); ui.close_menu(); }
                    if ui.add(egui::Button::new("Step Backward One Frame").shortcut_text(",")).clicked() { self.step(-1); ui.close_menu(); }
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

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").exact_height(22.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(&self.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{:.0}%", self.zoom * 100.0)).small());
                    ui.label(egui::RichText::new(self.tool.name()).small().color(Color32::from_gray(150)));
                    let n = self.ed.selection.len();
                    if n > 0 {
                        ui.label(egui::RichText::new(format!("{n} selected")).small().color(ACCENT));
                    }
                });
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut d = std::mem::replace(&mut self.dialog, Dialog::None);
        let mut close = false;
        let mut run: Option<(FileAction, String)> = None;
        let mut symbol_ok: Option<(bool, String, SymbolKind)> = None;
        let mut rename_ok: Option<(SymbolId, String)> = None;
        let mut delete_ok: Option<SymbolId> = None;
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let win = |t: &str| egui::Window::new(t.to_string()).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]);
        match &mut d {
            Dialog::None => {}
            Dialog::Path { title, path, action } => {
                win(title).show(ctx, |ui| {
                    ui.label("File path:");
                    let r = ui.add(egui::TextEdit::singleline(path).desired_width(360.0));
                    r.request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter {
                            run = Some((*action, path.clone()));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::Symbol { convert, name, kind } => {
                let convert = *convert;
                win(if convert { "Convert to Symbol" } else { "Create New Symbol" }).show(ctx, |ui| {
                    egui::Grid::new("sym").show(ui, |ui| {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(name)).request_focus();
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
                        if ui.button("OK").clicked() || enter { symbol_ok = Some((convert, name.clone(), *kind)); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::RenameSymbol { id, name } => {
                win("Rename Library Item").show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(name)).request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter { rename_ok = Some((*id, name.clone())); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::DeleteSymbol { id, uses } => {
                let name = self.ed.doc.symbol(*id).map(|s| s.name.clone()).unwrap_or_default();
                win("Delete Library Item").show(ctx, |ui| {
                    ui.label(format!("Delete “{name}”?"));
                    if *uses > 0 {
                        ui.label(egui::RichText::new(format!("{uses} instance(s) of it will also be removed from the document.")).color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() || enter { delete_ok = Some(*id); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::About => {
                win("About AnimateCraft").show(ctx, |ui| {
                    ui.heading("AnimateCraft 0.2 (demo)");
                    ui.label("Open-source 2D animation authoring in Rust, modelled on the Adobe Animate workflow.");
                    ui.label("Not affiliated with Adobe. Licensed MIT OR Apache-2.0.");
                    if ui.button("Close").clicked() || esc { close = true; }
                });
            }
        }
        if !close {
            self.dialog = d;
        }
        if let Some((a, p)) = run { self.do_file(a, &p); }
        if let Some((convert, n, k)) = symbol_ok {
            if convert {
                match self.ed.convert_to_symbol(&n, k) {
                    Some(id) => { self.lib_selected = Some(id); self.status = format!("Converted to symbol “{n}”"); }
                    None => self.status = "Couldn't convert: nothing selected on an editable layer.".into(),
                }
            } else {
                let id = self.ed.new_symbol(&n, k);
                self.lib_selected = Some(id);
                self.fit_pending = true;
                self.status = format!("Editing new symbol “{n}”. Press Esc to return to the scene.");
            }
        }
        if let Some((id, n)) = rename_ok { self.ed.rename_symbol(id, &n); }
        if let Some(id) = delete_ok {
            self.ed.delete_symbol(id);
            self.lib_selected = None;
            self.status = "Library item deleted".into();
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
        panels::tools_panel(self, ctx);
        panels::right_panel(self, ctx);
        stage::stage(self, ctx);
        panels::library_drag_ghost(self, ctx);
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
                if let Some(pm) = tiny_skia::Pixmap::from_vec(bytes, tiny_skia::IntSize::from_wh(w as u32, h as u32).unwrap()) {
                    let _ = pm.save_png(path.as_str());
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result<()> {
    let shot = std::env::var("AC_SCREENSHOT").is_ok();
    // Start maximised so the panels fit on any screen size / display scaling.
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 520.0])
        .with_maximized(!shot)
        .with_title("AnimateCraft");
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("AnimateCraft", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
