//! Rendering back-ends. The raster path is shared by the editor stage and
//! every export so what you see is exactly what you export.

use ac_core::{evaluate_scene, Color, Document, DrawItem, Matrix, Path, Seg};
use tiny_skia as sk;

pub struct RenderOpts {
    /// Extra transform applied on top of document coordinates (zoom/pan).
    pub view: Matrix,
    /// Multiply all alpha (used for onion skin ghosts).
    pub alpha: f32,
    /// Replace colours with this tint (onion skin), amount 0..1.
    pub tint: Option<(Color, f32)>,
}

impl Default for RenderOpts {
    fn default() -> Self {
        RenderOpts { view: Matrix::IDENTITY, alpha: 1.0, tint: None }
    }
}

fn to_sk_path(p: &Path) -> Option<sk::Path> {
    let mut b = sk::PathBuilder::new();
    for s in &p.segs {
        match *s {
            Seg::M(a) => b.move_to(a.x, a.y),
            Seg::L(a) => b.line_to(a.x, a.y),
            Seg::Q(c, a) => b.quad_to(c.x, c.y, a.x, a.y),
            Seg::C(c1, c2, a) => b.cubic_to(c1.x, c1.y, c2.x, c2.y, a.x, a.y),
            Seg::Z => b.close(),
        }
    }
    b.finish()
}

fn to_sk_transform(m: &Matrix) -> sk::Transform {
    sk::Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty)
}

fn shade(c: Color, alpha: f32, tint: Option<(Color, f32)>) -> sk::Color {
    let c = match tint {
        Some((t, amt)) => Color { a: c.a, ..c.lerp(Color { a: c.a, ..t }, amt.clamp(0.0, 1.0)) },
        None => c,
    };
    sk::Color::from_rgba8(c.r, c.g, c.b, ((c.a as f32) * alpha.clamp(0.0, 1.0)).round() as u8)
}

pub fn draw_items(pix: &mut sk::PixmapMut, items: &[DrawItem], opts: &RenderOpts) {
    for it in items {
        let Some(path) = to_sk_path(&it.path) else { continue };
        let m = opts.view.concat(&it.matrix);
        let ts = to_sk_transform(&m);
        let alpha = it.alpha * opts.alpha;
        let tint = opts.tint.or(it.tint);
        if let Some(oc) = it.outline {
            // Outline mode: thin hairline in the layer colour.
            let mut paint = sk::Paint::default();
            paint.set_color(shade(oc, alpha, None));
            paint.anti_alias = true;
            let stroke = sk::Stroke { width: 1.0, ..Default::default() };
            if let Some(p) = path.clone().transform(ts) {
                pix.stroke_path(&p, &paint, &stroke, sk::Transform::identity(), None);
            }
            continue;
        }
        if let Some(g) = &it.gradient {
            let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
            let stops: Vec<sk::GradientStop> = g.stops.iter().map(|(t, c)| sk::GradientStop::new(t.clamp(0.0, 1.0), shade(*c, alpha, tint))).collect();
            let gts = to_sk_transform(&g.matrix); // tiny-skia applies the fill transform to the shader too
            let shader = match g.kind {
                ac_core::GradientKind::Linear => sk::LinearGradient::new(sk::Point::from_xy(-1.0, 0.0), sk::Point::from_xy(1.0, 0.0), stops, sk::SpreadMode::Pad, gts),
                ac_core::GradientKind::Radial => sk::RadialGradient::new(sk::Point::from_xy(0.0, 0.0), sk::Point::from_xy(0.0, 0.0), 1.0, stops, sk::SpreadMode::Pad, gts),
            };
            match shader {
                Some(sh) => {
                    paint.shader = sh;
                    pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, ts, None);
                }
                None => {
                    if let Some(c) = g.stops.first().map(|s| s.1).or(it.fill) {
                        paint.set_color(shade(c, alpha, tint));
                        pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, ts, None);
                    }
                }
            }
        } else if let Some(fill) = it.fill {
            let mut paint = sk::Paint::default();
            paint.set_color(shade(fill, alpha, tint));
            paint.anti_alias = true;
            pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, ts, None);
        }
        if let Some(st) = it.stroke {
            let mut paint = sk::Paint::default();
            paint.set_color(shade(st.color, alpha, tint));
            paint.anti_alias = true;
            let stroke = sk::Stroke { width: st.width.max(0.1), line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() };
            pix.stroke_path(&path, &paint, &stroke, ts, None);
        }
    }
}

