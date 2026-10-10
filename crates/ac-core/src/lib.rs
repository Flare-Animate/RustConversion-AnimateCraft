//! AnimateCraft core: document model, timeline evaluation, tweens, editing
//! commands. No UI or rendering dependencies (portable to wasm).

pub mod demo;
pub mod editor;
pub mod geom;
pub mod model;
pub mod xfl;

pub use editor::{Editor, LayerFlag, Selection};
pub use geom::{pt, Matrix, Path, Pt, Seg};
pub use model::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn matrix_decompose_roundtrip() {
        for m in [
            Matrix::IDENTITY,
            Matrix::translate(10.0, -4.0).concat(&Matrix::rotate(0.7)).concat(&Matrix::scale(2.0, 0.5)),
            Matrix::scale(-1.0, 1.0),
            Matrix::scale(1.0, -3.0).concat(&Matrix::rotate(-2.0)),
        ] {
            let r = Matrix::compose(&m.decompose());
            for (x, y) in [(r.a, m.a), (r.b, m.b), (r.c, m.c), (r.d, m.d), (r.tx, m.tx), (r.ty, m.ty)] {
                assert!(close(x, y), "{m:?} -> {r:?}");
            }
        }
    }

    #[test]
    fn matrix_invert() {
        let m = Matrix::translate(5.0, 7.0).concat(&Matrix::rotate(1.1)).concat(&Matrix::scale(2.0, 3.0));
        let p = pt(3.0, -2.0);
        let q = m.invert().unwrap().apply(m.apply(p));
        assert!(close(p.x, q.x) && close(p.y, q.y));
    }

    #[test]
    fn classic_tween_rotation_takes_shortest_path() {
        let a = Matrix::rotate(170f32.to_radians());
        let b = Matrix::rotate(-170f32.to_radians());
        let mid = a.interpolate(&b, 0.5, 0).decompose();
        assert!(close(mid.skew_y.abs(), PI), "went the long way: {}", mid.skew_y.to_degrees());
        let spun = Matrix::IDENTITY.interpolate(&Matrix::IDENTITY, 0.5, 1).decompose();
        assert!(close(spun.skew_y.abs(), PI), "rotate CW 1x should be half-turned at t=.5");
    }

    #[test]
    fn ease_endpoints_and_direction() {
        for e in [-100.0, -30.0, 0.0, 50.0, 100.0] {
            assert!(close(apply_ease(0.0, e), 0.0) && close(apply_ease(1.0, e), 1.0));
        }
        assert!(apply_ease(0.5, 100.0) > 0.5, "ease out is fast at the start");
        assert!(apply_ease(0.5, -100.0) < 0.5, "ease in is slow at the start");
    }

    #[test]
    fn tween_hits_keyframes_exactly() {
        let doc = demo::demo_document();
        let ball = &doc.scene.layers[0];
        for (&f, kf) in &ball.keyframes {
            let (els, _) = model::layer_elements_at(ball, f).unwrap();
            assert_eq!(els[0].matrix, kf.elements[0].matrix, "frame {f}");
        }
        // mid-tween lies between the two keyframes
        let (els, _) = model::layer_elements_at(ball, 4).unwrap();
        let x = els[0].matrix.tx;
        assert!(x > 50.0 && x < 150.0, "x={x}");
    }

    #[test]
    fn shape_tween_endpoints() {
        let a = Path::rect(0.0, 0.0, 10.0, 10.0);
        let b = Path::star(5.0, 5.0, 8.0, 3.0, 5);
        let m0 = geom::morph_paths(&a, &b, 0.0);
        let (lo, hi) = m0.bounds().unwrap();
        assert!(close(lo.x, 0.0) && close(hi.x, 10.0));
        let m1 = geom::morph_paths(&a, &b, 1.0);
        let (lo1, hi1) = m1.bounds().unwrap();
        let (blo, bhi) = b.bounds().unwrap();
        assert!(close(lo1.x, blo.x) && close(hi1.y, bhi.y));
    }

    #[test]
    fn graphic_symbol_syncs_to_parent() {
        let mut doc = Document::default();
        let mut tl = Timeline::new("blink");
        tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("a", Shape { path: Path::rect(0., 0., 1., 1.), fill: Some(Color::rgb(1, 0, 0)), stroke: None })]));
        tl.layers[0].keyframes.insert(2, Keyframe::with(vec![Element::shape("b", Shape { path: Path::rect(0., 0., 1., 1.), fill: Some(Color::rgb(0, 0, 1)), stroke: None })]));
        tl.layers[0].end = 4;
        let id = doc.add_symbol("blink", SymbolKind::Graphic, tl);
        doc.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::instance("i", id, Matrix::IDENTITY)]));
        doc.scene.layers[0].end = 10;
        let fill = |doc: &Document, f| model::evaluate_scene(doc, f)[0].fill.unwrap();
        assert_eq!(fill(&doc, 1), Color::rgb(1, 0, 0));
        assert_eq!(fill(&doc, 2), Color::rgb(0, 0, 1));
        assert_eq!(fill(&doc, 4), Color::rgb(1, 0, 0), "loops after 4 frames");
        if let ElementKind::Instance(i) = &mut doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].kind {
            i.loop_mode = LoopMode::PlayOnce;
        }
        assert_eq!(fill(&doc, 9), Color::rgb(0, 0, 1), "play once holds last frame");
    }

    #[test]
    fn editor_keyframes_tweens_undo() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape { path: Path::rect(0., 0., 10., 10.), fill: Some(Color::rgb(0, 0, 0)), stroke: None }));
        ed.set_frame(9);
        ed.insert_keyframe();
        assert_eq!(ed.timeline().layers[0].end, 10);
        ed.selection = Some(Selection { layer: 0, element: 0 });
        ed.modify_selected("Move", |e| e.matrix = Matrix::translate(100.0, 0.0));
        ed.set_frame(0);
        ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 });
        let mid = ed.evaluate_current(5)[0].matrix.tx;
        assert!(close(mid, 100.0 * 5.0 / 9.0), "mid={mid}");
        ed.undo();
        assert_eq!(ed.evaluate_current(5)[0].matrix.tx, 0.0, "undo removed tween");
        ed.redo();
        assert!(ed.evaluate_current(5)[0].matrix.tx > 0.0);
    }

    #[test]
    fn insert_and_remove_frame_shift_keyframes() {
        let mut ed = Editor::new(Document::default());
        ed.set_frame(4);
        ed.insert_blank_keyframe();
        ed.set_frame(2);
        ed.insert_frame();
        assert!(ed.timeline().layers[0].keyframes.contains_key(&5));
        ed.remove_frame();
        assert!(ed.timeline().layers[0].keyframes.contains_key(&4));
    }

    #[test]
    fn convert_to_symbol_keeps_visual_position() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(Element::shape("box", Shape { path: Path::rect(100., 50., 20., 20.), fill: Some(Color::rgb(0, 0, 0)), stroke: None }));
        let before = ed.selection_bounds().unwrap();
        let id = ed.convert_to_symbol("Box", SymbolKind::Graphic).unwrap();
        let after = ed.selection_bounds().unwrap();
        assert!(close(before.0.x, after.0.x) && close(before.1.y, after.1.y));
        assert!(matches!(ed.selected_element().unwrap().kind, ElementKind::Instance(Instance { symbol, .. }) if symbol == id));
        assert!(ed.hit_test(pt(110.0, 60.0)).is_some());
        assert!(ed.hit_test(pt(10.0, 10.0)).is_none());
    }

    #[test]
    fn json_roundtrip() {
        let doc = demo::demo_document();
        let back = Document::from_json(&doc.to_json()).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn xfl_edges() {
        let p = xfl::parse_edges("!0 0|200 0[400 0 400 200|0 200S2|0 0").unwrap();
        assert_eq!(p.segs[0], Seg::M(pt(0.0, 0.0)));
        assert_eq!(p.segs[1], Seg::L(pt(10.0, 0.0)));
        assert_eq!(p.segs[2], Seg::Q(pt(20.0, 0.0), pt(20.0, 10.0)));
        let h = xfl::parse_edges("!#14.00 #28.80").unwrap();
        assert_eq!(h.segs[0], Seg::M(pt(1.0, 40.5 / 20.0)));
        assert!(xfl::parse_edges("!0 0 ?").is_err());
    }
}
