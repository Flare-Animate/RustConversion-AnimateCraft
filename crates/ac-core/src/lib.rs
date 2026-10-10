//! AnimateCraft core: document model, timeline evaluation, tweens, editing
//! commands. No UI or rendering dependencies (portable to wasm).

pub mod demo;
pub mod editor;
pub mod frames;
pub mod geom;
pub mod model;
pub mod xfl;

pub use editor::{Editor, FrameSel, LayerFlag, Selection};
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
        ed.selection = vec![Selection { layer: 0, element: 0 }];
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


    fn boxed(x: f32) -> Element {
        Element::shape("box", Shape { path: Path::rect(x, 0., 10., 10.), fill: Some(Color::rgb(0, 0, 0)), stroke: None })
    }
    fn kfs(ed: &Editor, layer: usize) -> Vec<u32> {
        ed.timeline().layers[layer].keyframes.keys().copied().collect()
    }

    #[test]
    fn delete_removes_object_from_whole_span() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.set_frame(9);
        ed.insert_frame(); // static span 1..10
        ed.set_frame(5);
        assert_eq!(ed.evaluate_current(5).len(), 1);
        ed.select(ed.hit_test(pt(5.0, 5.0)), false);
        assert_eq!(ed.delete_selected(), 1);
        for f in [0, 5, 9] {
            assert!(ed.evaluate_current(f).is_empty(), "frame {f} still shows the box");
        }
        assert_eq!(kfs(&ed, 0), vec![0], "no stray keyframe created");
    }

    #[test]
    fn delete_inside_tween_removes_both_ends() {
        let mut doc = demo::demo_document();
        doc.scene.layers[0].locked = false;
        let mut ed = Editor::new(doc);
        ed.set_frame(4);
        let x = ed.evaluate_current(4).iter().find(|i| i.src == Some((0, 0))).unwrap().matrix.tx;
        let y = ed.evaluate_current(4).iter().find(|i| i.src == Some((0, 0))).unwrap().matrix.ty;
        ed.select(ed.hit_test(pt(x, y)), false);
        assert_eq!(ed.selection, vec![Selection { layer: 0, element: 0 }]);
        ed.delete_selected();
        for f in 0..=9 {
            assert!(ed.evaluate_current(f).iter().all(|i| i.src.map(|s| s.0) != Some(0)), "ball visible at {f}");
        }
        ed.undo();
        assert!(ed.evaluate_current(4).iter().any(|i| i.src == Some((0, 0))));
    }

    #[test]
    fn multi_select_marquee_move_delete() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.add_element(boxed(50.0));
        ed.add_element(boxed(200.0));
        ed.select_in_rect(pt(-5.0, -5.0), pt(70.0, 20.0), false);
        assert_eq!(ed.selection.len(), 2);
        ed.modify_selected("Move", |e| e.matrix.ty += 30.0);
        let (lo, _) = ed.selection_bounds().unwrap();
        assert!(close(lo.y, 30.0));
        ed.delete_selected();
        assert_eq!(ed.evaluate_current(0).len(), 1);
        ed.select_all();
        assert_eq!(ed.selection.len(), 1);
    }

    #[test]
    fn copy_paste_duplicate() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.duplicate();
        assert_eq!(ed.evaluate_current(0).len(), 2);
        let (lo, _) = ed.selection_bounds().unwrap();
        assert!(close(lo.x, 10.0), "duplicate is offset");
        ed.copy_selected();
        ed.paste(true);
        assert_eq!(ed.evaluate_current(0).len(), 3);
    }

    #[test]
    fn frame_selection_ops() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.add_layer();
        // two layers, select frames 5..=8 on both and convert to keyframes
        ed.select_frames(FrameSel { layer0: 0, layer1: 1, f0: 4, f1: 7 });
        ed.insert_keyframe();
        assert_eq!(kfs(&ed, 0), vec![0, 4, 5, 6, 7]);
        assert_eq!(kfs(&ed, 1), vec![0, 4, 5, 6, 7]);
        ed.clear_keyframe();
        assert_eq!(kfs(&ed, 1), vec![0]);
        assert_eq!(ed.timeline().layers[1].end, 8);
        // remove those frames again
        ed.remove_frame();
        assert_eq!(ed.timeline().layers[1].end, 4);
    }

    #[test]
    fn clear_frames_keeps_later_content() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.set_frame(9);
        ed.insert_frame();
        ed.select_frames(FrameSel::between((0, 2), (0, 4)));
        ed.clear_frames();
        assert_eq!(ed.evaluate_current(1).len(), 1);
        assert!(ed.evaluate_current(3).is_empty());
        assert_eq!(ed.evaluate_current(6).len(), 1, "frames after the cleared block keep their content");
    }

    #[test]
    fn move_and_paste_frames() {
        let mut ed = Editor::new(Document::default());
        ed.set_frame(4);
        ed.add_element(boxed(0.0)); // creates keyframe at 4 (past end)
        assert_eq!(kfs(&ed, 0), vec![0, 4]);
        ed.select_frames(FrameSel::cell(0, 4));
        ed.move_frames(3);
        assert_eq!(kfs(&ed, 0), vec![0, 7]);
        assert!(ed.evaluate_current(4).is_empty());
        assert_eq!(ed.evaluate_current(7).len(), 1);
        ed.copy_frames();
        ed.select_frames(FrameSel::cell(0, 12));
        ed.paste_frames();
        assert_eq!(ed.evaluate_current(12).len(), 1);
        assert!(ed.timeline().layers[0].end >= 13);
    }

    #[test]
    fn dragging_a_tween_keyframe_stretches_spans() {
        let mut ed = Editor::new(demo::demo_document());
        ed.select_frames(FrameSel::cell(0, 9));
        ed.move_frames(4);
        assert_eq!(kfs(&ed, 0), vec![0, 13, 18, 27, 36, 47], "no blank or pinned keyframes left behind");
        let x = |f| ed.evaluate_current(f).iter().find(|i| i.src == Some((0, 0))).unwrap().matrix.tx;
        assert!(x(11) > x(0) && x(11) < x(13), "frames 10-13 now belong to the first tween");
        assert!(matches!(ed.timeline().layers[0].keyframes[&13].tween, Tween::Classic { .. }));
    }

    #[test]
    fn tween_on_selection() {
        let mut ed = Editor::new(Document::default());
        ed.add_element(boxed(0.0));
        ed.set_frame(10);
        ed.insert_keyframe();
        ed.select_all();
        ed.modify_selected("Move", |e| e.matrix.tx = 100.0);
        ed.select_frames(FrameSel::cell(0, 3));
        ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 });
        assert!(ed.evaluate_current(5)[0].matrix.tx > 0.0);
        ed.set_tween_params(100.0, 0);
        assert!(matches!(ed.timeline().layers[0].keyframes[&0].tween, Tween::Classic { ease, .. } if ease == 100.0));
        assert!(matches!(ed.timeline().layers[0].keyframes[&10].tween, Tween::None), "closing keyframe has no tween");
    }

    #[test]
    fn library_delete_removes_instances() {
        let mut ed = Editor::new(demo::demo_document());
        let ball = ed.doc.library.iter().find(|s| s.name == "Ball").unwrap().id;
        assert!(ed.symbol_usage(ball) > 0);
        ed.delete_symbol(ball);
        assert!(ed.doc.symbol(ball).is_none());
        assert_eq!(ed.symbol_usage(ball), 0);
        ac_core_render_smoke(&ed);
    }

    fn ac_core_render_smoke(ed: &Editor) {
        for f in 0..ed.frame_count() {
            let _ = ed.evaluate_current(f);
        }
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
