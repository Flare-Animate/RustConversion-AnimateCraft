//! Geometry primitives: points, Flash-style affine matrices, vector paths.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

pub const fn pt(x: f32, y: f32) -> Pt {
    Pt { x, y }
}

impl Pt {
    pub fn lerp(self, o: Pt, t: f32) -> Pt {
        pt(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    pub fn dist(self, o: Pt) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
}

/// Affine matrix in Flash/Animate layout:
/// `x' = a*x + c*y + tx`, `y' = b*x + d*y + ty`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Matrix decomposed the way Animate's classic tween sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decomposed {
    pub tx: f32,
    pub ty: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    /// radians
    pub skew_x: f32,
    /// radians; equals rotation when there is no skew
    pub skew_y: f32,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn translate(tx: f32, ty: f32) -> Self {
        Matrix { tx, ty, ..Self::IDENTITY }
    }
    pub fn scale(sx: f32, sy: f32) -> Self {
        Matrix { a: sx, d: sy, ..Self::IDENTITY }
    }
    pub fn rotate(rad: f32) -> Self {
        let (s, c) = rad.sin_cos();
        Matrix { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
    }

    pub fn apply(&self, p: Pt) -> Pt {
        pt(self.a * p.x + self.c * p.y + self.tx, self.b * p.x + self.d * p.y + self.ty)
    }

    /// `self * other`: apply `other` first, then `self`.
    pub fn concat(&self, o: &Matrix) -> Matrix {
        Matrix {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }

    pub fn invert(&self) -> Option<Matrix> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-9 {
            return None;
        }
        let id = 1.0 / det;
        Some(Matrix {
            a: self.d * id,
            b: -self.b * id,
            c: -self.c * id,
            d: self.a * id,
            tx: (self.c * self.ty - self.d * self.tx) * id,
            ty: (self.b * self.tx - self.a * self.ty) * id,
        })
    }

    pub fn decompose(&self) -> Decomposed {
        let scale_x = (self.a * self.a + self.b * self.b).sqrt();
        let mut scale_y = (self.c * self.c + self.d * self.d).sqrt();
        let det = self.a * self.d - self.b * self.c;
        let skew_y = self.b.atan2(self.a);
        let mut skew_x = (-self.c).atan2(self.d);
        if det < 0.0 {
            // a mirrored matrix: keep scale_x positive, flip y
            scale_y = -scale_y;
            skew_x = (self.c).atan2(-self.d);
        }
        Decomposed { tx: self.tx, ty: self.ty, scale_x, scale_y, skew_x, skew_y }
    }

    pub fn compose(d: &Decomposed) -> Matrix {
        Matrix {
            a: d.scale_x * d.skew_y.cos(),
            b: d.scale_x * d.skew_y.sin(),
            c: -d.scale_y * d.skew_x.sin(),
            d: d.scale_y * d.skew_x.cos(),
            tx: d.tx,
            ty: d.ty,
        }
    }

    /// Classic-tween interpolation: decompose both ends, interpolate scale,
    /// skew (shortest way round, plus `extra_turns` full CW rotations) and translation.
    pub fn interpolate(&self, o: &Matrix, t: f32, extra_turns: i32) -> Matrix {
        let a = self.decompose();
        let b = o.decompose();
        let turn = extra_turns as f32 * 2.0 * PI;
        let ang = |x: f32, y: f32| x + (shortest(x, y) + turn) * t;
        Matrix::compose(&Decomposed {
            tx: a.tx + (b.tx - a.tx) * t,
            ty: a.ty + (b.ty - a.ty) * t,
            scale_x: a.scale_x + (b.scale_x - a.scale_x) * t,
            scale_y: a.scale_y + (b.scale_y - a.scale_y) * t,
            skew_x: ang(a.skew_x, b.skew_x),
            skew_y: ang(a.skew_y, b.skew_y),
        })
    }
}

fn shortest(from: f32, to: f32) -> f32 {
    let mut d = (to - from) % (2.0 * PI);
    if d > PI {
        d -= 2.0 * PI;
    }
    if d < -PI {
        d += 2.0 * PI;
    }
    d
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Seg {
    M(Pt),
    L(Pt),
    Q(Pt, Pt),
    C(Pt, Pt, Pt),
    Z,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub segs: Vec<Seg>,
}

impl Path {
    pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
        Path {
            segs: vec![
                Seg::M(pt(x, y)),
                Seg::L(pt(x + w, y)),
                Seg::L(pt(x + w, y + h)),
                Seg::L(pt(x, y + h)),
                Seg::Z,
            ],
        }
    }

    /// Ellipse from 4 cubic arcs.
    pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
        const K: f32 = 0.552_284_8;
        let (ox, oy) = (rx * K, ry * K);
        Path {
            segs: vec![
                Seg::M(pt(cx + rx, cy)),
                Seg::C(pt(cx + rx, cy + oy), pt(cx + ox, cy + ry), pt(cx, cy + ry)),
                Seg::C(pt(cx - ox, cy + ry), pt(cx - rx, cy + oy), pt(cx - rx, cy)),
                Seg::C(pt(cx - rx, cy - oy), pt(cx - ox, cy - ry), pt(cx, cy - ry)),
                Seg::C(pt(cx + ox, cy - ry), pt(cx + rx, cy - oy), pt(cx + rx, cy)),
                Seg::Z,
            ],
        }
    }

    pub fn polygon(points: &[Pt]) -> Path {
        let mut segs = Vec::with_capacity(points.len() + 1);
        for (i, p) in points.iter().enumerate() {
            segs.push(if i == 0 { Seg::M(*p) } else { Seg::L(*p) });
        }
        segs.push(Seg::Z);
        Path { segs }
    }

    pub fn star(cx: f32, cy: f32, r_out: f32, r_in: f32, points: usize) -> Path {
        let n = points * 2;
        let pts: Vec<Pt> = (0..n)
            .map(|i| {
                let r = if i % 2 == 0 { r_out } else { r_in };
                let a = -PI / 2.0 + i as f32 * PI / points as f32;
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        Path::polygon(&pts)
    }

    /// Smooth open curve through freehand points (pencil "smooth" mode):
    /// quadratic segments through the midpoints of a simplified polyline.
    pub fn smooth_polyline(raw: &[Pt], tolerance: f32) -> Path {
        let pts = simplify(raw, tolerance);
        let mut segs = Vec::new();
        if pts.is_empty() {
            return Path { segs };
        }
        segs.push(Seg::M(pts[0]));
        if pts.len() < 3 {
            for p in &pts[1..] {
                segs.push(Seg::L(*p));
            }
            return Path { segs };
        }
        for i in 1..pts.len() - 1 {
            let mid = pts[i].lerp(pts[i + 1], 0.5);
            let end = if i == pts.len() - 2 { pts[i + 1] } else { mid };
            segs.push(Seg::Q(pts[i], end));
        }
        Path { segs }
    }

    pub fn transformed(&self, m: &Matrix) -> Path {
        let f = |p: &Pt| m.apply(*p);
        Path {
            segs: self
                .segs
                .iter()
                .map(|s| match s {
                    Seg::M(p) => Seg::M(f(p)),
                    Seg::L(p) => Seg::L(f(p)),
                    Seg::Q(a, b) => Seg::Q(f(a), f(b)),
                    Seg::C(a, b, c) => Seg::C(f(a), f(b), f(c)),
                    Seg::Z => Seg::Z,
                })
                .collect(),
        }
    }

    /// Flatten into polylines (one per subpath). `closed` flag per polyline.
    pub fn flatten(&self, steps: usize) -> Vec<(Vec<Pt>, bool)> {
        let mut out: Vec<(Vec<Pt>, bool)> = Vec::new();
        let mut cur: Vec<Pt> = Vec::new();
        let mut last = Pt::default();
        let mut start = Pt::default();
        for s in &self.segs {
            match *s {
                Seg::M(p) => {
                    if cur.len() > 1 {
                        out.push((std::mem::take(&mut cur), false));
                    }
                    cur.clear();
                    cur.push(p);
                    last = p;
                    start = p;
                }
                Seg::L(p) => {
                    cur.push(p);
                    last = p;
                }
                Seg::Q(c, p) => {
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        cur.push(last.lerp(c, t).lerp(c.lerp(p, t), t));
                    }
                    last = p;
                }
                Seg::C(c1, c2, p) => {
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        cur.push(cubic_at(last, c1, c2, p, t));
                    }
                    last = p;
                }
                Seg::Z => {
                    if cur.len() > 1 {
                        out.push((std::mem::take(&mut cur), true));
                    }
                    last = start;
                }
            }
        }
        if cur.len() > 1 {
            out.push((cur, false));
        }
        out
    }

    pub fn bounds(&self) -> Option<(Pt, Pt)> {
        let mut it = self.flatten(8).into_iter().flat_map(|(v, _)| v);
        let first = it.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in it {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
        Some((lo, hi))
    }

    /// Hit test in path-local coordinates: inside a closed subpath (even-odd),
    /// or within `stroke_tol` of any edge.
    pub fn hit(&self, p: Pt, filled: bool, stroke_tol: f32) -> bool {
        for (poly, closed) in self.flatten(12) {
            if filled && closed && point_in_poly(&poly, p) {
                return true;
            }
            let n = poly.len();
            let edges = if closed { n } else { n - 1 };
            for i in 0..edges {
                if dist_to_seg(p, poly[i], poly[(i + 1) % n]) <= stroke_tol {
                    return true;
                }
            }
        }
        false
    }

    /// Normalise to subpaths made only of cubic segments (used by shape tweens).
    pub fn to_cubic_subpaths(&self) -> Vec<CubicSubpath> {
        let mut out = Vec::new();
        let mut cur: Option<CubicSubpath> = None;
        let mut last = Pt::default();
        for s in &self.segs {
            match *s {
                Seg::M(p) => {
                    if let Some(c) = cur.take() {
                        out.push(c);
                    }
                    cur = Some(CubicSubpath { start: p, curves: vec![], closed: false });
                    last = p;
                }
                Seg::L(p) => {
                    if let Some(c) = cur.as_mut() {
                        c.curves.push([last.lerp(p, 1.0 / 3.0), last.lerp(p, 2.0 / 3.0), p]);
                    }
                    last = p;
                }
                Seg::Q(q, p) => {
                    if let Some(c) = cur.as_mut() {
                        c.curves.push([last.lerp(q, 2.0 / 3.0), p.lerp(q, 2.0 / 3.0), p]);
                    }
                    last = p;
                }
                Seg::C(a, b, p) => {
                    if let Some(c) = cur.as_mut() {
                        c.curves.push([a, b, p]);
                    }
                    last = p;
                }
                Seg::Z => {
                    if let Some(mut c) = cur.take() {
                        if last.dist(c.start) > 1e-4 {
                            let s = c.start;
                            c.curves.push([last.lerp(s, 1.0 / 3.0), last.lerp(s, 2.0 / 3.0), s]);
                        }
                        c.closed = true;
                        last = c.start;
                        out.push(c);
                    }
                }
            }
        }
        if let Some(c) = cur {
            out.push(c);
        }
        out
    }

    pub fn from_cubic_subpaths(subs: &[CubicSubpath]) -> Path {
        let mut segs = Vec::new();
        for s in subs {
            segs.push(Seg::M(s.start));
            for c in &s.curves {
                segs.push(Seg::C(c[0], c[1], c[2]));
            }
            if s.closed {
                segs.push(Seg::Z);
            }
        }
        Path { segs }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CubicSubpath {
    pub start: Pt,
    pub curves: Vec<[Pt; 3]>,
    pub closed: bool,
}

impl CubicSubpath {
    fn curve_len(&self, i: usize) -> f32 {
        let p0 = if i == 0 { self.start } else { self.curves[i - 1][2] };
        let c = &self.curves[i];
        p0.dist(c[0]) + c[0].dist(c[1]) + c[1].dist(c[2])
    }

    /// Split the longest curve in half (de Casteljau) until `n` curves exist.
    pub fn subdivide_to(&mut self, n: usize) {
        while self.curves.len() < n {
            if self.curves.is_empty() {
                let s = self.start;
                self.curves.push([s, s, s]);
                continue;
            }
            let (i, _) = (0..self.curves.len())
                .map(|i| (i, self.curve_len(i)))
                .fold((0, -1.0), |acc, x| if x.1 > acc.1 { x } else { acc });
            let p0 = if i == 0 { self.start } else { self.curves[i - 1][2] };
            let [c1, c2, p3] = self.curves[i];
            let m01 = p0.lerp(c1, 0.5);
            let m12 = c1.lerp(c2, 0.5);
            let m23 = c2.lerp(p3, 0.5);
            let a = m01.lerp(m12, 0.5);
            let b = m12.lerp(m23, 0.5);
            let mid = a.lerp(b, 0.5);
            self.curves[i] = [m01, a, mid];
            self.curves.insert(i + 1, [b, m23, p3]);
        }
    }

    /// Rotate the start of a closed subpath so its anchors best match `other`'s
    /// (automatic correspondence; a stand-in until shape hints exist).
    fn align_to(&mut self, other: &CubicSubpath) {
        let n = self.curves.len();
        if n < 2 || n != other.curves.len() {
            return;
        }
        let anchors_other: Vec<Pt> = other.curves.iter().map(|c| c[2]).collect();
        let best = (0..n)
            .map(|k| {
                let cost: f32 = (0..n).map(|i| {
                    let p = self.curves[(i + k) % n][2];
                    let q = anchors_other[i];
                    (p.x - q.x).powi(2) + (p.y - q.y).powi(2)
                }).sum();
                (k, cost)
            })
            .fold((0, f32::MAX), |a, b| if b.1 < a.1 { b } else { a })
            .0;
        if best != 0 {
            self.curves.rotate_left(best);
            self.start = self.curves[n - 1][2];
        }
    }

    fn centroid(&self) -> Pt {
        let n = self.curves.len() as f32 + 1.0;
        let mut s = self.start;
        for c in &self.curves {
            s.x += c[2].x;
            s.y += c[2].y;
        }
        pt(s.x / n, s.y / n)
    }

    fn collapsed(at: Pt, n: usize, closed: bool) -> CubicSubpath {
        CubicSubpath { start: at, curves: vec![[at, at, at]; n], closed }
    }
}

/// Morph two paths (shape tween). Subpath and curve counts are equalised first
/// (Animate uses shape hints for correspondence; that is a later milestone).
pub fn morph_paths(a: &Path, b: &Path, t: f32) -> Path {
    let mut sa = a.to_cubic_subpaths();
    let mut sb = b.to_cubic_subpaths();
    while sa.len() < sb.len() {
        let src = &sb[sa.len()];
        let at = sa.last().map(|s| s.centroid()).unwrap_or(src.centroid());
        sa.push(CubicSubpath::collapsed(at, src.curves.len(), src.closed));
    }
    while sb.len() < sa.len() {
        let src = &sa[sb.len()];
        let at = sb.last().map(|s| s.centroid()).unwrap_or(src.centroid());
        sb.push(CubicSubpath::collapsed(at, src.curves.len(), src.closed));
    }
    let mut out = Vec::with_capacity(sa.len());
    for (x, y) in sa.iter_mut().zip(sb.iter_mut()) {
        let n = x.curves.len().max(y.curves.len());
        x.subdivide_to(n);
        y.subdivide_to(n);
        if x.closed && y.closed {
            y.align_to(x);
        }
        out.push(CubicSubpath {
            start: x.start.lerp(y.start, t),
            curves: x
                .curves
                .iter()
                .zip(&y.curves)
                .map(|(p, q)| [p[0].lerp(q[0], t), p[1].lerp(q[1], t), p[2].lerp(q[2], t)])
                .collect(),
            closed: x.closed || y.closed,
        });
    }
    Path::from_cubic_subpaths(&out)
}

fn cubic_at(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f32) -> Pt {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    pt(
        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
    )
}

fn point_in_poly(poly: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_to_seg(p: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-9 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.dist(pt(a.x + dx * t, a.y + dy * t))
}

/// Ramer–Douglas–Peucker simplification.
pub fn simplify(pts: &[Pt], tol: f32) -> Vec<Pt> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let (mut idx, mut max) = (0, 0.0);
    for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = dist_to_seg(*p, first, last);
        if d > max {
            idx = i;
            max = d;
        }
    }
    if max > tol {
        let mut l = simplify(&pts[..=idx], tol);
        let r = simplify(&pts[idx..], tol);
        l.pop();
        l.extend(r);
        l
    } else {
        vec![first, last]
    }
}
