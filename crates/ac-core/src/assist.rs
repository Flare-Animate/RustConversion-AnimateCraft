//! Drawing assistants and symmetry, after OpenToonz's `tnztools/assistants`
//! (guidelines: `assistantline.cpp`, `assistantvanishingpoint.cpp`,
//! `assistantellipse.cpp`; replicators: `replicatormirror.cpp`,
//! `replicatorstar.cpp`, `replicatorgrid.cpp`; guideline choice from
//! `assistant.cpp`; BSD-3-Clause, © DWANGO and OpenToonz contributors; see
//! THIRD-PARTY-NOTICES.md).
//!
//! * **Guides** (Line, Vanishing point, Ellipse) pull freehand strokes onto a
//!   line or curve. The first ~40 screen pixels of a stroke choose the guide
//!   that fits best (OpenToonz's log-normal weighting); every point is then
//!   pulled onto it by the guide's magnetism.
//! * **Replicators** (Mirror, Star, Grid) repeat each stroke: symmetry and
//!   kaleidoscopes while you draw.

use crate::geom::{pt, Matrix, Pt};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantKind {
    /// A straight guide through `a` and `b`; `parallel`: any line parallel to it.
    Line {
        a: Pt,
        b: Pt,
        #[serde(default)]
        parallel: bool,
    },
    /// Lines through a vanishing point (combine two or three for perspective).
    VanishingPoint { c: Pt },
    /// An ellipse: centre `c`, first axis to `a`, second axis length from `b`;
    /// `concentric`: any ellipse of the same shape around `c`.
    Ellipse {
        c: Pt,
        a: Pt,
        b: Pt,
        #[serde(default)]
        concentric: bool,
    },
    /// Mirror symmetry across the line through `a` and `b`.
    Mirror { a: Pt, b: Pt },
    /// Radial symmetry: `count` copies around `c` (`a` sets the start angle),
    /// optionally mirrored (kaleidoscope).
    Star {
        c: Pt,
        a: Pt,
        count: u32,
        #[serde(default)]
        mirror: bool,
    },
    /// Copies on a grid with steps `a − c` and `b − c`: `count_a` along the
    /// first (forward) and `count_a_inv` backward, likewise for b.
    Grid {
        c: Pt,
        a: Pt,
        b: Pt,
        count_a: u32,
        #[serde(default)]
        count_a_inv: u32,
        count_b: u32,
        #[serde(default)]
        count_b_inv: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assistant {
    pub kind: AssistantKind,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Guides: how strongly strokes are pulled on (0…1).
    #[serde(default = "one")]
    pub magnetism: f32,
}

fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

impl Assistant {
    pub fn new(kind: AssistantKind) -> Assistant {
        Assistant { kind, enabled: true, magnetism: 1.0 }
    }
    pub fn name(&self) -> &'static str {
        self.kind.name()
    }
    pub fn is_guide(&self) -> bool {
        matches!(self.kind, AssistantKind::Line { .. } | AssistantKind::VanishingPoint { .. } | AssistantKind::Ellipse { .. })
    }
    /// Editable points (handles).
    pub fn points(&self) -> Vec<Pt> {
        match &self.kind {
            AssistantKind::Line { a, b, .. } | AssistantKind::Mirror { a, b } => vec![*a, *b],
            AssistantKind::VanishingPoint { c } => vec![*c],
            AssistantKind::Ellipse { c, a, b, .. } | AssistantKind::Grid { c, a, b, .. } => vec![*c, *a, *b],
            AssistantKind::Star { c, a, .. } => vec![*c, *a],
        }
    }
    /// Move handle `i` to `p`. Moving the first handle (the centre / start)
    /// carries the others along, as in OpenToonz.
    pub fn set_point(&mut self, i: usize, p: Pt) {
        let old = self.points();
        let Some(&o) = old.get(i) else { return };
        let d = pt(p.x - o.x, p.y - o.y);
        let mv = |q: &mut Pt| *q = pt(q.x + d.x, q.y + d.y);
        match &mut self.kind {
            AssistantKind::Line { a, b, .. } | AssistantKind::Mirror { a, b } => match i {
                0 => {
                    mv(a);
                    if matches!(self.kind, AssistantKind::Mirror { .. }) {
                        if let AssistantKind::Mirror { b, .. } = &mut self.kind {
                            mv(b);
                        }
                    }
                }
                _ => *b = p,
            },
            AssistantKind::VanishingPoint { c } => *c = p,
            AssistantKind::Ellipse { c, a, b, .. } | AssistantKind::Grid { c, a, b, .. } => match i {
                0 => {
                    mv(c);
                    mv(a);
                    mv(b);
                }
                1 => *a = p,
                _ => *b = p,
            },
            AssistantKind::Star { c, a, .. } => match i {
                0 => {
                    mv(c);
                    mv(a);
                }
                _ => *a = p,
            },
        }
    }
    /// Move every point by `d`.
    pub fn translate(&mut self, d: Pt) {
        let n = self.points().len();
        for i in 0..n {
            let q = self.points()[i];
            let q2 = pt(q.x + d.x, q.y + d.y);
            match &mut self.kind {
                AssistantKind::Line { a, b, .. } | AssistantKind::Mirror { a, b } => *[a, b][i] = q2,
                AssistantKind::VanishingPoint { c } => *c = q2,
                AssistantKind::Ellipse { c, a, b, .. } | AssistantKind::Grid { c, a, b, .. } => *[c, a, b][i] = q2,
                AssistantKind::Star { c, a, .. } => *[c, a][i] = q2,
            }
        }
    }
}

impl AssistantKind {
    pub const NAMES: [&'static str; 6] = ["Line", "Vanishing point", "Ellipse", "Mirror", "Star", "Grid"];
    pub fn name(&self) -> &'static str {
        match self {
            AssistantKind::Line { .. } => "Line",
            AssistantKind::VanishingPoint { .. } => "Vanishing point",
            AssistantKind::Ellipse { .. } => "Ellipse",
            AssistantKind::Mirror { .. } => "Mirror",
            AssistantKind::Star { .. } => "Star",
            AssistantKind::Grid { .. } => "Grid",
        }
    }
    /// A new assistant of a type (by name or script id) centred at `c` with
    /// handles `r` units out.
    pub fn create(name: &str, c: Pt, r: f32) -> Option<AssistantKind> {
        let n = name.to_ascii_lowercase().replace(['_', '-'], " ");
        Some(match n.as_str() {
            "line" => AssistantKind::Line { a: pt(c.x - r, c.y), b: pt(c.x + r, c.y), parallel: false },
            "vanishing point" | "vanishing" | "vp" => AssistantKind::VanishingPoint { c },
            "ellipse" => AssistantKind::Ellipse { c, a: pt(c.x + r, c.y), b: pt(c.x, c.y - r * 0.6), concentric: true },
            "mirror" => AssistantKind::Mirror { a: c, b: pt(c.x, c.y - r) },
            "star" => AssistantKind::Star { c, a: pt(c.x, c.y - r), count: 6, mirror: false },
            "grid" => AssistantKind::Grid { c, a: pt(c.x + r, c.y), b: pt(c.x, c.y + r), count_a: 3, count_a_inv: 0, count_b: 3, count_b_inv: 0 },
            _ => return None,
        })
    }
}

// ------------------------------------------------------------ guidelines

/// One guide a stroke can snap to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// Infinite line through two points.
    Line(Pt, Pt),
    /// Unit circle mapped by the matrix (and its inverse).
    Ellipse(Matrix, Matrix),
}

