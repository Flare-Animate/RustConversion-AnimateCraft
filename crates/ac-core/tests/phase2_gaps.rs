//! Phase 2 gap fixes: graphic loop options, frame label types, classic tween
//! Sync and per-property eases.

use ac_core::motion::{Ease, EaseDir, EaseFamily, PropEases};
use ac_core::*;

fn inst(sym: SymbolId, first: u32, x: f32) -> Element {
    let mut e = Element::instance("", sym, Matrix::translate(x, 0.0));
    if let ElementKind::Instance(i) = &mut e.kind {
        i.first_frame = first;
    }
    e
}

#[test]
fn reverse_loop_modes() {
    let i = |m: LoopMode| Instance { symbol: 1, loop_mode: m, first_frame: 3 };
    let frames = |m: LoopMode| (0..6).map(|l| i(m).symbol_frame(SymbolKind::Graphic, l, l, 5)).collect::<Vec<_>>();
    assert_eq!(frames(LoopMode::PlayReverseOnce), [3, 2, 1, 0, 0, 0]);
    assert_eq!(frames(LoopMode::LoopReverse), [3, 2, 1, 0, 4, 3]);
    assert_eq!(LoopMode::from_id("Loop Reverse"), Some(LoopMode::LoopReverse));
}

#[test]
fn comment_labels_are_not_targets() {
    let mut k = Keyframe::blank();
    k.label = Some("note".into());
    assert_eq!(k.target_label(), Some("note"));
    k.label_kind = LabelKind::Comment;
    assert_eq!(k.target_label(), None);
    k.label_kind = LabelKind::Anchor;
    assert_eq!(k.target_label(), Some("note"));
}

#[test]
fn sync_continues_graphic_frames_across_keyframes() {
    let mut l = Layer::new("a", 0);
    let mut k0 = Keyframe::with(vec![inst(7, 2, 0.0)]);
    k0.tween = Tween::Classic { ease: 0.0, rotate_turns: 0 };
    l.keyframes.insert(0, k0);
    l.keyframes.insert(10, Keyframe::with(vec![inst(7, 0, 100.0)]));
    l.end = 20;
    let ff = |l: &Layer, f: u32| match &layer_elements_at(l, f).unwrap().0[0].kind {
        ElementKind::Instance(i) => i.first_frame,
        _ => unreachable!(),
    };
    assert_eq!(ff(&l, 12), 0, "without Sync the second keyframe restarts");
    l.keyframes.get_mut(&0).unwrap().sync = true;
    assert_eq!(ff(&l, 12), 12, "with Sync it continues from 2 + 10");
}

#[test]
fn per_property_eases() {
    let mut l = Layer::new("a", 0);
    let a = Element::instance("", 1, Matrix::IDENTITY);
    let b = Element::instance("", 1, Matrix::translate(100.0, 0.0).concat(&Matrix::scale(3.0, 3.0)));
    let mut k0 = Keyframe::with(vec![a]);
    k0.tween = Tween::Classic { ease: 0.0, rotate_turns: 0 };
    l.keyframes.insert(0, k0);
    l.keyframes.insert(10, Keyframe::with(vec![b]));
    l.end = 11;
    let at5 = |l: &Layer| layer_elements_at(l, 5).unwrap().0[0].matrix.decompose();
    let lin = at5(&l);
    assert!((lin.tx - 50.0).abs() < 0.01 && (lin.scale_x - 2.0).abs() < 0.01);
    // ease only position (Quad In: slow start) — scale stays linear
    l.keyframes.get_mut(&0).unwrap().prop_eases = Some(Box::new(PropEases { position: Some(Ease::preset(EaseFamily::Quad, EaseDir::In)), ..Default::default() }));
    let d = at5(&l);
    assert!((d.tx - 25.0).abs() < 0.5, "position eased: {}", d.tx);
    assert!((d.scale_x - 2.0).abs() < 0.01, "scale untouched: {}", d.scale_x);
}

#[test]
fn tween_option_commands() {
    use ac_core::commands::execute;
    use serde_json::json;
    let mut ed = Editor::new(Default::default());
    execute(&mut ed, "timeline.set_label", &json!({"label": "todo", "type": "comment"})).unwrap();
    execute(&mut ed, "timeline.set_tween_options", &json!({"sync": true, "snap": false})).unwrap();
    execute(&mut ed, "timeline.set_property_ease", &json!({"property": "rotation", "ease": "Bounce Out"})).unwrap();
    let info = execute(&mut ed, "frame.info", &json!({})).unwrap();
    assert_eq!(info["label_type"], "comment");
    assert_eq!(info["sync"], true);
    assert_eq!(info["snap"], false);
    let k = &ed.timeline().layers[0].keyframes[&0];
    assert!(k.prop_eases.as_ref().unwrap().rotation.is_some());
    execute(&mut ed, "timeline.set_property_ease", &json!({"property": "rotation"})).unwrap();
    assert!(ed.timeline().layers[0].keyframes[&0].prop_eases.is_none(), "clearing the last one drops the set");
}

