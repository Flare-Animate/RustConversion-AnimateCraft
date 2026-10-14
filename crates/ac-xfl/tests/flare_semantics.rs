//! Documents using Flare's timeline semantics (retained tween curves, keyframe
//! playback clocks) export to .fla as Animate's model and look the same.

use ac_core::*;

/// A 6-frame graphic (a square stepping right) placed on the scene, plus a
/// classic-tweened square on a second layer with a strong ease.
fn doc() -> Document {
    let mut d = Document { width: 200.0, height: 100.0, ..Default::default() };
    let mut tl = Timeline::new("Stepper");
    let l = &mut tl.layers[0];
    for f in 0..6 {
        let sq = Element::shape("s", Shape::new(Path::rect(f as f32 * 10.0, 0.0, 8.0, 8.0), Some(Color::rgb(0, 100, 200)), None));
        l.keyframes.insert(f, Keyframe::with(vec![sq]));
    }
    l.end = 6;
    d.library.push(Symbol { id: 1, name: "Stepper".into(), kind: SymbolKind::Graphic, timeline: tl });
    let mut inst = Layer::new("inst", 0);
    inst.keyframes.insert(0, Keyframe::with(vec![Element::instance("", 1, Matrix::translate(10.0, 10.0))]));
    inst.end = 20;
    let mut tw = Layer::new("tween", 1);
    let a = Element::shape("r", Shape::new(Path::rect(0.0, 60.0, 10.0, 10.0), Some(Color::rgb(200, 0, 0)), None));
    let mut b = a.clone();
    b.matrix = Matrix::translate(150.0, 0.0);
    let mut k0 = Keyframe::with(vec![a]);
    k0.tween = Tween::Classic { ease: 100.0, rotate_turns: 0 };
    tw.keyframes.insert(0, k0);
    tw.keyframes.insert(19, Keyframe::with(vec![b]));
    tw.end = 20;
    d.scene.layers = vec![inst, tw];
    d
}

fn frames(d: &Document) -> Vec<String> {
    // world-space bounds of every item, to 0.05 px (imported shapes keep their
    // position in the path rather than the matrix)
    let r = |v: f32| (v * 20.0).round() / 20.0;
    (0..20)
        .map(|f| {
            let b: Vec<_> = evaluate_scene(d, f).iter().filter_map(|i| i.path.transformed(&i.matrix).bounds()).map(|(a, b)| [r(a.x), r(a.y), r(b.x), r(b.y)]).collect();
            format!("{f}: {b:?}")
        })
        .collect()
}

#[test]
fn retained_curves_and_clocks_round_trip_through_fla() {
    let mut d = doc();
    frames::set_retain_tween_curves(true);
    // split both layers at frame 7: the graphic keeps playing, the eased tween keeps its curve
    for l in &mut d.scene.layers {
        frames::convert_to_keyframes(l, 7, 7, false);
    }
    // and reverse the tween's second half
    frames::reverse_frames(&mut d.scene.layers[1], 7, 19);
    frames::set_retain_tween_curves(false);
    assert!(d.scene.layers[1].keyframes.values().any(|k| k.tween_boundary.is_some()));
    assert_eq!(d.scene.layers[0].keyframes[&7].playback_offset, 7);
    let before = frames(&d);

    let (pkg, warnings) = ac_xfl::write_package(&d);
    assert!(warnings.iter().any(|w| w.contains("frame by frame")), "{warnings:?}");
    let back = ac_xfl::read_package(&pkg).unwrap().doc;
    assert_eq!(frames(&back), before, "same picture on every frame after .fla");
    // the clock became the instance's First Frame
    let ElementKind::Instance(i) = &back.scene.layers[0].keyframes[&7].elements[0].kind else { panic!() };
    assert_eq!(i.first_frame, 7 % 6);
}
