//! Files: native open/save dialogs, recent files, settings, autosave and
//! crash recovery, and the "unsaved changes" guard.

use crate::{App, Dialog};
use ac_core::{demo::demo_document, Document, Editor};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FileAction {
    Open,
    Save,
    ExportGif,
    ExportPng,
    ExportSvg,
    ExportPngSequence,
    ExportVideo,
    ImportImage,
    ImportSvg,
    ImportSequence,
}

impl FileAction {
    pub fn title(self) -> &'static str {
        match self {
            FileAction::Open => "Open document",
            FileAction::Save => "Save document as",
            FileAction::ExportGif => "Export animated GIF",
            FileAction::ExportPng => "Export current frame as PNG",
            FileAction::ExportSvg => "Export current frame as SVG",
            FileAction::ExportPngSequence => "Export PNG sequence (choose a folder)",
            FileAction::ExportVideo => "Export video (MP4 / WebM / MOV)",
            FileAction::ImportImage => "Import image (PNG / JPG / GIF / BMP)",
            FileAction::ImportSvg => "Import SVG",
            FileAction::ImportSequence => "Import image sequence (pick the first numbered image)",
        }
    }
    fn default_name(self, doc_name: &str) -> String {
        match self {
            FileAction::Open | FileAction::ImportImage | FileAction::ImportSvg | FileAction::ImportSequence => String::new(),
            FileAction::Save => format!("{doc_name}.acx.json"),
            FileAction::ExportGif => format!("{doc_name}.gif"),
            FileAction::ExportPng => format!("{doc_name}.png"),
            FileAction::ExportSvg => format!("{doc_name}.svg"),
            FileAction::ExportPngSequence => format!("{doc_name}-frames"),
            FileAction::ExportVideo => format!("{doc_name}.mp4"),
        }
    }
    fn filters(self) -> (&'static str, &'static [&'static str]) {
        match self {
            FileAction::Open | FileAction::Save => ("AnimationCraft document", &["json"]),
            FileAction::ExportGif => ("Animated GIF", &["gif"]),
            FileAction::ExportPng => ("PNG image", &["png"]),
            FileAction::ExportSvg | FileAction::ImportSvg => ("SVG", &["svg"]),
            FileAction::ExportPngSequence => ("Folder", &[]),
            FileAction::ExportVideo => ("Video", &["mp4", "webm", "mov"]),
            FileAction::ImportImage | FileAction::ImportSequence => ("Images", &["png", "jpg", "jpeg", "gif", "bmp"]),
        }
    }
    fn picks_existing(self) -> bool {
        matches!(self, FileAction::Open | FileAction::ImportImage | FileAction::ImportSvg | FileAction::ImportSequence)
    }
}

/// Native dialogs can be turned off (headless UI tests): AC_NO_NATIVE_DIALOGS=1.
fn native_dialogs() -> bool {
    std::env::var_os("AC_NO_NATIVE_DIALOGS").is_none()
}

/// Persistent app settings (config dir / AnimationCraft / settings.json).
#[derive(Serialize, Deserialize, Default, Clone)]
pub(crate) struct Settings {
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub last_dir: Option<String>,
    #[serde(default)]
    pub library_left: bool,
}

fn app_dir(base: Option<PathBuf>) -> Option<PathBuf> {
    let d = base?.join("AnimationCraft");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}
