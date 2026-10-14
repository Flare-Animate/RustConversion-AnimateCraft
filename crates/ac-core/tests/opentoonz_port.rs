//! Features modelled on OpenToonz subsystems: Trace Bitmap, XDTS export.

use ac_core::commands::execute;
use ac_core::*;
use serde_json::json;

#[test]
fn trace_bitmap_replaces_the_bitmap_with_fills_in_one_step() {
    let mut ed = Editor::new(Document::default());
    // 30×20: left half red, right half blue
    let rgba: Vec<u8> = (0..20).flat_map(|_| (0..30).flat_map(|x| if x < 15 { [255, 0, 0, 255] } else { [0, 0, 255, 255] })).collect();
    let id = ed.add_bitmap(bitmap::BitmapAsset::from_rgba(0, "Flag", 30, 20, &rgba).unwrap());
    ed.place_bitmap(id, pt(100.0, 50.0)).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let n = execute(&mut ed, "bitmap.trace", &json!({"color_threshold": 50, "curve_fit": "normal", "corners": "many"})).unwrap();
    assert_eq!(n, json!(2));
    let els = &ed.timeline().layers[0].keyframes[&0].elements;
    assert_eq!(els.len(), 1);
    let ElementKind::Canvas(c) = &els[0].kind else { panic!("a merge shape") };
    assert_eq!(c.fills.len(), 2);
    let (lo, hi) = c.bounds().unwrap();
    assert!((lo.x - 100.0).abs() < 0.6 && (hi.x - 130.0).abs() < 0.6 && (lo.y - 50.0).abs() < 0.6, "placed where the bitmap was: {lo:?} {hi:?}");
    ed.undo();
    assert!(matches!(ed.timeline().layers[0].keyframes[&0].elements[0].kind, ElementKind::Bitmap(_)));
    assert!(execute(&mut ed, "bitmap.trace", &json!({"curve_fit": "wobbly"})).is_err());
}

#[test]
fn xdts_export_is_readable_json_after_the_identifier() {
    let d = demo::demo_document();
    let text = xdts::export(&d, &d.scene);
    let (head, body) = text.split_once('\n').unwrap();
    assert_eq!(head, xdts::IDENTIFIER);
    let v: serde_json::Value = serde_json::from_str(body).unwrap();
    let names = v["timeTables"][0]["timeTableHeaders"][0]["names"].as_array().unwrap();
    assert!(!names.is_empty());
    assert_eq!(v["timeTables"][0]["fields"][0]["tracks"].as_array().unwrap().len(), names.len());
}