#[test]
fn dragging_a_middle_span_end_moves_later_keyframes() {
    use ac_core::commands::execute;
    use serde_json::json;
    let mut l = Layer::new("a", 0);
    l.keyframes.insert(5, Keyframe::blank());
    l.keyframes.insert(9, Keyframe::blank());
    l.end = 12;
    let mut ed = Editor::new(Default::default());
    ed.doc.scene.layers = vec![l];
    // span 1–5 (0-based 0..5) now ends at frame 8 (1-based): later keys shift by +3
    execute(&mut ed, "timeline.set_span_end", &json!({"frame": 1, "end": 8})).unwrap();
    let l = &ed.timeline().layers[0];
    assert_eq!(l.keyframes.keys().copied().collect::<Vec<_>>(), [0, 8, 12]);
    assert_eq!(l.end, 15);
    // shrink it again
    execute(&mut ed, "timeline.set_span_end", &json!({"frame": 1, "end": 2})).unwrap();
    let l = &ed.timeline().layers[0];
    assert_eq!(l.keyframes.keys().copied().collect::<Vec<_>>(), [0, 2, 6]);
    assert_eq!(l.end, 9);
}

fn motion_doc() -> Editor {
    use ac_core::commands::execute;
    use serde_json::json;
    let mut ed = Editor::new(Default::default());
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 40, "h": 40, "fill": "#3366FF"})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.to_symbol", &json!({"name": "Box", "kind": "movieclip"})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.add_filter", &json!({"type": "blur", "blur_x": 0, "blur_y": 0})).unwrap();
    execute(&mut ed, "timeline.set_tween", &json!({"kind": "motion"})).unwrap();
    ed
}

#[test]
fn motion_tweens_animate_3d_and_filters() {
    use ac_core::commands::execute;
    use serde_json::json;
    let mut ed = motion_doc();
    execute(&mut ed, "motion.set_key", &json!({"prop": "rotation_y", "frame": 11, "value": 60})).unwrap();
    execute(&mut ed, "motion.set_key", &json!({"prop": "filter1.blur_x", "frame": 11, "value": 20})).unwrap();
    let l = &ed.timeline().layers[0];
    let mid = &layer_elements_at(l, 5).unwrap().0[0];
    assert!((mid.three_d.unwrap().ry - 30.0).abs() < 1.0, "{:?}", mid.three_d);
    let ac_core::fx::Filter::Blur { blur_x, .. } = mid.filters[0] else { panic!() };
    assert!((blur_x - 10.0).abs() < 0.5, "{blur_x}");
    let info = execute(&mut ed, "motion.info", &json!({})).unwrap();
    assert!(info["keys"]["rotation_y"].is_array() && info["keys"]["filter1.blur_x"].is_array());
}

#[test]
fn changing_3d_at_a_later_frame_keys_it() {
    let mut ed = motion_doc();
    ed.frame = 10;
    ed.selection = vec![Selection { layer: 0, element: 0 }];
    ed.modify_selected("3D", |e| e.three_d = Some(ac_core::deform::Transform3D { rx: 40.0, ..Default::default() }));
    let l = &ed.timeline().layers[0];
    assert!(l.keyframes[&0].elements[0].three_d.is_none(), "the object itself is unchanged");
    let at = |f| layer_elements_at(l, f).unwrap().0[0].three_d.map(|t| t.rx).unwrap_or(0.0);
    assert!(at(0).abs() < 1e-3 && (at(10) - 40.0).abs() < 1e-3 && (at(5) - 20.0).abs() < 1.0);
}

#[test]
fn bezier_handles_shape_a_property_curve() {
    use ac_core::motion::{PropKey, Track};
    let mut t = Track { keys: vec![PropKey::new(0, 0.0, false), PropKey::new(10, 100.0, false)], ease: None };
    assert!((t.value_at(5.0).unwrap() - 50.0).abs() < 1e-3, "corner keys: linear");
    // flat handles at both ends = ease in-out
    t.set_handles(0, Some([0.0, 0.0, 4.0, 0.0]));
    t.set_handles(10, Some([-4.0, 0.0, 0.0, 0.0]));
    assert!((t.value_at(5.0).unwrap() - 50.0).abs() < 0.5, "symmetric");
    assert!(t.value_at(2.0).unwrap() < 15.0, "slow start: {}", t.value_at(2.0).unwrap());
    // an overshooting out-handle goes above the end value
    t.set_handles(0, Some([0.0, 0.0, 3.0, 300.0]));
    assert!(t.value_at(6.0).unwrap() > 100.0);
}

#[test]
fn copy_and_paste_a_curve() {
    use ac_core::motion::{Motion, Prop};
    let mut m = Motion::default();
    m.rotation.set(0, 0.0);
    m.rotation.set(8, 90.0);
    let c = m.curve(Prop::Rotation);
    assert!(m.paste_curve(Prop::Alpha, c.clone(), 5), "pasted, keys past the span dropped");
    assert_eq!(m.alpha.keys.len(), 1);
    assert!(!m.paste_curve(Prop::X, c, 20));
}
