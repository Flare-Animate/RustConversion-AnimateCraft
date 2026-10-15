//! Swatches: a column of colour chips beside the Tools panel (Window ›
//! Swatches, Ctrl+F9). Click a chip for the fill colour, Shift+click for the
//! stroke; with something selected the colour is applied to it too.
//! Right-click a chip to replace, use or delete it.
//!
//! Palettes import from Photoshop `.aco` and Adobe Swatch Exchange `.ase`
//! (readers and writers from PhotoCraft's `photocraft-psd`, MIT OR Apache-2.0,
//! used as a git dependency), GIMP `.gpl` and colour tables `.act` (our own
//! readers), and export as `.ase`, `.aco` or `.gpl`. The swatches are kept in
//! the settings, so they are the same in every document.

use crate::App;
use ac_core::{ElementKind, Stroke};
use eframe::egui::{self, vec2, Color32, RichText, Sense};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Swatch {
    #[serde(default)]
    pub name: String,
    pub rgba: [u8; 4],
}

impl Swatch {
    fn new(name: impl Into<String>, c: Color32) -> Swatch {
        Swatch { name: name.into(), rgba: c.to_srgba_unmultiplied() }
    }
    pub fn color(&self) -> Color32 {
        let [r, g, b, a] = self.rgba;
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }
    fn label(&self) -> String {
        let [r, g, b, a] = self.rgba;
        let hex = if a == 255 { format!("#{r:02X}{g:02X}{b:02X}") } else { format!("#{r:02X}{g:02X}{b:02X} · {:.0}%", a as f32 / 2.55) };
        if self.name.is_empty() { hex } else { format!("{}  {hex}", self.name) }
    }
}

/// The standard set: a grey ramp, then twelve hues at full, light and dark.
pub(crate) fn defaults() -> Vec<Swatch> {
    let mut v = Vec::new();
    for i in 0..12u32 {
        let g = (i * 255 / 11) as u8;
        v.push(Swatch::new("", Color32::from_gray(g)));
    }
    for (s, b) in [(1.0, 1.0), (0.45, 1.0), (1.0, 0.55)] {
        for h in 0..12 {
            let [r, g, bl] = hsb(h as f32 * 30.0, s, b);
            v.push(Swatch::new("", Color32::from_rgb(r, g, bl)));
        }
    }
    v
}

fn hsb(h: f32, s: f32, b: f32) -> [u8; 3] {
    let c = b * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, bl) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = b - c;
    [to8(r + m), to8(g + m), to8(bl + m)]
}

fn to8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// CIE L*a*b* (D50, as Photoshop and ASE store it) to sRGB.
fn lab(l: f32, a: f32, b: f32) -> [u8; 3] {
    let fy = (l + 16.0) / 116.0;
    let (fx, fz) = (fy + a / 500.0, fy - b / 200.0);
    let f = |t: f32| if t > 6.0 / 29.0 { t * t * t } else { 3.0 * (6.0f32 / 29.0).powi(2) * (t - 4.0 / 29.0) };
    let (x, y, z) = (0.964_22 * f(fx), f(fy), 0.825_21 * f(fz));
    // XYZ (D50) → linear sRGB with Bradford adaptation to D65
    let r = 3.133_856 * x - 1.616_867 * y - 0.490_615 * z;
    let g = -0.978_768 * x + 1.916_142 * y + 0.033_454 * z;
    let bl = 0.071_945 * x - 0.228_991 * y + 1.405_243 * z;
    let enc = |c: f32| if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    [to8(enc(r)), to8(enc(g)), to8(enc(bl))]
}

fn cmyk(c: f32, m: f32, y: f32, k: f32) -> [u8; 3] {
    [to8((1.0 - c) * (1.0 - k)), to8((1.0 - m) * (1.0 - k)), to8((1.0 - y) * (1.0 - k))]
}

/// Read a palette file by its extension.
pub(crate) fn read(path: &std::path::Path) -> Result<Vec<Swatch>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let v = parse(&ext, &data)?;
    if v.is_empty() {
        return Err("no colours in that file".into());
    }
    Ok(v)
}

