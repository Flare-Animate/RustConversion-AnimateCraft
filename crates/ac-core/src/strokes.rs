//! Artistic stroke styles (Properties ▸ Stroke ▸ Style): Ragged, Stipple
//! and Hatched. They are generated as plain geometry at evaluation time, so
//! every renderer and exporter draws them the same way. Randomness is seeded
//! by the path, so a stroke looks the same on every frame.

use crate::geom::{pt, Path, Pt, Seg};
use crate::model::{Color, Dash, Stroke};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 40) as f32) / (1u64 << 24) as f32
    }
    /// −1 … 1
    fn sym(&mut self) -> f32 {
        self.next() * 2.0 - 1.0
    }
}

fn seed(path: &Path) -> u64 {
    let mut h: u64 = 0x9E3779B97F4A7C15;
    for s in &path.segs {
        if let Seg::M(p) | Seg::L(p) = s {
            h = (h ^ (p.x.to_bits() as u64) ^ ((p.y.to_bits() as u64) << 32)).wrapping_mul(0x100000001B3);
        }
    }
    h | 1
}

/// Points every `step` along a polyline, with the unit normal there.
fn samples(pts: &[Pt], closed: bool, step: f32) -> Vec<(Pt, Pt)> {
    let mut ring = pts.to_vec();
    if closed && pts.len() > 2 {
        ring.push(pts[0]);
    }
    let mut out = vec![];
    let mut carry = 0.0;
    for w in ring.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.dist(b);
        if len < 1e-6 {
            continue;
        }
        let (dx, dy) = ((b.x - a.x) / len, (b.y - a.y) / len);
        let normal = pt(-dy, dx);
        let mut d = carry;
        while d <= len {
            out.push((a.lerp(b, d / len), normal));
            d += step;
        }
        carry = d - len;
    }
    out
}

/// Geometry for an artistic stroke: (path, fill, stroke) parts.
pub fn artistic(path: &Path, stroke: Stroke, dash: Dash) -> Vec<(Path, Option<Color>, Option<Stroke>)> {
    let w = stroke.width.max(0.5);
    let mut rng = Rng(seed(path));
    let mut out = vec![];
    for (pts, closed) in path.flatten(12) {
        if pts.len() < 2 {
            continue;
        }
        match dash {
            Dash::Ragged => {
                // a wobbly line: points pushed sideways by up to 60% of the width
                let mut s = samples(&pts, closed, (w * 1.2).max(1.5));
                if !closed {
                    // finish at the line's end
                    let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
                    let l = a.dist(b).max(1e-6);
                    s.push((b, pt(-(b.y - a.y) / l, (b.x - a.x) / l)));
                }
                let mut segs = vec![];
                for (i, (p, n)) in s.iter().enumerate() {
                    let k = rng.sym() * w * 0.6;
                    let q = pt(p.x + n.x * k, p.y + n.y * k);
                    segs.push(if i == 0 { Seg::M(q) } else { Seg::L(q) });
                }
                if closed {
                    segs.push(Seg::Z);
                }
                out.push((Path { segs }, None, Some(Stroke { width: w * 0.8, ..stroke })));
            }
            Dash::Stipple => {
                // dots scattered across the stroke's width
                let mut segs = vec![];
                // (medium density, small-to-medium dots; at least half a pixel)
                for (p, n) in samples(&pts, closed, (w * 0.3).max(0.6)) {
                    for _ in 0..2 {
                        let k = rng.sym() * w * 0.45;
                        let along = rng.sym() * w * 0.2;
                        let c = pt(p.x + n.x * k - n.y * along, p.y + n.y * k + n.x * along);
                        let r = (w * (0.1 + rng.next() * 0.12)).max(0.5);
                        segs.extend(Path::ellipse(c.x, c.y, r, r).segs);
                    }
                }
                out.push((Path { segs }, Some(stroke.color), None));
            }
            Dash::Hatched => {
                // short strokes across the line, slightly jiggled
                let mut segs = vec![];
                for (p, n) in samples(&pts, closed, (w * 0.9).max(1.5)) {
                    let tilt = rng.sym() * 0.25;
                    let (nx, ny) = (n.x - n.y * tilt, n.y + n.x * tilt);
                    let h = w * 0.5 * (0.85 + rng.next() * 0.3);
                    segs.push(Seg::M(pt(p.x - nx * h, p.y - ny * h)));
                    segs.push(Seg::L(pt(p.x + nx * h, p.y + ny * h)));
                }
                out.push((Path { segs }, None, Some(Stroke { width: (w * 0.22).max(0.4), ..stroke })));
            }
            _ => {}
        }
    }
    out
}

/// Is this one of the generated styles?
pub fn is_artistic(d: Dash) -> bool {
    matches!(d, Dash::Ragged | Dash::Stipple | Dash::Hatched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artistic_strokes_are_stable_and_stay_near_the_line() {
        let line = Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::L(pt(100.0, 0.0))] };
        let st = Stroke { color: Color::rgb(0, 0, 0), width: 6.0 };
        for d in [Dash::Ragged, Dash::Stipple, Dash::Hatched] {
            let a = artistic(&line, st, d);
            assert_eq!(a, artistic(&line, st, d), "{d:?} is deterministic");
            let (lo, hi) = a[0].0.bounds().unwrap();
            assert!(lo.y >= -4.0 && hi.y <= 4.0 && hi.x <= 103.0, "{d:?}: {lo:?} {hi:?}");
            assert!(a[0].0.segs.len() > 10, "{d:?} has detail");
        }
    }
}