impl Guide {
    pub fn nearest(&self, p: Pt) -> Pt {
        match *self {
            Guide::Line(a, b) => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let l2 = dx * dx + dy * dy;
                if l2 < 1e-12 {
                    return a;
                }
                let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2;
                pt(a.x + dx * t, a.y + dy * t)
            }
            Guide::Ellipse(m, inv) => {
                let q = inv.apply(p);
                let l = (q.x * q.x + q.y * q.y).sqrt();
                if l < 1e-9 {
                    return m.apply(pt(1.0, 0.0));
                }
                m.apply(pt(q.x / l, q.y / l))
            }
        }
    }
}

/// Unit circle → the ellipse with centre `c`, first axis to `a`, second axis
/// as long as `b`'s distance from the first axis.
pub fn ellipse_matrix(c: Pt, a: Pt, b: Pt) -> Option<Matrix> {
    let x = pt(a.x - c.x, a.y - c.y);
    let lx = (x.x * x.x + x.y * x.y).sqrt();
    if lx < 1e-6 {
        return None;
    }
    let n = pt(-x.y / lx, x.x / lx);
    let ry = ((b.x - c.x) * n.x + (b.y - c.y) * n.y).abs().max(1e-3);
    Some(Matrix { a: x.x, b: x.y, c: n.x * ry, d: n.y * ry, tx: c.x, ty: c.y })
}

