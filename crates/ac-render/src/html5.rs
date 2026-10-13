//! Publish ▸ HTML5 Canvas: a page that plays the movie in any browser.
//!
//! Every frame is evaluated with the same engine as the stage (tweens,
//! symbols, masks, camera, effects all resolved), and stored as a display
//! list over shared tables (paths, items, effect groups, masks), so static
//! content costs nothing per frame. A small runtime (`html5_runtime.js`)
//! draws the lists on a `<canvas>`, runs frame scripts with the same
//! CreateJS-style API as the editor (`this.stop()`, `this.ball.x`,
//! `addEventListener("click", …)`), drives buttons and plays sounds with
//! Animate's Event/Start/Stop/Stream rules.

use ac_core::fx::{BevelType, BlendMode, Filter};
use ac_core::model::*;
use ac_core::{Matrix, Path, Seg};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

pub const RUNTIME: &str = include_str!("html5_runtime.js");

#[derive(Clone, Debug, Default)]
pub struct Html5Opts {
    /// One self-contained .html (images and sounds inlined).
    pub single_file: bool,
    pub loop_play: bool,
    /// Frame range (0-based inclusive) or the whole movie.
    pub range: Option<(u32, u32)>,
}

/// Published files: (relative path, bytes).
pub type Files = Vec<(String, Vec<u8>)>;

fn n(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        let s = format!("{r:.2}");
        s.trim_end_matches('0').to_string()
    }
}
fn nm(m: &Matrix) -> String {
    let p = |v: f32| {
        let r = (v * 10000.0).round() / 10000.0;
        let s = format!("{r}");
        if s == "-0" { "0".into() } else { s }
    };
    format!("[{},{},{},{},{},{}]", p(m.a), p(m.b), p(m.c), p(m.d), n(m.tx), n(m.ty))
}
fn js_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '<' => o.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
fn svg_path(p: &Path) -> String {
    let mut s = String::new();
    for seg in &p.segs {
        match *seg {
            Seg::M(a) => {
                let _ = write!(s, "M{} {}", n(a.x), n(a.y));
            }
            Seg::L(a) => {
                let _ = write!(s, "L{} {}", n(a.x), n(a.y));
            }
            Seg::Q(c, a) => {
                let _ = write!(s, "Q{} {} {} {}", n(c.x), n(c.y), n(a.x), n(a.y));
            }
            Seg::C(c1, c2, a) => {
                let _ = write!(s, "C{} {} {} {} {} {}", n(c1.x), n(c1.y), n(c2.x), n(c2.y), n(a.x), n(a.y));
            }
            Seg::Z => s.push('Z'),
        }
    }
    s
}
fn rgba(c: Color, alpha: f32, tint: Option<(Color, f32)>, cx: Option<ColorXform>) -> String {
    let c = match tint {
        Some((t, amt)) => Color { a: c.a, ..c.lerp(Color { a: c.a, ..t }, amt.clamp(0.0, 1.0)) },
        None => c,
    };
    let c = cx.map(|x| x.apply(c)).unwrap_or(c);
    let a = c.a as f32 / 255.0 * alpha.clamp(0.0, 1.0);
    if a >= 0.999 {
        format!("\"#{:02x}{:02x}{:02x}\"", c.r, c.g, c.b)
    } else {
        format!("\"rgba({},{},{},{})\"", c.r, c.g, c.b, (a * 1000.0).round() / 1000.0)
    }
}