pub(crate) fn parse(ext: &str, data: &[u8]) -> Result<Vec<Swatch>, String> {
    use photocraft_psd::{aco, ase};
    let rgb = |n: &str, [r, g, b]: [u8; 3]| Swatch::new(n, Color32::from_rgb(r, g, b));
    match ext {
        "aco" => {
            let f = aco::parse(data).map_err(|e| format!(".aco: {e}"))?;
            Ok(f.swatches
                .iter()
                .filter_map(|s| {
                    let n = |v: u16| v as f32 / 65535.0;
                    let c = match s.color {
                        aco::AcoColor::Rgb([r, g, b]) => [(r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8],
                        aco::AcoColor::Hsb([h, sa, b]) => hsb(n(h) * 360.0, n(sa), n(b)),
                        aco::AcoColor::Cmyk([c, m, y, k]) => cmyk(n(c), n(m), n(y), n(k)),
                        aco::AcoColor::Lab { l, a, b } => lab(l as f32 / 100.0, a as f32 / 100.0, b as f32 / 100.0),
                        aco::AcoColor::Gray(g) => [to8(g as f32 / 10000.0); 3],
                        aco::AcoColor::Other { .. } => return None,
                    };
                    Some(rgb(&s.name, c))
                })
                .collect())
        }
        "ase" => {
            let f = ase::parse(data).map_err(|e| format!(".ase: {e}"))?;
            let one = |s: &ase::AseSwatch| -> Option<Swatch> {
                let c = match &s.color {
                    ase::AseColor::Rgb([r, g, b]) => [to8(*r), to8(*g), to8(*b)],
                    ase::AseColor::Cmyk([c, m, y, k]) => cmyk(*c, *m, *y, *k),
                    ase::AseColor::Lab([l, a, b]) => lab(l * 100.0, *a, *b),
                    ase::AseColor::Gray(g) => [to8(*g); 3],
                    ase::AseColor::Other { .. } => return None,
                };
                Some(rgb(&s.name, c))
            };
            Ok(f.entries
                .iter()
                .flat_map(|e| match e {
                    ase::AseEntry::Swatch(s) => vec![s],
                    ase::AseEntry::Group { swatches, .. } => swatches.iter().collect(),
                })
                .filter_map(one)
                .collect())
        }
        "gpl" => {
            let text = String::from_utf8_lossy(data);
            let mut lines = text.lines();
            if !lines.next().is_some_and(|l| l.trim_start_matches('\u{feff}').trim() == "GIMP Palette") {
                return Err(".gpl: not a GIMP palette (first line must be \"GIMP Palette\")".into());
            }
            Ok(lines
                .filter_map(|l| {
                    let l = l.trim();
                    if l.is_empty() || l.starts_with('#') || l.contains(':') && !l.starts_with(|c: char| c.is_ascii_digit()) {
                        return None; // comments and "Name:" / "Columns:" headers
                    }
                    let mut it = l.split_whitespace();
                    let mut ch = [0u8; 3];
                    for c in &mut ch {
                        *c = it.next()?.parse().ok()?;
                    }
                    Some(rgb(&it.collect::<Vec<_>>().join(" "), ch))
                })
                .collect())
        }
        "act" => {
            // 256 RGB triples, then (optional) a big-endian count of used colours
            if data.len() < 768 {
                return Err(".act: a colour table is 768 bytes".into());
            }
            let n = if data.len() >= 770 { u16::from_be_bytes([data[768], data[769]]).clamp(1, 256) as usize } else { 256 };
            Ok(data[..n * 3].chunks(3).map(|c| rgb("", [c[0], c[1], c[2]])).collect())
        }
        _ => Err(format!("unknown palette type .{ext} (use .aco, .ase, .gpl or .act)")),
    }
}

/// Write a palette by extension (.ase, .aco or .gpl).
pub(crate) fn write(path: &std::path::Path, v: &[Swatch]) -> Result<(), String> {
    use photocraft_psd::{aco, ase};
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let bytes = match ext.as_str() {
        "ase" => {
            let entries = v
                .iter()
                .map(|s| {
                    let [r, g, b, _] = s.rgba;
                    ase::AseEntry::Swatch(ase::AseSwatch { name: s.label(), color: ase::AseColor::Rgb([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]), kind: ase::AseKind::Process })
                })
                .collect();
            ase::write(&ase::AseFile { entries }).map_err(|e| e.to_string())?
        }
        "aco" => {
            let sw: Vec<aco::AcoSwatch> = v
                .iter()
                .map(|s| {
                    let [r, g, b, _] = s.rgba;
                    aco::AcoSwatch { name: s.label(), color: aco::AcoColor::Rgb([r as u16 * 257, g as u16 * 257, b as u16 * 257]) }
                })
                .collect();
            aco::write(&sw).map_err(|e| e.to_string())?
        }
        "gpl" => {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Flare");
            let mut t = format!("GIMP Palette\nName: {name}\nColumns: 12\n#\n");
            for s in v {
                let [r, g, b, _] = s.rgba;
                t.push_str(&format!("{r:3} {g:3} {b:3}\t{}\n", if s.name.is_empty() { format!("#{r:02X}{g:02X}{b:02X}") } else { s.name.clone() }));
            }
            t.into_bytes()
        }
        _ => return Err(format!("can't write .{ext} (use .ase, .aco or .gpl)")),
    };
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[derive(Default)]
pub(crate) struct State {
    pub open: bool,
    /// Import replaces the swatches instead of adding to them.
    pub replace: bool,
}

fn save(app: &mut App) {
    app.settings.save();
}

/// Use a colour as the fill (or stroke) and apply it to the selection.
pub(crate) fn use_color(app: &mut App, c: Color32, stroke: bool) {
    let col = crate::from32(c);
    if stroke {
        app.stroke = c;
        app.stroke_on = true;
    } else {
        app.fill = c;
        app.fill_on = true;
        app.topts.fill_gradient = None;
    }
    if app.ed.selection.is_empty() {
        return;
    }
    let has_canvas = app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Canvas(_))));
    if has_canvas {
        if stroke {
            let w = app.stroke_w;
            app.ed.merge_set_paint(None, None, Some(Some(Stroke { color: col, width: w })));
        } else {
            app.ed.merge_set_paint(Some(Some(col)), Some(None), None);
        }
    }
    let w = app.stroke_w;
    let others = app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Shape(_) | ElementKind::Text(_))));
    if others {
        app.ed.modify_selected(if stroke { "Stroke Color" } else { "Fill Color" }, move |e| match &mut e.kind {
            ElementKind::Shape(s) if stroke => s.stroke = Some(Stroke { color: col, width: s.stroke.map_or(w, |st| st.width) }),
            ElementKind::Shape(s) => {
                s.fill = Some(col);
                s.gradient = None;
            }
            ElementKind::Text(t) if !stroke => t.color = col,
            _ => {}
        });
    }
}

