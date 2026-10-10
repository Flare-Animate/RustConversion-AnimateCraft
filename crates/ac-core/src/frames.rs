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
    if !layer.keyframes.contains_key(&0) {
        layer.keyframes.insert(0, Keyframe::blank());
    }
}

/// Pin the content that frame `f` currently shows into a keyframe at `f`, so
/// edits before `f` can't change it. No-op past the layer end.
fn pin(layer: &mut Layer, f: u32) {
    if f < layer.end && !layer.keyframes.contains_key(&f) {
        let (els, tween) = content_at(layer, f);
        let mut kf = Keyframe::with(els);
        kf.tween = tween;
        layer.keyframes.insert(f, kf);
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
        let kf = if blank {
            Keyframe::blank()
        } else {
            let (els, tween) = content_at(layer, f);
            let mut kf = Keyframe::with(els);
            kf.tween = tween;
            kf
        };
        layer.keyframes.insert(f, kf);
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
    let (els, tween) = content_at(layer, f0);
    let mut first = layer.keyframes.get(&f0).cloned().unwrap_or_else(|| Keyframe::with(els));
    if !layer.keyframes.contains_key(&f0) {
        first.tween = tween;
    }
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
        let (els, tween) = content_at(layer, f0);
        let mut kf = Keyframe::with(els);
        kf.tween = tween;
        vec![(0, kf)]
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