/// CSS filter + composite operation for an effect group.
fn fx_json(filters: &[Filter], blend: BlendMode) -> String {
    let css = |c: Color, strength: f32| format!("rgba({},{},{},{})", c.r, c.g, c.b, (c.a as f32 / 255.0 * (strength / 100.0).min(1.0) * 100.0).round() / 100.0);
    let mut f = vec![];
    let mut unsupported = false;
    for x in filters {
        match x {
            Filter::Blur { blur_x, blur_y, .. } => f.push(format!("blur({}px)", n((blur_x + blur_y) / 4.0))),
            Filter::DropShadow { blur_x, blur_y, strength, angle, distance, color, inner, knockout, hide_object, .. } => {
                unsupported |= *inner || *knockout || *hide_object;
                let (dx, dy) = (angle.to_radians().cos() * distance, angle.to_radians().sin() * distance);
                f.push(format!("drop-shadow({}px {}px {}px {})", n(dx), n(dy), n((blur_x + blur_y) / 4.0), css(*color, *strength)));
            }
            Filter::Glow { blur_x, blur_y, strength, color, inner, knockout, .. } => {
                unsupported |= *inner || *knockout;
                let k = ((strength / 100.0).ceil() as usize).clamp(1, 3);
                for _ in 0..k {
                    f.push(format!("drop-shadow(0 0 {}px {})", n((blur_x + blur_y) / 4.0), css(*color, 100.0)));
                }
            }
            Filter::Bevel { blur_x, blur_y, strength, shadow, highlight, angle, distance, kind, .. } => {
                unsupported |= *kind != BevelType::Outer;
                let (dx, dy) = (angle.to_radians().cos() * distance, angle.to_radians().sin() * distance);
                let b = n((blur_x + blur_y) / 4.0);
                f.push(format!("drop-shadow({}px {}px {b}px {})", n(-dx), n(-dy), css(*highlight, *strength)));
                f.push(format!("drop-shadow({}px {}px {b}px {})", n(dx), n(dy), css(*shadow, *strength)));
            }
            Filter::AdjustColor { brightness, contrast, saturation, hue } => {
                f.push(format!("brightness({}) contrast({}) saturate({}) hue-rotate({}deg)", n(1.0 + brightness / 100.0), n(1.0 + contrast / 100.0), n(1.0 + saturation / 100.0), n(*hue)));
            }
        }
    }
    let op = match blend {
        BlendMode::Normal | BlendMode::Layer => "source-over",
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::Lighten => "lighten",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::HardLight => "hard-light",
        BlendMode::Add => "lighter",
        BlendMode::Subtract => "difference",
        BlendMode::Difference => "difference",
        BlendMode::Invert => "exclusion",
        BlendMode::Alpha => "destination-in",
        BlendMode::Erase => "destination-out",
    };
    let _ = unsupported;
    format!("{{\"f\":{},\"o\":\"{op}\"}}", js_str(&f.join(" ")))
}

struct Tables {
    paths: Vec<String>,
    path_ix: HashMap<String, usize>,
    items: Vec<String>,
    item_ix: HashMap<String, usize>,
    fx: Vec<String>,
    fx_ix: HashMap<String, usize>,
    clips: Vec<String>,
    clip_ix: HashMap<String, usize>,
    images: Vec<(String, Vec<u8>, bool)>,
    image_ix: HashMap<usize, usize>,
}

fn intern(v: &mut Vec<String>, ix: &mut HashMap<String, usize>, s: String) -> usize {
    if let Some(&i) = ix.get(&s) {
        return i;
    }
    v.push(s.clone());
    ix.insert(s, v.len() - 1);
    v.len() - 1
}

