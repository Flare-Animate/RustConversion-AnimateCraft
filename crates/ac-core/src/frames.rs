//! Frame-level operations on a single layer, matching Animate's timeline
//! semantics. Pure functions over `Layer` so they are easy to test; the
//! `Editor` applies them across every layer in a frame selection.

use crate::model::*;

/// What a keyframe created at `f` should contain: the visible (possibly
/// tweened) content, or past the layer's end the last keyframe's content.
pub fn content_at(layer: &Layer, f: u32) -> (Vec<Element>, Tween) {
    if let Some((els, _)) = layer_elements_at(layer, f) {
        let tween = layer.span_at(f).map(|(_, k, _)| k.tween).unwrap_or(Tween::None);
        return (els, tween);
    }
    let last = layer.keyframes.values().next_back().map(|k| k.elements.clone()).unwrap_or_default();
    (last, Tween::None)
}

/// Keyframe starts in `a..=b` (empty when `a > b`, instead of panicking).
fn keys_in(layer: &Layer, a: u32, b: u32) -> Vec<u32> {
    if a > b {
        return vec![];
    }
    layer.keyframes.range(a..=b).map(|(k, _)| *k).collect()
}

fn ensure_first(layer: &mut Layer) {
    if layer.end == 0 {
        layer.end = 1;
    }
    layer.keyframes.entry(0).or_insert_with(Keyframe::blank);
}

/// A keyframe holding what frame `f` shows: content, the span's tween and,
/// inside a motion tween, the rest of its property keys (rebased to `f`).
pub fn keyframe_at(layer: &Layer, f: u32) -> Keyframe {
    let (els, tween) = content_at(layer, f);
    let mut kf = Keyframe::with(els);
    kf.tween = tween;
    if let Some((s, src, _)) = layer.span_at(f) {
        kf.ease_curve = src.ease_curve.clone();
        kf.orient_to_path = src.orient_to_path;
        if let (Some(m), Some(el)) = (&src.motion, kf.elements.first()) {
            let mut head = (**m).clone();
            let pose = crate::motion::Pose::of(el, m.pivot);
            kf.motion = Some(Box::new(head.split(f - s, &pose)));
        }
    }
    kf
}

/// Insert a keyframe at `f` that keeps what the frame shows, and end a
/// motion tween's keys there.
fn split_at(layer: &mut Layer, f: u32) {
    let kf = keyframe_at(layer, f);
    if let Some((s, _, _)) = layer.span_at(f) {
        if let Some(pose) = kf.elements.first().zip(kf.motion.as_ref()).map(|(el, m)| crate::motion::Pose::of(el, m.pivot)) {
            if let Some(m) = layer.keyframes.get_mut(&s).and_then(|k| k.motion.as_mut()) {
                m.split(f - s, &pose);
            }
        }
    }
    layer.keyframes.insert(f, kf);
}

/// Pin the content that frame `f` currently shows into a keyframe at `f`, so
/// edits before `f` can't change it. No-op past the layer end.
fn pin(layer: &mut Layer, f: u32) {
    if f < layer.end && !layer.keyframes.contains_key(&f) {
        split_at(layer, f);
    }
}

fn shift_after(layer: &mut Layer, after: u32, delta: i64) {
    let moved: Vec<(u32, Keyframe)> = layer.keyframes.range(after + 1..).map(|(k, v)| (*k, v.clone())).collect();
    for (k, _) in &moved {
        layer.keyframes.remove(k);
    }
    for (k, v) in moved {
        let nk = (k as i64 + delta).max(0) as u32;
        layer.keyframes.insert(nk, v);
    }
}

/// F5: insert `n` frames at `at`, lengthening the span there. Past the end,
/// extends the layer up to and including `at`.
pub fn insert_frames(layer: &mut Layer, at: u32, n: u32) {
    if at >= layer.end {
        layer.end = at + 1;
        return;
    }
    shift_after(layer, at, n as i64);
    layer.end += n;
}

/// Shift+F5: delete frames `f0..=f1`; later frames move left.
pub fn remove_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    pin(layer, f1 + 1);
    let n = f1 - f0 + 1;
    let doomed = keys_in(layer, f0, f1);
    for k in doomed {
        layer.keyframes.remove(&k);
    }
    shift_after(layer, f1, -(n as i64));
    layer.end -= n;
    ensure_first(layer);
}

/// F6 (keep content) / F7 (blank): make every frame in `f0..=f1` a keyframe.
pub fn convert_to_keyframes(layer: &mut Layer, f0: u32, f1: u32, blank: bool) {
    for f in f0..=f1 {
        if layer.keyframes.contains_key(&f) {
            continue;
        }
        if blank {
            layer.keyframes.insert(f, Keyframe::blank());
        } else if f < layer.end {
            split_at(layer, f);
        } else {
            let (els, _) = content_at(layer, f);
            layer.keyframes.insert(f, Keyframe::with(els));
        }
    }
    layer.end = layer.end.max(f1 + 1);
}