/// The swatch column beside the tools.
pub(crate) fn panel(app: &mut App, ctx: &egui::Context) {
    if !app.swatch_ui.open {
        return;
    }
    egui::SidePanel::new(app.layout.tools_side(), "swatches").exact_width(138.0).resizable(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Swatches").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").on_hover_text("Close (Ctrl+F9)").clicked() {
                    app.swatch_ui.open = false;
                }
                ui.menu_button("…", |ui| menu(app, ui)).response.on_hover_text("Swatch options");
            });
        });
        ui.label(RichText::new("Click: fill · Shift+click: stroke").small().color(Color32::from_gray(140)));
        ui.add_space(2.0);
        let list = app.settings.swatches.clone().unwrap_or_else(defaults);
        let mut action: Option<(usize, u8)> = None; // 0 fill, 1 stroke, 2 replace, 3 delete
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let cell = 18.0;
            let cols = ((ui.available_width() + 2.0) / (cell + 2.0)).floor().max(1.0) as usize;
            for row in list.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                    for (j, s) in row.1.iter().enumerate() {
                        let i = row.0 * cols + j;
                        let (r, resp) = ui.allocate_exact_size(vec2(cell, cell), Sense::click());
                        let c = s.color();
                        if c.a() < 255 {
                            ui.painter().rect_filled(r, 0.0, Color32::from_gray(200));
                            ui.painter().rect_filled(egui::Rect::from_min_size(r.min, r.size() / 2.0), 0.0, Color32::from_gray(255));
                            ui.painter().rect_filled(egui::Rect::from_min_size(r.center(), r.size() / 2.0), 0.0, Color32::from_gray(255));
                        }
                        ui.painter().rect_filled(r, 0.0, c);
                        let current = c == app.fill || c == app.stroke;
                        let edge = if resp.hovered() { Color32::WHITE } else if current { Color32::from_rgb(0x3D, 0x8E, 0xF0) } else { Color32::from_gray(70) };
                        ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0_f32, edge));
                        let resp = resp.on_hover_text(s.label());
                        if resp.clicked() {
                            action = Some((i, if ui.input(|inp| inp.modifiers.shift) { 1 } else { 0 }));
                        }
                        resp.context_menu(|ui| {
                            ui.label(RichText::new(s.label()).small());
                            if ui.button("Use as Fill").clicked() { action = Some((i, 0)); ui.close_menu(); }
                            if ui.button("Use as Stroke").clicked() { action = Some((i, 1)); ui.close_menu(); }
                            if ui.button("Replace with Current Fill").clicked() { action = Some((i, 2)); ui.close_menu(); }
                            if ui.button("Delete Swatch").clicked() { action = Some((i, 3)); ui.close_menu(); }
                        });
                    }
                });
            }
            ui.add_space(4.0);
            if ui.button("+ Add fill colour").on_hover_text("Add the current fill colour").clicked() {
                add(app, app.fill);
            }
        });
        if let Some((i, op)) = action {
            let mut list = list;
            match op {
                0 | 1 => use_color(app, list[i].color(), op == 1),
                2 => {
                    list[i] = Swatch::new("", app.fill);
                    app.settings.swatches = Some(list);
                    save(app);
                }
                _ => {
                    list.remove(i);
                    app.settings.swatches = Some(list);
                    save(app);
                }
            }
        }
    });
}