/// Render a scene frame at `scale` into an RGBA pixmap (stage-sized, clipped).
pub fn render_frame(doc: &Document, frame: u32, scale: f32) -> sk::Pixmap {
    let w = (doc.width * scale).round().max(1.0) as u32;
    let h = (doc.height * scale).round().max(1.0) as u32;
    let mut pix = sk::Pixmap::new(w, h).expect("pixmap");
    let bg = doc.background;
    pix.fill(sk::Color::from_rgba8(bg.r, bg.g, bg.b, 255));
    let items = evaluate_scene(doc, frame);
    draw_items(&mut pix.as_mut(), &items, &RenderOpts { view: Matrix::scale(scale, scale), ..Default::default() });
    pix
}

/// Straight (un-premultiplied) RGBA bytes.
pub fn unpremultiplied(pix: &sk::Pixmap) -> Vec<u8> {
    pix.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()
}

pub fn export_png(doc: &Document, frame: u32, scale: f32, path: &std::path::Path) -> Result<(), String> {
    render_frame(doc, frame, scale).save_png(path).map_err(|e| e.to_string())
}

pub fn export_gif(doc: &Document, scale: f32, path: &std::path::Path) -> Result<(), String> {
    let first = render_frame(doc, 0, scale);
    let (w, h) = (first.width() as u16, first.height() as u16);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = gif::Encoder::new(file, w, h, &[]).map_err(|e| e.to_string())?;
    enc.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
    let delay = (100.0 / doc.fps).round() as u16;
    for f in 0..doc.scene.frame_count() {
        let pix = render_frame(doc, f, scale);
        let mut rgba = unpremultiplied(&pix);
        let mut frame = gif::Frame::from_rgba_speed(w, h, &mut rgba, 10);
        frame.delay = delay;
        enc.write_frame(&frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn svg_path(p: &Path) -> String {
    let mut s = String::new();
    for seg in &p.segs {
        use std::fmt::Write;
        let _ = match *seg {
            Seg::M(a) => write!(s, "M{:.2} {:.2}", a.x, a.y),
            Seg::L(a) => write!(s, "L{:.2} {:.2}", a.x, a.y),
            Seg::Q(c, a) => write!(s, "Q{:.2} {:.2} {:.2} {:.2}", c.x, c.y, a.x, a.y),
            Seg::C(c1, c2, a) => write!(s, "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}", c1.x, c1.y, c2.x, c2.y, a.x, a.y),
            Seg::Z => write!(s, "Z"),
        };
    }
    s
}

/// Export one frame as a standalone SVG.
pub fn export_svg(doc: &Document, frame: u32) -> String {
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n<rect width=\"100%\" height=\"100%\" fill=\"{bg}\"/>\n",
        w = doc.width,
        h = doc.height,
        bg = doc.background.to_hex()
    );
    let mut defs = String::new();
    let mut body = String::new();
    for (gi, it) in evaluate_scene(doc, frame).into_iter().enumerate() {
        let m = it.matrix;
        let col = |c: Color| {
            let c = match it.tint { Some((t, a)) => c.lerp(t, a), None => c };
            (c.to_hex(), c.a as f32 / 255.0 * it.alpha)
        };
        let fill = match (&it.gradient, it.fill) {
            (Some(g), _) => {
                let gm = g.matrix;
                let tag = if g.kind == ac_core::GradientKind::Linear { "linearGradient" } else { "radialGradient" };
                let geo = if g.kind == ac_core::GradientKind::Linear { "x1=\"-1\" y1=\"0\" x2=\"1\" y2=\"0\"" } else { "cx=\"0\" cy=\"0\" r=\"1\"" };
                defs += &format!("<{tag} id=\"g{gi}\" gradientUnits=\"userSpaceOnUse\" {geo} gradientTransform=\"matrix({:.4} {:.4} {:.4} {:.4} {:.2} {:.2})\">", gm.a, gm.b, gm.c, gm.d, gm.tx, gm.ty);
                for (t, c) in &g.stops {
                    let (h, a) = col(*c);
                    defs += &format!("<stop offset=\"{t:.3}\" stop-color=\"{h}\" stop-opacity=\"{a:.3}\"/>");
                }
                defs += &format!("</{tag}>\n");
                format!("fill=\"url(#g{gi})\" fill-rule=\"evenodd\"")
            }
            (None, Some(c)) => { let (h, a) = col(c); format!("fill=\"{h}\" fill-opacity=\"{a:.3}\" fill-rule=\"evenodd\"") }
            (None, None) => "fill=\"none\"".into(),
        };
        let stroke = match it.stroke {
            Some(s) => { let (h, a) = col(s.color); format!(" stroke=\"{h}\" stroke-opacity=\"{a:.3}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"", s.width) }
            None => String::new(),
        };
        body += &format!(
            "<path transform=\"matrix({:.4} {:.4} {:.4} {:.4} {:.2} {:.2})\" d=\"{}\" {}{}/>\n",
            m.a, m.b, m.c, m.d, m.tx, m.ty, svg_path(&it.path), fill, stroke
        );
    }
    if !defs.is_empty() {
        out += &format!("<defs>\n{defs}</defs>\n");
    }
    out + &body + "</svg>\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_demo_frames() {
        let doc = ac_core::demo::demo_document();
        let a = render_frame(&doc, 0, 1.0);
        let b = render_frame(&doc, 9, 1.0);
        assert_eq!((a.width(), a.height()), (550, 400));
        assert_ne!(a.data(), b.data(), "animation changes pixels");
        // ground is green at the bottom-left
        let p = a.pixel(5, 395).unwrap().demultiply();
        assert!(p.green() > p.red() && p.green() > p.blue());
    }

    #[test]
    fn gradient_follows_object_and_view() {
        use ac_core::{Element, Gradient, GradientKind, Shape};
        let mut doc = Document { width: 200.0, height: 200.0, ..Default::default() };
        let mut sh = Shape::new(Path::rect(-40.0, -40.0, 80.0, 80.0), Some(Color::rgb(0, 0, 255)), None);
        sh.gradient = Some(Gradient::fit(GradientKind::Radial, vec![(0.0, Color::rgb(0, 0, 255)), (1.0, Color::rgb(255, 255, 255))], ac_core::pt(-40.0, -40.0), ac_core::pt(40.0, 40.0)));
        let mut el = Element::shape("g", sh);
        el.matrix = Matrix::translate(100.0, 100.0);
        doc.scene.layers[0].keyframes.insert(0, ac_core::Keyframe::with(vec![el]));
        for scale in [1.0, 2.0] {
            let pix = render_frame(&doc, 0, scale);
            let c = pix.pixel((100.0 * scale) as u32, (100.0 * scale) as u32).unwrap().demultiply();
            assert!(c.blue() > 200 && c.red() < 60, "centre is the first stop at scale {scale}: {c:?}");
            let e = pix.pixel((139.0 * scale) as u32, (139.0 * scale) as u32).unwrap().demultiply();
            assert!(e.red() > 150, "corner is near the last stop at scale {scale}: {e:?}");
        }
    }

    #[test]
    fn svg_is_well_formed_ish() {
        let doc = ac_core::demo::demo_document();
        let s = export_svg(&doc, 12);
        assert!(s.starts_with("<svg") && s.trim_end().ends_with("</svg>"));
        assert!(s.matches("<path").count() > 10);
    }
}