/// The guides an assistant offers for a stroke starting at `p0`, with its magnetism.
pub fn guides(list: &[Assistant], p0: Pt) -> Vec<(Guide, f32)> {
    let mut out = vec![];
    for a in list.iter().filter(|a| a.enabled) {
        let g = match &a.kind {
            AssistantKind::Line { a, b, parallel } => {
                if *parallel {
                    Some(Guide::Line(p0, pt(p0.x + b.x - a.x, p0.y + b.y - a.y)))
                } else {
                    Some(Guide::Line(*a, *b))
                }
            }
            AssistantKind::VanishingPoint { c } => (c.dist(p0) > 1e-3).then_some(Guide::Line(*c, p0)),
            AssistantKind::Ellipse { c, a: ax, b, concentric } => ellipse_matrix(*c, *ax, *b).and_then(|m| {
                let m = if *concentric {
                    let inv = m.invert()?;
                    let q = inv.apply(p0);
                    let s = (q.x * q.x + q.y * q.y).sqrt().max(1e-3);
                    Matrix { a: m.a * s, b: m.b * s, c: m.c * s, d: m.d * s, ..m }
                } else {
                    m
                };
                Some(Guide::Ellipse(m, m.invert()?))
            }),
            _ => None,
        };
        if let Some(g) = g {
            out.push((g, a.magnetism.clamp(0.0, 1.0)));
        }
    }
    out
}

/// OpenToonz's `TGuideline::calcTrackWeight`: mean deviation of the stroke's
/// first ~40 screen pixels from the guide, weighted log-normally around 20 px.
/// `px` = stage units per screen pixel.
pub fn track_weight(g: &Guide, pts: &[Pt], px: f32) -> f32 {
    const SNAP: f32 = 20.0;
    const MAX: f32 = 2.0 * SNAP;
    if pts.len() < 2 {
        return f32::INFINITY;
    }
    let (mut sum_w, mut sum_len, mut sum_dev) = (0.0f32, 0.0f32, 0.0f32);
    for i in 1..pts.len() {
        let len = pts[i].dist(pts[i - 1]) / px;
        sum_len += len;
        let mid_len = sum_len - 0.5 * len;
        if mid_len > 1e-6 {
            let w = len * (-0.5 * (mid_len / SNAP).ln().powi(2)).exp() / mid_len;
            sum_w += w;
            let mid = pts[i - 1].lerp(pts[i], 0.5);
            sum_dev += w * mid.dist(g.nearest(mid)) / px;
        }
        if sum_len >= MAX {
            break;
        }
    }
    if sum_w > 1e-9 {
        sum_dev / sum_w
    } else {
        f32::INFINITY
    }
}

/// The guide a stroke snaps to (best fitting start), if any.
pub fn best_guide(list: &[Assistant], pts: &[Pt], px: f32) -> Option<(Guide, f32)> {
    let p0 = *pts.first()?;
    guides(list, p0).into_iter().map(|(g, m)| (track_weight(&g, pts, px), g, m)).filter(|(w, _, _)| w.is_finite()).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap()).map(|(_, g, m)| (g, m))
}

/// Pull a stroke onto its best guide.
pub fn snap(list: &[Assistant], pts: &[Pt], px: f32) -> Vec<Pt> {
    match best_guide(list, pts, px) {
        Some((g, m)) => pts.iter().map(|p| p.lerp(g.nearest(*p), m)).collect(),
        None => pts.to_vec(),
    }
}