fn add(app: &mut App, c: Color32) {
    let mut list = app.settings.swatches.clone().unwrap_or_else(defaults);
    if list.iter().all(|s| s.rgba != c.to_srgba_unmultiplied()) {
        list.push(Swatch::new("", c));
        app.settings.swatches = Some(list);
        save(app);
    }
}

fn menu(app: &mut App, ui: &mut egui::Ui) {
    if ui.button("Add Fill Colour").clicked() { add(app, app.fill); ui.close_menu(); }
    if ui.button("Add Stroke Colour").clicked() { add(app, app.stroke); ui.close_menu(); }
    ui.separator();
    if ui.button("Add Colours from File…").on_hover_text(".aco, .ase, .gpl, .act").clicked() {
        app.swatch_ui.replace = false;
        app.file(crate::files::FileAction::ImportSwatches);
        ui.close_menu();
    }
    if ui.button("Replace Colours from File…").clicked() {
        app.swatch_ui.replace = true;
        app.file(crate::files::FileAction::ImportSwatches);
        ui.close_menu();
    }
    if ui.button("Save Colours…").on_hover_text(".ase, .aco or .gpl").clicked() {
        app.file(crate::files::FileAction::ExportSwatches);
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Sort by Colour").clicked() {
        let mut list = app.settings.swatches.clone().unwrap_or_else(defaults);
        list.sort_by(|a, b| sort_key(a).partial_cmp(&sort_key(b)).unwrap_or(std::cmp::Ordering::Equal));
        app.settings.swatches = Some(list);
        save(app);
        ui.close_menu();
    }
    if ui.button("Reset to Standard").clicked() {
        app.settings.swatches = None;
        save(app);
        ui.close_menu();
    }
    if ui.button("Clear All").clicked() {
        app.settings.swatches = Some(Vec::new());
        save(app);
        ui.close_menu();
    }
}

