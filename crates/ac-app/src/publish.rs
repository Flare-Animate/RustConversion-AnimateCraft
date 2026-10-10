//! Publish Settings (Ctrl+Shift+F12) and background export jobs.

use crate::files::FileAction;
use crate::App;
use ac_render::ExportOpts;
use eframe::egui::{self, Color32, RichText};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

pub struct PublishSettings {
    pub open: bool,
    pub scale: f32,
    pub transparent: bool,
    pub whole: bool,
    pub from: u32,
    pub to: u32,
    pub gif_loops: u16,
    pub gif_quality: i32,
    pub seq_prefix: String,
    pub ffmpeg: String,
    pub job: Option<Job>,
    ffmpeg_checked: Option<Option<String>>,
}

impl Default for PublishSettings {
    fn default() -> Self {
        PublishSettings {
            open: false,
            scale: 1.0,
            transparent: false,
            whole: true,
            from: 1,
            to: 1,
            gif_loops: 0,
            gif_quality: 70,
            seq_prefix: "frame".into(),
            ffmpeg: "ffmpeg".into(),
            job: None,
            ffmpeg_checked: None,
        }
    }
}

pub struct Job {
    pub label: String,
    pub done: Arc<AtomicU32>,
    pub total: Arc<AtomicU32>,
    pub cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<String, String>>,
}

impl PublishSettings {
    pub fn opts(&self) -> ExportOpts {
        ExportOpts {
            scale: self.scale.clamp(0.05, 8.0),
            range: if self.whole { None } else { Some((self.from.max(1) - 1, self.to.max(self.from).max(1) - 1)) },
            transparent: self.transparent,
            loops: self.gif_loops,
            // quality 100 → speed 1 (best), 0 → 30 (fastest)
            gif_speed: (30.0 - self.gif_quality as f32 / 100.0 * 29.0).round() as i32,
        }
    }
}

/// Exports that take a while run on a worker thread with progress + cancel.
fn spawn(app: &mut App, label: String, work: impl FnOnce(&mut dyn FnMut(u32, u32) -> bool) -> Result<String, String> + Send + 'static) {
    if app.publish.job.is_some() {
        app.status = "An export is already running.".into();
        return;
    }
    let (tx, rx) = mpsc::channel();
    let done = Arc::new(AtomicU32::new(0));
    let total = Arc::new(AtomicU32::new(1));
    let cancel = Arc::new(AtomicBool::new(false));
    let (d, t, c) = (done.clone(), total.clone(), cancel.clone());
    let ctx = app.ctx.clone();
    std::thread::spawn(move || {
        let mut progress = |a: u32, b: u32| {
            d.store(a, Ordering::Relaxed);
            t.store(b, Ordering::Relaxed);
            ctx.request_repaint();
            !c.load(Ordering::Relaxed)
        };
        let r = work(&mut progress);
        let _ = tx.send(r);
        ctx.request_repaint();
    });
    app.status = format!("{label}…");
    app.publish.job = Some(Job { label, done, total, cancel, rx });
}

/// File actions handled here: multi-frame exports and imports.
pub(crate) fn run_file_action(app: &mut App, action: FileAction, path: &str) -> Result<String, String> {
    let doc = app.ed.doc.clone();
    let o = app.publish.opts();
    let p = std::path::PathBuf::from(path);
    match action {
        FileAction::ExportGif => {
            spawn(app, "Exporting GIF".into(), move |prog| ac_render::export_gif_opts(&doc, &o, &p, prog).map(|_| format!("Exported animated GIF to {}", p.display())));
            Ok("Exporting GIF…".into())
        }
        FileAction::ExportPngSequence => {
            let prefix = app.publish.seq_prefix.clone();
            spawn(app, "Exporting PNG sequence".into(), move |prog| ac_render::export_png_sequence(&doc, &o, &p, &prefix, prog).map(|n| format!("Exported {n} PNG frames to {}", p.display())));
            Ok("Exporting PNG sequence…".into())
        }
        FileAction::ExportVideo => {
            let ff = app.publish.ffmpeg.clone();
            if ac_render::ffmpeg_version(&ff).is_none() {
                return Err(format!("video export needs ffmpeg, and “{ff}” wasn't found. Install ffmpeg (https://ffmpeg.org; on Windows: winget install ffmpeg) or set its path in File › Publish Settings"));
            }
            spawn(app, "Exporting video".into(), move |prog| ac_render::export_video(&doc, &o, &p, &ff, prog).map(|_| format!("Exported video to {}", p.display())));
            Ok("Exporting video…".into())
        }
        FileAction::ImportImage => crate::import::import_image(app, path),
        FileAction::ImportSvg => crate::import::import_svg(app, path),
        FileAction::ImportSequence => crate::import::import_sequence(app, path),
        _ => Err("unsupported".into()),
    }
}