fn settings_path() -> Option<PathBuf> {
    std::env::var_os("AC_CONFIG_DIR").map(PathBuf::from).map(|p| { let _ = std::fs::create_dir_all(&p); p.join("settings.json") }).or_else(|| app_dir(dirs::config_dir()).map(|d| d.join("settings.json")))
}
pub(crate) fn recovery_path() -> Option<PathBuf> {
    std::env::var_os("AC_CONFIG_DIR").map(|p| PathBuf::from(p).join("recovery.acx.json")).or_else(|| app_dir(dirs::data_local_dir()).map(|d| d.join("recovery.acx.json")))
}
fn recovery_meta_path() -> Option<PathBuf> {
    recovery_path().map(|p| p.with_file_name("recovery.meta.json"))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RecoveryMeta {
    pub original: Option<String>,
    pub saved_at: u64,
}

impl Settings {
    pub fn load() -> Settings {
        settings_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        if let (Some(p), Ok(s)) = (settings_path(), serde_json::to_string_pretty(self)) {
            let _ = std::fs::write(p, s);
        }
    }
    pub fn add_recent(&mut self, path: &str) {
        let full = std::fs::canonicalize(path).map(|p| p.display().to_string()).unwrap_or_else(|_| path.to_string());
        self.recent.retain(|r| r != &full);
        self.recent.insert(0, full.clone());
        self.recent.truncate(10);
        self.last_dir = Path::new(&full).parent().map(|p| p.display().to_string());
        self.save();
    }
}

/// Something the user asked for that would discard unsaved changes.
#[derive(Clone, Debug)]
pub(crate) enum Pending {
    New,
    OpenDemo,
    Open,
    OpenPath(String),
    Quit,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl App {
    pub(crate) fn doc_name(&self) -> String {
        self.file_path
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .map(|n| n.to_string_lossy().trim_end_matches(".json").trim_end_matches(".acx").to_string())
            .unwrap_or_else(|| "Untitled".into())
    }
    pub(crate) fn dirty(&self) -> bool {
        self.ed.revision != self.saved_rev
    }

    /// Start a file action: native dialog when available, typed path otherwise.
    pub(crate) fn file(&mut self, action: FileAction) {
        let name = action.default_name(&self.doc_name());
        if !native_dialogs() {
            self.dialog = Dialog::Path { title: action.title().into(), path: if name.is_empty() { "untitled.acx.json".into() } else { name }, action };
            return;
        }
        let mut d = rfd::FileDialog::new().set_title(action.title());
        if let Some(dir) = self.settings.last_dir.clone().or_else(|| self.file_path.as_ref().and_then(|p| Path::new(p).parent().map(|d| d.display().to_string()))) {
            d = d.set_directory(dir);
        }
        let (label, exts) = action.filters();
        if !exts.is_empty() {
            d = d.add_filter(label, exts);
        }
        let picked = if action == FileAction::ExportPngSequence {
            d.pick_folder()
        } else if action.picks_existing() {
            d.pick_file()
        } else {
            d.set_file_name(&name).save_file()
        };
        if let Some(p) = picked {
            self.do_file(action, &p.display().to_string());
        }
    }

    pub(crate) fn save(&mut self) {
        match self.file_path.clone() {
            Some(p) => self.do_file(FileAction::Save, &p),
            None => self.file(FileAction::Save),
        }
    }

    /// Run `p` now, or ask first when there are unsaved changes.
    pub(crate) fn guarded(&mut self, p: Pending) {
        if self.dirty() {
            self.dialog = Dialog::Unsaved(p);
        } else {
            self.run_pending(p);
        }
    }

    pub(crate) fn run_pending(&mut self, p: Pending) {
        match p {
            Pending::New => self.load_doc(Document::default(), None),
            Pending::OpenDemo => self.load_doc(demo_document(), None),
            Pending::Open => self.file(FileAction::Open),
            Pending::OpenPath(path) => self.do_file(FileAction::Open, &path),
            Pending::Quit => {
                self.quit_confirmed = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn load_doc(&mut self, doc: Document, path: Option<String>) {
        self.ed = Editor::new(doc);
        self.saved_rev = self.ed.revision;
        self.autosave_rev = self.ed.revision;
        self.file_path = path;
        self.fit_pending = true;
        self.lib_selected = None;
        self.ft_pivot = None;
    }

    pub(crate) fn do_file(&mut self, action: FileAction, path: &str) {
        let p = Path::new(path);
        let res: Result<String, String> = match action {
            FileAction::Open => std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|s| {
                Document::from_json(&s).map_err(|e| format!("not an AnimationCraft document ({e})")).map(|d| {
                    self.load_doc(d, Some(path.into()));
                    self.settings.add_recent(path);
                    format!("Opened {path}")
                })
            }),
            FileAction::Save => std::fs::write(p, self.ed.doc.with_floats_committed().to_json()).map_err(|e| e.to_string()).map(|_| {
                self.file_path = Some(path.into());
                self.saved_rev = self.ed.revision;
                self.settings.add_recent(path);
                clear_recovery();
                format!("Saved {}", std::fs::canonicalize(p).map(|c| c.display().to_string()).unwrap_or(path.into()))
            }),
            FileAction::ExportPng => ac_render::export_png(&self.ed.doc, self.ed.frame, 2.0, p).map(|_| format!("Exported frame {} to {path}", self.ed.frame + 1)),
            FileAction::ExportSvg => std::fs::write(p, ac_render::export_svg(&self.ed.doc, self.ed.frame)).map_err(|e| e.to_string()).map(|_| format!("Exported SVG to {path}")),
            other => crate::publish::run_file_action(self, other, path),
        };
        let saved = action == FileAction::Save && res.is_ok();
        self.status = res.unwrap_or_else(|e| format!("Error: {e}"));
        if saved {
            if let Some(p) = self.after_save.take() {
                self.run_pending(p);
            }
        } else if action == FileAction::Save {
            self.after_save = None;
        }
    }

    /// Autosave once changes have sat unsaved for `AC_AUTOSAVE_SECS` (default 60 s).
    pub(crate) fn autosave_tick(&mut self, ctx: &egui::Context) {
        if self.ed.revision == self.autosave_rev {
            self.dirty_since = None;
            return;
        }
        let interval: f64 = std::env::var("AC_AUTOSAVE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(60.0);
        let t = ctx.input(|i| i.time);
        let since = *self.dirty_since.get_or_insert(t);
        if t - since < interval {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((interval - (t - since)).max(0.2)));
            return;
        }
        if let (Some(p), Some(m)) = (recovery_path(), recovery_meta_path()) {
            let meta = RecoveryMeta { original: self.file_path.clone(), saved_at: now_secs() };
            if std::fs::write(&p, self.ed.doc.with_floats_committed().to_json()).is_ok() {
                let _ = std::fs::write(m, serde_json::to_string(&meta).unwrap_or_default());
                self.autosave_rev = self.ed.revision;
                self.dirty_since = None;
            }
        }
    }

    /// On launch: offer to restore an autosave left behind by a crash.
    pub(crate) fn check_recovery(&mut self) {
        let Some(p) = recovery_path() else { return };
        if !p.exists() {
            return;
        }
        let meta: Option<RecoveryMeta> = recovery_meta_path().and_then(|m| std::fs::read_to_string(m).ok()).and_then(|s| serde_json::from_str(&s).ok());
        self.dialog = Dialog::Recover { original: meta.as_ref().and_then(|m| m.original.clone()), age_secs: meta.map(|m| now_secs().saturating_sub(m.saved_at)).unwrap_or(0) };
    }

    pub(crate) fn restore_recovery(&mut self) {
        let Some(p) = recovery_path() else { return };
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| Document::from_json(&s).map_err(|e| e.to_string())) {
            Ok(d) => {
                let original: Option<String> = recovery_meta_path().and_then(|m| std::fs::read_to_string(m).ok()).and_then(|s| serde_json::from_str::<RecoveryMeta>(&s).ok()).and_then(|m| m.original);
                self.load_doc(d, original);
                self.saved_rev = u64::MAX; // restored work is unsaved
                self.status = "Restored your work from the last autosave. Save it to keep it.".into();
            }
            Err(e) => self.status = format!("Couldn't restore the autosave: {e}"),
        }
    }
}

pub(crate) fn clear_recovery() {
    for p in [recovery_path(), recovery_meta_path()].into_iter().flatten() {
        let _ = std::fs::remove_file(p);
    }
}

pub(crate) fn fmt_age(secs: u64) -> String {
    match secs {
        0..=89 => "just now".into(),
        90..=5399 => format!("{} minutes ago", secs / 60),
        5400..=172_799 => format!("{} hours ago", secs / 3600),
        _ => format!("{} days ago", secs / 86400),
    }
}