/// Greys first (dark to light), then by hue, then lightness.
fn sort_key(s: &Swatch) -> (u8, f32, f32) {
    let [r, g, b, _] = s.rgba.map(|v| v as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    if d < 0.04 {
        return (0, mx, 0.0);
    }
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (1, h, mx)
}

/// File › Import swatches result (FileAction::ImportSwatches).
pub(crate) fn import(app: &mut App, path: &str) -> Result<String, String> {
    let new = read(std::path::Path::new(path))?;
    let n = new.len();
    let mut list = if app.swatch_ui.replace { Vec::new() } else { app.settings.swatches.clone().unwrap_or_else(defaults) };
    list.extend(new);
    app.settings.swatches = Some(list);
    app.swatch_ui.open = true;
    save(app);
    Ok(format!("{} {n} swatch{} from {}", if app.swatch_ui.replace { "Loaded" } else { "Added" }, if n == 1 { "" } else { "es" }, path))
}

pub(crate) fn export(app: &mut App, path: &str) -> Result<String, String> {
    let list = app.settings.swatches.clone().unwrap_or_else(defaults);
    write(std::path::Path::new(path), &list)?;
    Ok(format!("Saved {} swatches to {path}", list.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_writable_format() {
        let v = vec![Swatch::new("Sky", Color32::from_rgb(0x40, 0x90, 0xE0)), Swatch::new("", Color32::from_rgb(255, 0, 0)), Swatch::new("", Color32::from_gray(17))];
        let dir = std::env::temp_dir().join(format!("flare-swatches-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["ase", "aco", "gpl"] {
            let p = dir.join(format!("t.{ext}"));
            write(&p, &v).unwrap();
            let back = read(&p).unwrap();
            assert_eq!(back.iter().map(|s| s.rgba).collect::<Vec<_>>(), v.iter().map(|s| s.rgba).collect::<Vec<_>>(), "{ext}");
            assert!(back[0].name.contains("Sky"), "{ext}: {:?}", back[0].name);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reads_gimp_and_act() {
        let gpl = "GIMP Palette\nName: Test\nColumns: 4\n# comment\n255   0   0\tRed\n  0 128 255 Deep Sky\n";
        let v = parse("gpl", gpl.as_bytes()).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].rgba, [0, 128, 255, 255]);
        assert_eq!(v[1].name, "Deep Sky");
        assert!(parse("gpl", b"not a palette").is_err());
        let mut act = vec![0u8; 772];
        act[3..6].copy_from_slice(&[10, 20, 30]);
        act[768..770].copy_from_slice(&2u16.to_be_bytes());
        let v = parse("act", &act).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].rgba, [10, 20, 30, 255]);
        assert!(parse("act", &[0; 10]).is_err());
    }

    #[test]
    fn converts_lab_cmyk_and_hsb() {
        assert_eq!(lab(100.0, 0.0, 0.0), [255, 255, 255]);
        assert_eq!(lab(0.0, 0.0, 0.0), [0, 0, 0]);
        let red = lab(54.29, 80.81, 69.89); // sRGB red in Lab (D50)
        assert!(red[0] > 245 && red[1] < 20 && red[2] < 20, "{red:?}");
        assert_eq!(cmyk(0.0, 1.0, 1.0, 0.0), [255, 0, 0]);
        assert_eq!(hsb(120.0, 1.0, 1.0), [0, 255, 0]);
        // an .aco with CMYK and Lab entries
        use photocraft_psd::aco::{write as w, AcoColor, AcoSwatch};
        let bytes = w(&[AcoSwatch { name: "C".into(), color: AcoColor::Cmyk([65535, 0, 0, 0]) }, AcoSwatch { name: "L".into(), color: AcoColor::Lab { l: 10000, a: 0, b: 0 } }]).unwrap();
        let v = parse("aco", &bytes).unwrap();
        assert_eq!(v[0].rgba, [0, 255, 255, 255]);
        assert_eq!(v[1].rgba, [255, 255, 255, 255]);
    }

    #[test]
    fn defaults_and_sorting() {
        let d = defaults();
        assert_eq!(d.len(), 48);
        assert_eq!(d[0].rgba, [0, 0, 0, 255]);
        assert_eq!(d[11].rgba, [255, 255, 255, 255]);
        let mut s = d.clone();
        s.reverse();
        s.sort_by(|a, b| sort_key(a).partial_cmp(&sort_key(b)).unwrap());
        assert_eq!(s[0].rgba, [0, 0, 0, 255]);
    }
}