/// Poll the running job; show Publish Settings when open.
pub fn window(app: &mut App, ctx: &egui::Context) {
    if let Some(job) = &app.publish.job {
        if let Ok(r) = job.rx.try_recv() {
            app.status = match r {
                Ok(s) => s,
                Err(e) => format!("{} failed: {e}", job.label),
            };
            app.publish.job = None;
        }
    }
    if let Some(job) = &app.publish.job {
        let (d, t) = (job.done.load(Ordering::Relaxed), job.total.load(Ordering::Relaxed).max(1));
        let label = job.label.clone();
        let cancel = job.cancel.clone();
        egui::Window::new("Exporting").collapsible(false).resizable(false).anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -40.0]).show(ctx, |ui| {
            ui.label(format!("{label}: frame {d} of {t}"));
            ui.add(egui::ProgressBar::new(d as f32 / t as f32).desired_width(240.0));
            if ui.button("Cancel").clicked() {
                cancel.store(true, Ordering::Relaxed);
            }
        });
    }
    if !app.publish.open {
        return;
    }
    let mut open = true;
    let n = app.ed.doc.total_frames();
    let (w, h) = (app.ed.doc.width, app.ed.doc.height);
    egui::Window::new("Publish Settings").open(&mut open).resizable(false).show(ctx, |ui| {
        let ps = &mut app.publish;
        egui::Grid::new("pub").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Scale");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut ps.scale).range(0.05..=8.0).speed(0.01).suffix("×"));
                ui.label(RichText::new(format!("{} × {} px", (w * ps.scale).round(), (h * ps.scale).round())).color(Color32::from_gray(160)));
            });
            ui.end_row();
            ui.label("Frames");
            ui.horizontal(|ui| {
                ui.radio_value(&mut ps.whole, true, format!("All ({n})"));
                ui.radio_value(&mut ps.whole, false, "From");
                ui.add_enabled(!ps.whole, egui::DragValue::new(&mut ps.from).range(1..=n));
                ui.label("to");
                ui.add_enabled(!ps.whole, egui::DragValue::new(&mut ps.to).range(1..=n));
            });
            ui.end_row();
            ui.label("Background");
            ui.checkbox(&mut ps.transparent, "Transparent (PNG, GIF, WebM, MOV)");
            ui.end_row();
            ui.label("GIF");
            ui.horizontal(|ui| {
                ui.label("Loop");
                ui.add(egui::DragValue::new(&mut ps.gif_loops).range(0..=100).custom_formatter(|v, _| if v == 0.0 { "forever".into() } else { format!("{v}×") }));
                ui.label("Quality");
                ui.add(egui::Slider::new(&mut ps.gif_quality, 0..=100));
            });
            ui.end_row();
            ui.label("PNG sequence");
            ui.horizontal(|ui| {
                ui.label("File prefix");
                ui.add(egui::TextEdit::singleline(&mut ps.seq_prefix).desired_width(90.0));
                ui.label(RichText::new(format!("e.g. {}0001.png", ps.seq_prefix)).color(Color32::from_gray(150)));
            });
            ui.end_row();
            ui.label("Video");
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("ffmpeg");
                    if ui.add(egui::TextEdit::singleline(&mut ps.ffmpeg).desired_width(180.0)).changed() {
                        ps.ffmpeg_checked = None;
                    }
                });
                let found = ps.ffmpeg_checked.get_or_insert_with(|| ac_render::ffmpeg_version(&ps.ffmpeg)).clone();
                match found {
                    Some(v) => ui.label(RichText::new(format!("✔ {}", v.split(" Copyright").next().unwrap_or(&v))).small().color(Color32::from_rgb(0x70, 0xD0, 0x80))),
                    None => ui.label(RichText::new("ffmpeg not found: install it (Windows: winget install ffmpeg) or give its full path").small().color(Color32::from_rgb(0xFF, 0xB0, 0x40))),
                };
                ui.label(RichText::new(".mp4 = H.264 · .webm = VP9 (alpha) · .mov = ProRes 4444 (alpha)").small().color(Color32::from_gray(150)));
            });
            ui.end_row();
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label("Export:");
            for (a, l) in [(FileAction::ExportGif, "GIF…"), (FileAction::ExportPngSequence, "PNG sequence…"), (FileAction::ExportVideo, "Video…"), (FileAction::ExportPng, "PNG frame…"), (FileAction::ExportSvg, "SVG frame…")] {
                if ui.button(l).clicked() {
                    app.file(a);
                }
            }
        });
    });
    if !open {
        app.publish.open = false;
    }
}
