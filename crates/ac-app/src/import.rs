//! File › Import: bitmaps (PNG/JPG/GIF/BMP, animated GIF, numbered
//! sequences) into the library and onto the stage, and SVG as editable shapes.

use crate::App;
use ac_core::{pt, BitmapAsset, Color, Element, Gradient, GradientKind, Keyframe, Matrix, Path, Pt, Seg, Shape, Stroke};
use std::path::{Path as FsPath, PathBuf};

fn file_name(path: &str) -> String {
    FsPath::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Bitmap".into())
}

/// Decode any supported image into a library bitmap (PNGs are kept as they are).
fn load_bitmap(path: &str) -> Result<BitmapAsset, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let name = file_name(path);
    if bytes.starts_with(b"\x89PNG") {
        if let Ok(a) = BitmapAsset::from_png(&name, bytes.clone()) {
            return Ok(a);
        }
    }
    let img = image::load_from_memory(&bytes).map_err(|e| format!("{name}: {e}"))?.to_rgba8();
    BitmapAsset::from_rgba(0, &name, img.width(), img.height(), img.as_raw())
}

/// Where to put something of size w×h: centred on the stage (top-left in a symbol).
fn place_at(app: &App, w: f32, h: f32) -> Pt {
    if app.ed.editing.is_some() {
        return pt(0.0, 0.0);
    }
    let d = &app.ed.doc;
    pt(((d.width - w) / 2.0).max(0.0).round(), ((d.height - h) / 2.0).max(0.0).round())
}

pub(crate) fn import_image(app: &mut App, path: &str) -> Result<String, String> {
    if path.to_lowercase().ends_with(".gif") {
        if let Some(r) = import_animated_gif(app, path) {
            return r;
        }
    }
    if !app.ed.layer_editable(app.ed.layer) {
        return Err("the current layer is locked or hidden".into());
    }
    let asset = load_bitmap(path)?;
    let (w, h, name) = (asset.width, asset.height, asset.name.clone());
    let mark = app.ed.undo_depth();
    let id = app.ed.add_bitmap(asset);
    let at = place_at(app, w as f32, h as f32);
    let s = app.ed.place_bitmap(id, at);
    app.ed.squash_undo(mark, "Import");
    if let Some(s) = s {
        app.ed.selection = vec![s];
    }
    Ok(format!("Imported {name} ({w}×{h}) to the stage and library"))
}

/// Animated GIF → one keyframe per frame on a new layer. None = not animated.
fn import_animated_gif(app: &mut App, path: &str) -> Option<Result<String, String>> {
    use image::AnimationDecoder;
    let f = std::fs::File::open(path).ok()?;
    let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(f)).ok()?;
    let frames: Vec<image::Frame> = dec.into_frames().collect_frames().ok()?;
    if frames.len() < 2 {
        return None;
    }
    let name = file_name(path);
    let mut assets = Vec::new();
    for (i, fr) in frames.iter().enumerate() {
        let buf = fr.buffer();
        match BitmapAsset::from_rgba(0, &format!("{name} {:03}", i + 1), buf.width(), buf.height(), buf.as_raw()) {
            Ok(a) => assets.push(a),
            Err(e) => return Some(Err(e)),
        }
    }
    Some(import_frames(app, &name, assets))
}

/// Put a list of bitmaps on consecutive keyframes of a new layer.
fn import_frames(app: &mut App, name: &str, assets: Vec<BitmapAsset>) -> Result<String, String> {
    let n = assets.len();
    let (w, h) = assets.first().map(|a| (a.width as f32, a.height as f32)).unwrap_or((0.0, 0.0));
    let at = place_at(app, w, h);
    let mark = app.ed.undo_depth();
    app.ed.add_layer();
    let li = app.ed.layer;
    let start = app.ed.frame;
    let ids: Vec<u32> = assets.into_iter().map(|a| app.ed.add_bitmap(a)).collect();
    {
        let l = &mut app.ed.timeline_mut().layers[li];
        l.name = name.to_string();
        l.keyframes.clear();
        if start > 0 {
            l.keyframes.insert(0, Keyframe::blank());
        }
        for (k, id) in ids.iter().enumerate() {
            l.keyframes.insert(start + k as u32, Keyframe::with(vec![Element::bitmap("", *id, Matrix::translate(at.x, at.y))]));
        }
        l.end = start + n as u32;
    }
    app.ed.squash_undo(mark, "Import");
    Ok(format!("Imported {n} frames of {name} onto a new layer"))
}

/// File › Import › Image Sequence: `walk0001.png`, `walk0002.png`, … next to the picked file.
pub(crate) fn import_sequence(app: &mut App, path: &str) -> Result<String, String> {
    let p = PathBuf::from(path);
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = p.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let digits = stem.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return import_image(app, path);
    }
    let prefix = &stem[..stem.len() - digits];
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut files: Vec<(u64, PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|f| {
            let s = f.file_stem()?.to_string_lossy().to_string();
            let e = f.extension()?.to_string_lossy().to_string();
            if !e.eq_ignore_ascii_case(&ext) || !s.starts_with(prefix) {
                return None;
            }
            let num = &s[prefix.len()..];
            (!num.is_empty() && num.chars().all(|c| c.is_ascii_digit())).then(|| (num.parse().unwrap_or(0), f))
        })
        .collect();
    files.sort();
    let assets: Result<Vec<BitmapAsset>, String> = files.iter().map(|(_, f)| load_bitmap(&f.display().to_string())).collect();
    import_frames(app, prefix.trim_end_matches(['_', '-', ' ']), assets?)
}