/// Shift+F6: turn keyframes in the range back into plain frames.
pub fn clear_keyframes(layer: &mut Layer, f0: u32, f1: u32) {
    let doomed = keys_in(layer, f0.max(1), f1);
    for k in doomed {
        layer.keyframes.remove(&k);
    }
}

/// Delete on frames ("Clear Frames"): the range becomes one blank keyframe;
/// frames after it keep what they showed.
pub fn clear_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    pin(layer, f1 + 1);
    let doomed = keys_in(layer, f0, f1);
    for k in doomed {
        layer.keyframes.remove(&k);
    }
    layer.keyframes.insert(f0, Keyframe::blank());
}

/// Apply a tween to every span overlapping `f0..=f1`. Animate needs an end
/// keyframe; one is created at the span's last frame when missing.
pub fn set_tween(layer: &mut Layer, f0: u32, f1: u32, tween: Tween) {
    let starts: Vec<u32> = layer
        .keyframes
        .keys()
        .copied()
        .filter(|&s| s <= f1 && layer.span_end(s) > f0)
        .collect();
    for s in starts {
        if s >= layer.end {
            continue;
        }
        let end = layer.span_end(s);
        if !matches!(tween, Tween::None) && !layer.keyframes.contains_key(&end) && end > s + 1 {
            let content = layer.keyframes[&s].elements.clone();
            layer.keyframes.insert(end - 1, Keyframe::with(content));
        }
        // The closing keyframe of the last span has nothing to tween to.
        let has_next = !keys_in(layer, s + 1, layer.end.saturating_sub(1)).is_empty();
        if let Some(k) = layer.keyframes.get_mut(&s) {
            k.tween = if has_next { tween } else { Tween::None };
            k.motion = None;
        }
    }
}

/// A copied run of frames, relative to its first frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameClip {
    pub len: u32,
    pub keyframes: Vec<(u32, Keyframe)>,
}

pub fn copy_frames(layer: &Layer, f0: u32, f1: u32) -> FrameClip {
    let mut keyframes = Vec::new();
    let first = layer.keyframes.get(&f0).cloned().unwrap_or_else(|| keyframe_at(layer, f0));
    keyframes.push((0, first));
    for k in keys_in(layer, f0 + 1, f1) {
        keyframes.push((k - f0, layer.keyframes[&k].clone()));
    }
    FrameClip { len: f1 - f0 + 1, keyframes }
}

/// Paste replaces frames `at..at+len`; frames after keep their content.
pub fn paste_frames(layer: &mut Layer, at: u32, clip: &FrameClip) {
    let end = at + clip.len;
    pin(layer, end);
    let doomed = keys_in(layer, at, end - 1);
    for k in doomed {
        layer.keyframes.remove(&k);
    }
    for (rel, kf) in &clip.keyframes {
        layer.keyframes.insert(at + rel, kf.clone());
    }
    layer.end = layer.end.max(end);
    ensure_first(layer);
}

/// Drag frames: move the keyframes in `f0..=f1` by `delta` frames, like
/// dragging them in Animate. Spans around the old position stretch to fill
/// the gap; keyframes in the destination block are replaced. If the block
/// holds no keyframe, what its first frame shows is dropped as a keyframe.
pub fn move_frames(layer: &mut Layer, f0: u32, f1: u32, delta: i64) {
    if delta == 0 || f0 >= layer.end {
        return;
    }
    let delta = delta.max(-(f0 as i64));
    let f1 = f1.min(layer.end - 1);
    let keys = keys_in(layer, f0, f1);
    let block: Vec<(u32, Keyframe)> = if keys.is_empty() {
        vec![(0, keyframe_at(layer, f0))]
    } else {
        let b = keys.iter().map(|k| (k - f0, layer.keyframes[k].clone())).collect();
        for k in &keys {
            layer.keyframes.remove(k);
        }
        b
    };
    let n0 = (f0 as i64 + delta) as u32;
    let n1 = n0 + (f1 - f0);
    for k in keys_in(layer, n0, n1) {
        layer.keyframes.remove(&k);
    }
    for (rel, kf) in block {
        layer.keyframes.insert(n0 + rel, kf);
    }
    layer.end = layer.end.max(n1 + 1);
    ensure_first(layer);
}

/// Convert to Frame-by-Frame Animation: every frame of the tweens in
/// `f0..=f1` becomes a keyframe holding what it showed.
pub fn convert_to_frame_by_frame(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    // cut the tweens at the range ends first so the rest keeps its timing
    pin(layer, f0);
    pin(layer, f1 + 1);
    let starts: Vec<u32> = layer.keyframes.iter().filter(|(&s, k)| s <= f1 && layer.span_end(s) > f0 && !matches!(k.tween, Tween::None)).map(|(s, _)| *s).collect();
    let mut out = vec![];
    for s in starts {
        let e = layer.span_end(s);
        for f in s.max(f0)..e.min(f1 + 1) {
            if let Some((els, _)) = layer_elements_at(layer, f) {
                let mut kf = Keyframe::with(els);
                if f == s {
                    let src = &layer.keyframes[&s];
                    kf.label = src.label.clone();
                    kf.label_kind = src.label_kind;
                    kf.script = src.script.clone();
                }
                out.push((f, kf));
            }
        }
    }
    for (f, kf) in out {
        layer.keyframes.insert(f, kf);
    }
}