// ------------------------------------------------------------ replicators

fn reflect(a: Pt, b: Pt) -> Option<Matrix> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-9 {
        return None;
    }
    let (c2, s2) = ((dx * dx - dy * dy) / l2, 2.0 * dx * dy / l2);
    // reflection about the line through a: p' = a + R (p − a)
    let r = Matrix { a: c2, b: s2, c: s2, d: -c2, tx: 0.0, ty: 0.0 };
    Some(Matrix::translate(a.x, a.y).concat(&r).concat(&Matrix::translate(-a.x, -a.y)))
}

/// The extra copies every stroke gets (identity not included), as maps of
/// stage points. Replicators combine: a mirror plus a star gives the star's
/// copies of both the stroke and its mirror image.
pub fn replicas(list: &[Assistant]) -> Vec<Matrix> {
    let mut all = vec![Matrix::IDENTITY];
    for a in list.iter().filter(|a| a.enabled) {
        let mine: Vec<Matrix> = match &a.kind {
            AssistantKind::Mirror { a, b } => reflect(*a, *b).into_iter().collect(),
            AssistantKind::Star { c, a, count, mirror } => {
                let n = (*count).clamp(1, 64);
                let mut v = vec![];
                for i in 0..n {
                    let rot = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(std::f32::consts::TAU * i as f32 / n as f32)).concat(&Matrix::translate(-c.x, -c.y));
                    if i > 0 {
                        v.push(rot);
                    }
                    if *mirror {
                        if let Some(r) = reflect(*c, *a) {
                            v.push(rot.concat(&r));
                        }
                    }
                }
                v
            }
            AssistantKind::Grid { c, a, b, count_a, count_a_inv, count_b, count_b_inv } => {
                let (da, db) = (pt(a.x - c.x, a.y - c.y), pt(b.x - c.x, b.y - c.y));
                let mut v = vec![];
                for ib in -(*count_b_inv as i32)..(*count_b).max(1) as i32 {
                    for ia in -(*count_a_inv as i32)..(*count_a).max(1) as i32 {
                        if ia != 0 || ib != 0 {
                            v.push(Matrix::translate(da.x * ia as f32 + db.x * ib as f32, da.y * ia as f32 + db.y * ib as f32));
                        }
                    }
                }
                v.truncate(400);
                v
            }
            _ => continue,
        };
        let mut next = all.clone();
        for m in &mine {
            for base in &all {
                next.push(m.concat(base));
            }
        }
        all = next;
        all.truncate(1000);
    }
    all.into_iter().skip(1).collect()
}