// ------------------------------------------------------------------ SVG

fn color(c: usvg::Color, opacity: f32) -> Color {
    Color { r: c.red, g: c.green, b: c.blue, a: (opacity.clamp(0.0, 1.0) * 255.0).round() as u8 }
}

fn convert_path(p: &usvg::tiny_skia_path::Path) -> Path {
    use usvg::tiny_skia_path::PathSegment as S;
    let mut segs = Vec::new();
    for s in p.segments() {
        segs.push(match s {
            S::MoveTo(a) => Seg::M(pt(a.x, a.y)),
            S::LineTo(a) => Seg::L(pt(a.x, a.y)),
            S::QuadTo(c, a) => Seg::Q(pt(c.x, c.y), pt(a.x, a.y)),
            S::CubicTo(c1, c2, a) => Seg::C(pt(c1.x, c1.y), pt(c2.x, c2.y), pt(a.x, a.y)),
            S::Close => Seg::Z,
        });
    }
    Path { segs }
}

fn tf(t: usvg::Transform) -> Matrix {
    Matrix { a: t.sx, b: t.ky, c: t.kx, d: t.sy, tx: t.tx, ty: t.ty }
}

/// Fill paint → (solid colour, gradient in the path's local space).
fn paint(p: &usvg::Paint, opacity: f32) -> (Option<Color>, Option<Gradient>) {
    let stops = |s: &[usvg::Stop]| -> Vec<(f32, Color)> { s.iter().map(|st| (st.offset().get(), color(st.color(), st.opacity().get() * opacity))).collect() };
    match p {
        usvg::Paint::Color(c) => (Some(color(*c, opacity)), None),
        usvg::Paint::LinearGradient(g) => {
            let st = stops(g.stops());
            let (a, b) = (pt(g.x1(), g.y1()), pt(g.x2(), g.y2()));
            let len = a.dist(b).max(1e-3);
            let m = Matrix::translate((a.x + b.x) / 2.0, (a.y + b.y) / 2.0).concat(&Matrix::rotate((b.y - a.y).atan2(b.x - a.x))).concat(&Matrix::scale(len / 2.0, len / 2.0));
            (st.first().map(|s| s.1), Some(Gradient { kind: GradientKind::Linear, stops: st, matrix: tf(g.transform()).concat(&m) }))
        }
        usvg::Paint::RadialGradient(g) => {
            let st = stops(g.stops());
            let r = g.r().get();
            let m = Matrix::translate(g.cx(), g.cy()).concat(&Matrix::scale(r, r));
            (st.first().map(|s| s.1), Some(Gradient { kind: GradientKind::Radial, stops: st, matrix: tf(g.transform()).concat(&m) }))
        }
        usvg::Paint::Pattern(_) => (Some(Color::rgb(128, 128, 128)), None),
    }
}

fn walk(g: &usvg::Group, out: &mut Vec<Element>) {
    for node in g.children() {
        match node {
            usvg::Node::Group(c) => walk(c, out),
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let (fill, gradient) = p.fill().map(|f| paint(f.paint(), f.opacity().get())).unwrap_or((None, None));
                let stroke = p.stroke().and_then(|s| match s.paint() {
                    usvg::Paint::Color(c) => Some(Stroke { color: color(*c, s.opacity().get()), width: s.width().get() }),
                    other => paint(other, s.opacity().get()).0.map(|c| Stroke { color: c, width: s.width().get() }),
                });
                if fill.is_none() && stroke.is_none() {
                    continue;
                }
                let mut shape = Shape::new(convert_path(p.data()), fill, stroke);
                shape.gradient = gradient;
                let mut el = Element::shape(p.id(), shape);
                el.matrix = tf(p.abs_transform());
                out.push(el);
            }
            usvg::Node::Text(t) => walk(t.flattened(), out),
            usvg::Node::Image(_) => {}
        }
    }
}

pub(crate) fn import_svg(app: &mut App, path: &str) -> Result<String, String> {
    if !app.ed.layer_editable(app.ed.layer) {
        return Err("the current layer is locked or hidden".into());
    }
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("not a valid SVG ({e})"))?;
    let mut els = Vec::new();
    walk(tree.root(), &mut els);
    if els.is_empty() {
        return Err("the SVG has no visible shapes".into());
    }
    let n = els.len();
    let s = tree.size();
    let at = place_at(app, s.width(), s.height());
    let mut g = Element::group(els);
    g.name = file_name(path);
    g.matrix = Matrix::translate(at.x, at.y);
    match app.ed.add_element(g) {
        Some(sel) => {
            app.ed.selection = vec![sel];
            Ok(format!("Imported {} as a group of {n} shapes (Ctrl+Shift+G to ungroup)", file_name(path)))
        }
        None => Err("the current layer is locked or hidden".into()),
    }
}

/// Files dropped onto the window.
pub(crate) fn dropped(app: &mut App, path: &str) -> Result<String, String> {
    let l = path.to_lowercase();
    if l.ends_with(".svg") {
        import_svg(app, path)
    } else if [".wav", ".mp3", ".ogg", ".oga", ".flac"].iter().any(|e| l.ends_with(e)) {
        app.import_sound(path)
    } else if l.ends_with(".json") || l.ends_with(".fla") || l.ends_with(".xfl") {
        app.guarded(crate::files::Pending::OpenPath(path.to_string()));
        Ok(String::new())
    } else {
        import_image(app, path)
    }
}
