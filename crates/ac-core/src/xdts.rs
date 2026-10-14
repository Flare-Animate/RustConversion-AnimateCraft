//! XDTS timing-sheet export ("exchangeDigitalTimeSheet", read by Clip Studio
//! Paint, OpenToonz, TVPaint and other exposure-sheet tools).
//!
//! Format as written by OpenToonz's `xdtsio.cpp` (BSD-3-Clause, © DWANGO and
//! OpenToonz contributors; see THIRD-PARTY-NOTICES.md): an identifier line,
//! then JSON with one time table whose cell field has a track per layer.
//!
//! Mapping: each normal layer (bottom → top = columns left → right) is a track
//! named after the layer; each keyframe with content is a drawing numbered
//! 1, 2, 3… in time order; blank keyframes and frames past the layer's end are
//! empty cells. Tweened frames show their keyframe's drawing (key timing).

use crate::model::*;
use serde_json::{json, Value};

pub const IDENTIFIER: &str = "exchangeDigitalTimeSheet Save Data";
const NULL_CELL: &str = "SYMBOL_NULL_CELL";

/// The XDTS text for the current scene `tl` of `doc`.
pub fn export(doc: &Document, tl: &Timeline) -> String {
    let duration = tl.frame_count();
    let layers: Vec<&Layer> = tl.layers.iter().rev().filter(|l| l.kind == LayerKind::Normal).collect();
    let names: Vec<String> = layers.iter().map(|l| l.name.clone()).collect();
    let tracks: Vec<Value> = layers.iter().enumerate().map(|(n, l)| json!({ "trackNo": n, "frames": track_frames(l, duration) })).collect();
    let data = json!({
        "header": { "cut": "1", "scene": "1" },
        "timeTables": [{
            "duration": duration,
            "name": if tl.name.is_empty() { doc_name(doc) } else { tl.name.clone() },
            "timeTableHeaders": [{ "fieldId": 0, "names": names }],
            "fields": [{ "fieldId": 0, "tracks": tracks }]
        }],
        "version": 5
    });
    format!("{IDENTIFIER}\n{}\n", serde_json::to_string_pretty(&data).unwrap_or_default())
}

fn doc_name(_doc: &Document) -> String {
    "Scene 1".into()
}

/// Cell changes of one layer: (frame, value) where value is a drawing number
/// or the null cell.
fn track_frames(l: &Layer, duration: u32) -> Vec<Value> {
    let mut out = vec![];
    let mut number = 0u32;
    let mut last: Option<String> = None;
    let mut push = |f: u32, v: String, last: &mut Option<String>| {
        if last.as_deref() != Some(v.as_str()) {
            out.push(json!({ "frame": f, "data": [{ "id": 0, "values": [v.clone()] }] }));
            *last = Some(v);
        }
    };
    if l.keyframes.keys().next().is_some_and(|&k| k > 0) {
        push(0, NULL_CELL.into(), &mut last);
    }
    for (&start, kf) in &l.keyframes {
        if start >= l.end.min(duration) {
            break;
        }
        let v = if kf.elements.is_empty() {
            NULL_CELL.to_string()
        } else {
            number += 1;
            number.to_string()
        };
        push(start, v, &mut last);
    }
    if l.end < duration {
        push(l.end, NULL_CELL.into(), &mut last);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Path;

    #[test]
    fn layers_become_tracks_and_keys_become_drawings() {
        let mut d = Document::default();
        let sq = Element::shape("s", Shape::new(Path::rect(0.0, 0.0, 5.0, 5.0), Some(Color::rgb(0, 0, 0)), None));
        let mut a = Layer::new("A", 0);
        a.keyframes.insert(0, Keyframe::with(vec![sq.clone()]));
        a.keyframes.insert(3, Keyframe::blank());
        a.keyframes.insert(5, Keyframe::with(vec![sq.clone()]));
        a.end = 8;
        let mut b = Layer::new("B", 1);
        b.keyframes.clear();
        b.keyframes.insert(2, Keyframe::with(vec![sq]));
        b.end = 10;
        d.scene.layers = vec![b, a]; // top: B
        let text = export(&d, &d.scene);
        assert!(text.starts_with(IDENTIFIER));
        let v: Value = serde_json::from_str(text.split_once('\n').unwrap().1).unwrap();
        assert_eq!(v["version"], 5);
        let t = &v["timeTables"][0];
        assert_eq!(t["duration"], 10);
        assert_eq!(t["timeTableHeaders"][0]["names"], json!(["A", "B"]), "bottom layer first");
        let frames = |n: usize| t["fields"][0]["tracks"][n]["frames"].as_array().unwrap().iter().map(|f| (f["frame"].as_u64().unwrap(), f["data"][0]["values"][0].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert_eq!(frames(0), vec![(0, "1".into()), (3, NULL_CELL.into()), (5, "2".into()), (8, NULL_CELL.into())]);
        assert_eq!(frames(1), vec![(0, NULL_CELL.into()), (2, "1".into())]);
    }
}