/// A stroke as drawn with the assistants: snapped, then replicated (the
/// first polyline is the stroke itself).
pub fn apply(list: &[Assistant], pts: &[Pt], px: f32) -> Vec<Vec<Pt>> {
    let s = snap(list, pts, px);
    let mut out = vec![s.clone()];
    for m in replicas(list) {
        out.push(s.iter().map(|p| m.apply(*p)).collect());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wobbly(from: Pt, to: Pt) -> Vec<Pt> {
        (0..=40).map(|i| {
            let t = i as f32 / 40.0;
            let w = if i % 2 == 0 { 1.5 } else { -1.5 };
            let (dx, dy) = (to.x - from.x, to.y - from.y);
            let l = (dx * dx + dy * dy).sqrt();
            pt(from.x + dx * t - dy / l * w, from.y + dy * t + dx / l * w)
        }).collect()
    }

    #[test]
    fn strokes_snap_to_the_best_fitting_guide() {
        let list = vec![
            Assistant::new(AssistantKind::Line { a: pt(0.0, 0.0), b: pt(100.0, 0.0), parallel: true }),
            Assistant::new(AssistantKind::VanishingPoint { c: pt(500.0, 500.0) }),
        ];
        // a roughly horizontal stroke snaps to a horizontal line through its start
        let s = snap(&list, &wobbly(pt(10.0, 50.0), pt(200.0, 50.0)), 1.0);
        assert!(s.iter().all(|p| (p.y - s[0].y).abs() < 1e-3), "flat");
        // a stroke heading for the vanishing point snaps onto the ray
        let s = snap(&list, &wobbly(pt(100.0, 100.0), pt(300.0, 300.0)), 1.0);
        assert!(s.iter().all(|p| (p.x - p.y).abs() < 4.0), "on the diagonal ray: {:?}", s[20]);
        // magnetism 0.5 pulls halfway
        let half = vec![Assistant { magnetism: 0.5, ..list[0].clone() }];
        let raw = wobbly(pt(10.0, 50.0), pt(200.0, 50.0));
        let s = snap(&half, &raw, 1.0);
        assert!((s[1].y - (raw[1].y + raw[0].y) * 0.5).abs() < 0.8);
        // no guides → unchanged
        assert_eq!(snap(&[], &raw, 1.0), raw);
    }

    #[test]
    fn concentric_ellipse_guide_passes_through_the_start() {
        let list = vec![Assistant::new(AssistantKind::Ellipse { c: pt(0.0, 0.0), a: pt(100.0, 0.0), b: pt(0.0, 50.0), concentric: true })];
        let raw: Vec<Pt> = (0..30).map(|i| {
            let a = i as f32 * 0.05;
            pt(60.0 * a.cos() + 0.7, 30.0 * a.sin() - 0.5)
        }).collect();
        let s = snap(&list, &raw, 1.0);
        for p in &s {
            let e = (p.x / 60.0).powi(2) + (p.y / 30.0).powi(2);
            assert!((e - 1.0).abs() < 0.05, "on the 60×30 ellipse: {p:?}");
        }
    }

    #[test]
    fn replicators_mirror_star_and_grid() {
        let m = vec![Assistant::new(AssistantKind::Mirror { a: pt(100.0, 0.0), b: pt(100.0, 10.0) })];
        let r = replicas(&m);
        assert_eq!(r.len(), 1);
        assert!(r[0].apply(pt(90.0, 5.0)).dist(pt(110.0, 5.0)) < 1e-4);
        let star = vec![Assistant::new(AssistantKind::Star { c: pt(0.0, 0.0), a: pt(0.0, -10.0), count: 4, mirror: false })];
        let r = replicas(&star);
        assert_eq!(r.len(), 3);
        assert!(r.iter().any(|m| m.apply(pt(10.0, 0.0)).dist(pt(-10.0, 0.0)) < 1e-4));
        let kaleido = vec![Assistant::new(AssistantKind::Star { c: pt(0.0, 0.0), a: pt(0.0, -10.0), count: 4, mirror: true })];
        assert_eq!(replicas(&kaleido).len(), 7);
        let grid = vec![Assistant::new(AssistantKind::Grid { c: pt(0.0, 0.0), a: pt(50.0, 0.0), b: pt(0.0, 50.0), count_a: 3, count_a_inv: 1, count_b: 2, count_b_inv: 0 })];
        assert_eq!(replicas(&grid).len(), 4 * 2 - 1);
        // mirror + star combine
        let both = [m.clone(), star.clone()].concat();
        assert_eq!(replicas(&both).len(), 2 * 4 - 1);
        assert_eq!(apply(&both, &[pt(1.0, 1.0), pt(2.0, 2.0)], 1.0).len(), 8);
        // disabled assistants do nothing
        let off = vec![Assistant { enabled: false, ..m[0].clone() }];
        assert!(replicas(&off).is_empty());
    }

    #[test]
    fn handles_move_and_serialize() {
        let mut a = Assistant::new(AssistantKind::create("star", pt(0.0, 0.0), 50.0).unwrap());
        a.set_point(0, pt(10.0, 10.0));
        assert_eq!(a.points(), vec![pt(10.0, 10.0), pt(10.0, -40.0)]);
        let j = serde_json::to_string(&a).unwrap();
        assert!(j.contains("\"type\":\"star\""));
        assert_eq!(serde_json::from_str::<Assistant>(&j).unwrap(), a);
        assert!(AssistantKind::create("vanishing_point", pt(0.0, 0.0), 1.0).is_some());
    }
}
