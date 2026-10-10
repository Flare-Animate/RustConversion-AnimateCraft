//! System fonts for text (font picker + outline loading). Installs a loader
//! into `ac_core::text` so documents can name any installed family.

use std::sync::OnceLock;

fn db() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut d = fontdb::Database::new();
        d.load_system_fonts();
        if let Some(dir) = std::env::var_os("AC_FONT_DIR") {
            d.load_fonts_dir(dir);
        }
        d
    })
}

fn load(family: &str) -> Option<Vec<u8>> {
    let d = db();
    let q = fontdb::Query { families: &[fontdb::Family::Name(family)], ..Default::default() };
    let id = d.query(&q)?;
    let mut out = None;
    // ab_glyph reads the first face of a collection; good enough for .ttc defaults
    d.with_face_data(id, |data, _index| out = Some(data.to_vec()));
    out
}

/// Make named font families available to text outlines (call once at start-up).
pub fn install_system_fonts() {
    ac_core::text::set_font_loader(load);
}

/// Installed font family names, sorted.
pub fn families() -> &'static [String] {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut v: Vec<String> = db().faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
        v.sort_by_key(|s| s.to_lowercase());
        v.dedup();
        v
    })
}