impl Tables {
    fn path(&mut self, p: &Path) -> usize {
        intern(&mut self.paths, &mut self.path_ix, svg_path(p))
    }
    fn image(&mut self, img: &Arc<ac_core::bitmap::Image>) -> usize {
        let key = Arc::as_ptr(img) as usize;
        if let Some(&i) = self.image_ix.get(&key) {
            return i;
        }
        // re-encode the decoded (premultiplied) pixels as PNG
        let mut rgba = Vec::with_capacity(img.rgba.len());
        for px in img.rgba.chunks_exact(4) {
            let a = px[3] as u32;
            let un = |c: u8| (c as u32 * 255 + a / 2).checked_div(a).unwrap_or(0).min(255) as u8;
            rgba.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), px[3]]);
        }
        let mut png_bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png_bytes, img.width, img.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            if let Ok(mut w) = enc.write_header() {
                let _ = w.write_image_data(&rgba);
            }
        }
        let name = format!("images/img{}.png", self.images.len() + 1);
        self.images.push((name, png_bytes, img.smoothing));
        self.image_ix.insert(key, self.images.len() - 1);
        self.images.len() - 1
    }
    fn clip(&mut self, c: &ac_core::Clip) -> usize {
        let parts: Vec<String> = c.shapes.iter().map(|(p, m)| format!("[{},{}]", self.path(p), nm(m))).collect();
        intern(&mut self.clips, &mut self.clip_ix, format!("[{}]", parts.join(",")))
    }
    fn item(&mut self, it: &DrawItem) -> usize {
        let mut s = format!("{{\"p\":{},\"m\":{}", self.path(&it.path), nm(&it.matrix));
        let a = it.alpha;
        if let Some(oc) = it.outline {
            let _ = write!(s, ",\"ol\":{}", rgba(oc, a, None, None));
        } else if let Some(img) = &it.image {
            let _ = write!(s, ",\"img\":{}", self.image(img));
            if a < 0.999 {
                let _ = write!(s, ",\"a\":{}", n(a));
            }
        } else if let Some((img, pm)) = &it.pattern {
            let _ = write!(s, ",\"pat\":{},\"pm\":{}", self.image(img), nm(pm));
            if a < 0.999 {
                let _ = write!(s, ",\"a\":{}", n(a));
            }
        } else if let Some(g) = &it.gradient {
            let stops: Vec<String> = g.stops.iter().map(|(t, c)| format!("[{},{}]", n(*t), rgba(*c, a, it.tint, it.cx))).collect();
            let _ = write!(s, ",\"g\":{{\"k\":{},\"m\":{},\"s\":[{}]}}", if g.kind == GradientKind::Linear { 0 } else { 1 }, nm(&g.matrix), stops.join(","));
        } else if let Some(f) = it.fill {
            let _ = write!(s, ",\"f\":{}", rgba(f, a, it.tint, it.cx));
        }
        if it.outline.is_none() {
            if let Some(st) = it.stroke {
                let _ = write!(s, ",\"s\":{},\"w\":{}", rgba(st.color, a, it.tint, it.cx), n(st.width.max(0.1)));
                let ss = &it.stroke_style;
                let cap = match (ss.dash, ss.cap) {
                    (Dash::Dotted, _) | (_, Cap::Round) => "round",
                    (_, Cap::None) => "butt",
                    (_, Cap::Square) => "square",
                };
                let join = match ss.join {
                    Join::Miter => "miter",
                    Join::Round => "round",
                    Join::Bevel => "bevel",
                };
                if cap != "round" || join != "round" {
                    let _ = write!(s, ",\"cj\":\"{cap} {join}\"");
                }
                let w = st.width.max(0.1);
                match ss.dash {
                    Dash::Solid => {}
                    Dash::Dashed => {
                        let _ = write!(s, ",\"d\":[{},{}]", n((ss.dash_len * w).max(0.5)), n((ss.gap * w).max(0.5)));
                    }
                    Dash::Dotted => {
                        let _ = write!(s, ",\"d\":[0.01,{}]", n((ss.gap * w).max(w * 1.2)));
                    }
                }
            }
        }
        if !it.clips.is_empty() {
            let c: Vec<String> = it.clips.iter().map(|c| self.clip(c).to_string()).collect();
            let _ = write!(s, ",\"c\":[{}]", c.join(","));
        }
        if let Some((l, e)) = it.src {
            let _ = write!(s, ",\"src\":\"{l}:{e}\"");
        }
        s.push('}');
        intern(&mut self.items, &mut self.item_ix, s)
    }

    /// A frame's items as a display-list tree: item indices, and effect
    /// groups `{"x": fx, "c": [...]}` drawn through an offscreen canvas.
    fn list(&mut self, items: &[DrawItem], level: usize) -> String {
        let mut out = vec![];
        let mut i = 0;
        while i < items.len() {
            let Some(g) = items[i].fx.get(level).cloned() else {
                out.push(self.item(&items[i]).to_string());
                i += 1;
                continue;
            };
            let mut j = i + 1;
            while j < items.len() && items[j].fx.get(level).is_some_and(|h| Arc::ptr_eq(h, &g)) {
                j += 1;
            }
            let fx = intern(&mut self.fx, &mut self.fx_ix, fx_json(&g.filters, g.blend));
            let inner = self.list(&items[i..j], level + 1);
            out.push(format!("{{\"x\":{fx},\"c\":{inner}}}"));
            i = j;
        }
        format!("[{}]", out.join(","))
    }
}

fn b64(data: &[u8]) -> String {
    crate::base64_lite::encode(data)
}

fn mime_of(data: &[u8]) -> &'static str {
    if data.starts_with(b"RIFF") {
        "audio/wav"
    } else if data.starts_with(b"OggS") {
        "audio/ogg"
    } else if data.starts_with(b"fLaC") {
        "audio/flac"
    } else {
        "audio/mpeg"
    }
}