/// Reverse Frames: frame f in `f0..=f1` shows what frame `f0 + f1 − f` showed.
/// Tweens keep working (backwards, with mirrored eases).
pub fn reverse_frames(layer: &mut Layer, f0: u32, f1: u32) {
    if f0 >= layer.end {
        return;
    }
    let f1 = f1.min(layer.end - 1);
    if f1 <= f0 {
        return;
    }
    pin(layer, f1 + 1);
    if !layer.keyframes.contains_key(&f0) {
        if layer.span_at(f0).is_some() {
            split_at(layer, f0);
        } else {
            return;
        }
    }
    let keys = keys_in(layer, f0, f1);
    let mirror = |f: u32| f0 + f1 - f;
    let mut placed: std::collections::BTreeMap<u32, Keyframe> = Default::default();
    let mut tween_starts: Vec<(u32, Keyframe)> = vec![];
    for &k in &keys {
        let e = layer.span_end(k).min(f1 + 1);
        let kf = layer.keyframes[&k].clone();
        let (r0, r1) = (mirror(e - 1), mirror(k));
        let target = layer.keyframes.get(&layer.span_end(k)).filter(|_| layer.span_end(k) <= f1 + 1);
        match kf.tween {
            Tween::Motion { .. } => {
                let mut kf2 = kf.clone();
                if let Some(m) = kf2.motion.as_mut() {
                    m.reverse(e - k);
                }
                kf2.ease_curve = kf.ease_curve.as_ref().map(|c| c.reversed());
                kf2.tween = Tween::Motion { ease: -kf.tween.ease() };
                placed.insert(r0, kf2);
            }
            Tween::Classic { .. } | Tween::Shape { .. } if target.is_some() => {
                let mut end = kf.clone();
                end.tween = Tween::None;
                end.ease_curve = None;
                placed.entry(r1).or_insert(end);
                let mut start = if r0 > f0 {
                    target.unwrap().clone()
                } else {
                    let (els, _) = content_at(layer, e - 1);
                    Keyframe::with(els)
                };
                start.label = None;
                start.script = None;
                start.tween = match kf.tween {
                    Tween::Classic { ease, rotate_turns } => Tween::Classic { ease: -ease, rotate_turns: -rotate_turns },
                    Tween::Shape { ease } => Tween::Shape { ease: -ease },
                    t => t,
                };
                start.ease_curve = kf.ease_curve.as_ref().map(|c| c.reversed());
                start.orient_to_path = kf.orient_to_path;
                start.motion = None;
                tween_starts.push((if r0 > f0 { r0 - 1 } else { r0 }, start));
            }
            _ => {
                let mut st = kf.clone();
                st.tween = Tween::None;
                placed.entry(r0).or_insert(st);
            }
        }
    }
    for (f, kf) in tween_starts {
        placed.insert(f, kf);
    }
    for k in keys {
        layer.keyframes.remove(&k);
    }
    layer.keyframes.extend(placed);
}

/// Drag the end of a layer's last span: lengthen/shorten the layer. Motion
/// tween keys stretch with the span (as in Animate).
/// Drag the end of the span starting at `start` to `new_end` (exclusive):
/// later keyframes move with it (Animate's Ctrl-drag of a span's last
/// frame); a motion tween's property keys stretch to the new length.
pub fn set_span_end(layer: &mut Layer, start: u32, new_end: u32) {
    if !layer.keyframes.contains_key(&start) {
        return;
    }
    let next = layer.keyframes.range(start + 1..).next().map(|(&k, _)| k);
    let Some(old_end) = next else { return set_last_span_end(layer, new_end) };
    let new_end = new_end.max(start + 1);
    let delta = new_end as i64 - old_end as i64;
    if delta == 0 {
        return;
    }
    if let Some(m) = layer.keyframes.get_mut(&start).and_then(|k| k.motion.as_mut()) {
        m.fit(old_end - start, new_end - start, true);
    }
    let later: Vec<u32> = layer.keyframes.range(old_end..).map(|(&k, _)| k).collect();
    let moved: Vec<(u32, Keyframe)> = later.iter().map(|k| (*k, layer.keyframes.remove(k).unwrap())).collect();
    for (k, kf) in moved {
        layer.keyframes.insert((k as i64 + delta) as u32, kf);
    }
    layer.end = (layer.end as i64 + delta).max(1) as u32;
}

pub fn set_last_span_end(layer: &mut Layer, new_end: u32) {
    let Some((&start, _)) = layer.keyframes.iter().next_back() else { return };
    let new_end = new_end.max(start + 1);
    let old_len = layer.end.saturating_sub(start);
    let new_len = new_end - start;
    if let Some(m) = layer.keyframes.get_mut(&start).and_then(|k| k.motion.as_mut()) {
        m.fit(old_len, new_len, true);
    }
    layer.end = new_end;
}