/// Build the published files.
pub fn publish(doc: &Document, o: &Html5Opts) -> Files {
    let total = doc.total_frames();
    let (f0, f1) = o.range.unwrap_or((0, total - 1));
    let (f0, f1) = (f0.min(total - 1), f1.min(total - 1).max(f0.min(total - 1)));
    let mut t = Tables { paths: vec![], path_ix: HashMap::new(), items: vec![], item_ix: HashMap::new(), fx: vec![], fx_ix: HashMap::new(), clips: vec![], clip_ix: HashMap::new(), images: vec![], image_ix: HashMap::new() };
    let mut frames = vec![];
    let mut extra = vec![]; // per frame: buttons + named instances + scripts + sounds
    for f in f0..=f1 {
        let items = evaluate_scene(doc, f);
        frames.push(t.list(&items, 0));
        let (si, local) = doc.locate_frame(f);
        let scene = doc.scene_at(si).unwrap_or(&doc.scene);
        let view = camera_view(doc, scene, local);
        let mut parts = vec![];
        // buttons: Over / Down states and the hit area
        let mut buttons = vec![];
        let mut named = vec![];
        for (li, layer) in scene.layers.iter().enumerate() {
            if layer.kind != LayerKind::Normal || !layer_shown(scene, li) {
                continue;
            }
            let Some((els, _)) = layer_elements_at(layer, local) else { continue };
            for (ei, el) in els.iter().enumerate() {
                if !el.name.is_empty() && !matches!(el.kind, ElementKind::Shape(_) | ElementKind::Canvas(_)) && !named.iter().any(|(nn, _): &(String, String)| *nn == el.name) {
                    let d = el.matrix.decompose();
                    named.push((el.name.clone(), format!("{{\"src\":\"{li}:{ei}\",\"x\":{},\"y\":{},\"r\":{},\"sx\":{},\"sy\":{},\"a\":{},\"m\":{}}}", n(el.matrix.tx), n(el.matrix.ty), n(d.skew_y.to_degrees()), n(d.scale_x), n(d.scale_y), n(el.alpha), nm(&el.matrix))));
                }
                let ElementKind::Instance(inst) = &el.kind else { continue };
                let Some(sym) = doc.symbol(inst.symbol) else { continue };
                if sym.kind != SymbolKind::Button {
                    continue;
                }
                let state = |s: u32| {
                    let ctx = EvalCtx { button: Some(((li, ei), s)), ..EvalCtx::new(doc, f) };
                    let mut out = vec![];
                    evaluate(&ctx, scene, local, &Matrix::IDENTITY, 1.0, 0, &mut out);
                    if let Some(v) = &view {
                        apply_view(&mut out, v);
                    }
                    out.retain(|it| it.src == Some((li, ei)));
                    out
                };
                let over = state(1);
                let down = state(2);
                let dur = sym.timeline.frame_count();
                let m = view.map(|v| v.concat(&el.matrix)).unwrap_or(el.matrix);
                let mut hit = vec![];
                let ctx = EvalCtx::new(doc, f);
                evaluate(&ctx, &sym.timeline, if dur >= 4 { 3 } else { 0 }, &m, 1.0, 1, &mut hit);
                let hits: Vec<String> = hit.iter().map(|h| format!("[{},{}]", t.path(&h.path), nm(&h.matrix))).collect();
                let ov = t.list(&over, 0);
                let dn = t.list(&down, 0);
                buttons.push(format!("{{\"src\":\"{li}:{ei}\",\"name\":{},\"over\":{ov},\"down\":{dn},\"hit\":[{}]}}", js_str(&el.name), hits.join(",")));
            }
        }
        if !buttons.is_empty() {
            parts.push(format!("\"b\":[{}]", buttons.join(",")));
        }
        if !named.is_empty() {
            let v: Vec<String> = named.iter().map(|(k, v)| format!("{}:{v}", js_str(k))).collect();
            parts.push(format!("\"n\":{{{}}}", v.join(",")));
        }
        // keyframe scripts and sounds that start here
        let mut scripts = vec![];
        let mut sounds = vec![];
        for l in &scene.layers {
            if let Some(k) = l.keyframes.get(&local) {
                if let Some(sc) = k.script.as_ref().filter(|s| !s.trim().is_empty()) {
                    scripts.push(js_str(sc));
                }
                if let (Some(fs), false) = (&k.sound, matches!(l.kind, LayerKind::Guide | LayerKind::Folder | LayerKind::Camera)) {
                    if let Some(ix) = doc.sounds.iter().position(|s| s.id == fs.sound) {
                        let end = doc.scene_start(si) + l.span_end(local);
                        sounds.push(format!("{{\"s\":{ix},\"y\":\"{}\",\"r\":{},\"l\":{},\"e\":{}}}", fs.sync.name().to_lowercase(), fs.repeat.max(1), fs.forever, end.saturating_sub(f0)));
                    }
                }
            }
        }
        if !scripts.is_empty() {
            parts.push(format!("\"js\":[{}]", scripts.join(",")));
        }
        if !sounds.is_empty() {
            parts.push(format!("\"snd\":[{}]", sounds.join(",")));
        }
        extra.push(if parts.is_empty() { "0".to_string() } else { format!("{{{}}}", parts.join(",")) });
    }
    // scenes (for currentFrame / gotoAndPlay inside a scene) and labels
    let mut scenes = vec![];
    let mut labels = vec![];
    for (i, sc) in doc.scenes().enumerate() {
        let st = doc.scene_start(i) as i64 - f0 as i64;
        scenes.push(format!("{{\"name\":{},\"start\":{st},\"count\":{}}}", js_str(&sc.name), sc.frame_count()));
        for l in &sc.layers {
            for (fr, k) in &l.keyframes {
                if let Some(lb) = &k.label {
                    labels.push(format!("[{},{},{}]", js_str(lb), i, fr));
                }
            }
        }
    }
    let images: Vec<String> = t
        .images
        .iter()
        .map(|(name, bytes, smooth)| {
            let src = if o.single_file { format!("data:image/png;base64,{}", b64(bytes)) } else { name.clone() };
            format!("[{},{}]", js_str(&src), smooth)
        })
        .collect();
    let sounds: Vec<String> = doc.sounds.iter().map(|s| format!("[{},{}]", js_str(&s.name), js_str(&format!("data:{};base64,{}", mime_of(&s.data), b64(&s.data))))).collect();
    let bg = doc.background;
    let mut data = String::with_capacity(1 << 16);
    let _ = write!(
        data,
        "var AC_MOVIE={{\"w\":{},\"h\":{},\"fps\":{},\"bg\":\"#{:02x}{:02x}{:02x}\",\"loop\":{},\"scenes\":[{}],\"labels\":[{}],\"paths\":[{}],\"items\":[{}],\"fx\":[{}],\"clips\":[{}],\"images\":[{}],\"sounds\":[{}],\"frames\":[{}],\"extra\":[{}]}};",
        n(doc.width),
        n(doc.height),
        n(doc.fps),
        bg.r,
        bg.g,
        bg.b,
        o.loop_play,
        scenes.join(","),
        labels.join(","),
        t.paths.iter().map(|p| js_str(p)).collect::<Vec<_>>().join(","),
        t.items.join(","),
        t.fx.join(","),
        t.clips.join(","),
        images.join(","),
        sounds.join(","),
        frames.join(",\n"),
        extra.join(",")
    );
    data.push('\n');
    let title = "AnimationCraft movie";
    let mut files: Files = vec![];
    let html = |scripts: &str| {
        format!(
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"AnimationCraft\">\n<title>{title}</title>\n<style>html,body{{margin:0;height:100%;background:#1e1e1e;display:flex;align-items:center;justify-content:center;overflow:hidden}}canvas{{display:block;max-width:100vw;max-height:100vh;width:auto;height:auto;image-rendering:auto}}</style>\n</head>\n<body>\n<canvas id=\"ac-stage\" width=\"{}\" height=\"{}\"></canvas>\n{scripts}</body>\n</html>\n",
            doc.width.round() as u32,
            doc.height.round() as u32
        )
    };
    if o.single_file {
        files.push(("index.html".into(), html(&format!("<script>\n{data}</script>\n<script>\n{RUNTIME}</script>\n")).into_bytes()));
    } else {
        files.push(("index.html".into(), html("<script src=\"movie.js\"></script>\n<script src=\"animationcraft-runtime.js\"></script>\n").into_bytes()));
        files.push(("movie.js".into(), data.into_bytes()));
        files.push(("animationcraft-runtime.js".into(), RUNTIME.as_bytes().to_vec()));
        for (name, bytes, _) in t.images {
            files.push((name, bytes));
        }
    }
    files
}

/// Publish to a folder (index.html + movie.js + runtime + images/) or, for
/// a path ending in .html, one self-contained page.
pub fn publish_to(doc: &Document, o: &Html5Opts, path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let single = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
    let o = Html5Opts { single_file: single || o.single_file, ..o.clone() };
    let files = publish(doc, &o);
    if single {
        std::fs::write(path, &files[0].1).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(path.to_path_buf());
    }
    for (name, bytes) in files {
        let p = path.join(&name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(path.join("index.html"))
}
