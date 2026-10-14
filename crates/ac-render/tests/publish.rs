//! Phase 4 publishing: HTML5 Canvas, sprite sheets, animated SVG, Lottie.

use ac_core::*;
use ac_render::formats::{lottie, sprite_sheet, AtlasFormat, SheetOpts};
use ac_render::html5::{publish, Html5Opts};
use ac_render::{export_animated_svg, ExportOpts};

/// 10 frames: a static square, a ball tweening right on frames 1-5, then still.
fn doc() -> Document {
    let mut d = Document { width: 100.0, height: 50.0, fps: 10.0, ..Default::default() };
    let mut bg = Layer::new("bg", 0);
    bg.keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 20.0, 20.0), Some(Color::rgb(0, 0, 255)), None))]));
    bg.end = 10;
    let mut ball = Layer::new("ball", 1);
    let el = |x: f32| Element { name: "ball".into(), matrix: Matrix::translate(x, 25.0), ..Element::group(vec![Element::shape("", Shape::new(Path::ellipse(0.0, 0.0, 5.0, 5.0), Some(Color::rgb(255, 0, 0)), None))]) };
    let mut k0 = Keyframe::with(vec![el(10.0)]);
    k0.tween = Tween::Classic { ease: 0.0, rotate_turns: 0 };
    k0.script = Some("this.stop();".into());
    k0.label = Some("start".into());
    ball.keyframes.insert(0, k0);
    ball.keyframes.insert(4, Keyframe::with(vec![el(90.0)]));
    ball.end = 10;
    d.scene.layers = vec![ball, bg];
    d
}

#[test]
fn html5_shares_static_content_and_carries_scripts() {
    let files = publish(&doc(), &Html5Opts { loop_play: true, ..Default::default() });
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(names, ["index.html", "movie.js", "animationcraft-runtime.js"]);
    let movie = String::from_utf8(files[1].1.clone()).unwrap();
    let json = movie.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';');
    let m: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(m["frames"].as_array().unwrap().len(), 10);
    // the square + 5 ball positions (frames 4-9 are the same) = 6 items, 2 paths
    assert_eq!(m["items"].as_array().unwrap().len(), 6, "{}", m["items"]);
    assert_eq!(m["paths"].as_array().unwrap().len(), 2);
    assert_eq!(m["extra"][0]["js"][0], "this.stop();");
    assert_eq!(m["extra"][0]["n"]["ball"]["x"], 10);
    assert_eq!(m["labels"][0][0], "start");
    let index = String::from_utf8(files[0].1.clone()).unwrap();
    assert!(index.contains("movie.js") && index.contains("<canvas id=\"ac-stage\" width=\"100\" height=\"50\""));
    // one self-contained page
    let single = publish(&doc(), &Html5Opts { single_file: true, ..Default::default() });
    assert_eq!(single.len(), 1);
    let page = String::from_utf8(single[0].1.clone()).unwrap();
    assert!(page.contains("var AC_MOVIE=") && page.contains("AnimationCraft HTML5 Canvas runtime") && !page.contains("src=\"movie.js\""));
}

#[test]
fn sprite_sheet_dedupes_and_writes_atlases() {
    let o = ExportOpts { transparent: true, ..Default::default() };
    let (png_bytes, atlas) = sprite_sheet(&doc(), &o, &SheetOpts::default(), "ball.png").unwrap();
    let a: serde_json::Value = serde_json::from_str(&atlas).unwrap();
    let frames = a["frames"].as_object().unwrap();
    assert_eq!(frames.len(), 10);
    // frames 5..10 show the same picture: one cell
    assert_eq!(frames["ball0005"]["frame"], frames["ball0009"]["frame"]);
    assert_ne!(frames["ball0000"]["frame"], frames["ball0001"]["frame"]);
    assert_eq!(a["meta"]["image"], "ball.png");
    let dec = png::Decoder::new(std::io::Cursor::new(png_bytes)).read_info().unwrap();
    let (w, h) = (dec.info().width, dec.info().height);
    assert_eq!((a["meta"]["size"]["w"].as_u64().unwrap() as u32, a["meta"]["size"]["h"].as_u64().unwrap() as u32), (w, h));
    // trimmed: the first frame's cell is the content's bounding box
    assert!(frames["ball0000"]["trimmed"].as_bool().unwrap());
    let xml = sprite_sheet(&doc(), &o, &SheetOpts { format: AtlasFormat::StarlingXml, ..Default::default() }, "ball.png").unwrap().1;
    assert_eq!(xml.matches("<SubTexture").count(), 10);
    let arr = sprite_sheet(&doc(), &o, &SheetOpts { format: AtlasFormat::JsonArray, ..Default::default() }, "ball.png").unwrap().1;
    let arr: serde_json::Value = serde_json::from_str(&arr).unwrap();
    assert_eq!(arr["frames"][3]["filename"], "ball0003");
}

#[test]
fn animated_svg_and_lottie_cover_every_frame() {
    let svg = export_animated_svg(&doc(), &ExportOpts::default());
    // the square is in every frame group; frames 4-9 share one group
    assert_eq!(svg.matches("<animate ").count(), 5, "{svg}");
    assert!(svg.contains("repeatCount=\"indefinite\""));
    let (json, warn) = lottie(&doc(), &ExportOpts::default());
    assert!(warn.is_empty(), "{warn:?}");
    let l: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(l["op"], 10);
    assert_eq!(l["w"], 100);
    let layers = l["layers"].as_array().unwrap();
    // 5 distinct frames (one shape layer each) + the stage colour
    assert_eq!(layers.len(), 6);
    assert_eq!((layers[4]["ip"].as_u64(), layers[4]["op"].as_u64()), (Some(4), Some(10)));
    // the red ball is a bezier with a red fill
    let shapes = layers[0]["shapes"].as_array().unwrap();
    assert!(shapes.iter().any(|g| g["it"].as_array().unwrap().iter().any(|x| x["ty"] == "fl" && x["c"]["k"][0] == 1)));
}

/// A movie clip blinking red/green/blue/white on a main timeline that a
/// script stops on frame 1.
fn held_doc() -> Document {
    let mut d = Document { width: 40.0, height: 40.0, fps: 10.0, ..Default::default() };
    let mut tl = Timeline::new("Blink");
    let cols = [Color::rgb(255, 0, 0), Color::rgb(0, 255, 0), Color::rgb(0, 0, 255), Color::rgb(255, 255, 255)];
    for (i, c) in cols.iter().enumerate() {
        tl.layers[0].keyframes.insert(i as u32, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 40.0, 40.0), Some(*c), None))]));
    }
    tl.layers[0].end = 4;
    d.library.push(Symbol { id: 1, name: "Blink".into(), kind: SymbolKind::MovieClip, timeline: tl });
    d.next_symbol_id = 2;
    let mut k = Keyframe::with(vec![Element::instance("", 1, Matrix::IDENTITY)]);
    k.script = Some("this.stop();".into());
    d.scene.layers[0].keyframes.insert(0, k);
    d.scene.layers[0].end = 2;
    d
}

#[test]
fn held_frames_keep_movie_clips_playing() {
    let d = held_doc();
    assert_eq!(d.movie_clip_lengths(0), vec![4]);
    let files = publish(&d, &Html5Opts::default());
    let movie = String::from_utf8(files[1].1.clone()).unwrap();
    let json = movie.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';');
    let m: serde_json::Value = serde_json::from_str(json).unwrap();
    let v = m["extra"][0]["v"].as_array().expect("clock variants for the held frame");
    assert_eq!(v.len(), 4);
    assert_ne!(v[0], v[1]);
    assert_eq!(v[0], m["frames"][0], "phase 0 is the normal frame");
}

#[test]
fn lottie_masks_become_track_mattes() {
    let mut d = Document { width: 100.0, height: 100.0, ..Default::default() };
    let mut mask = Layer::new("Mask", 0);
    mask.kind = LayerKind::Mask;
    mask.keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::ellipse(50.0, 50.0, 30.0, 30.0), Some(Color::rgb(0, 0, 0)), None))]));
    let mut art = Layer::new("Art", 1);
    art.parent = Some(0);
    art.keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 100.0, 100.0), Some(Color::rgb(255, 0, 0)), None))]));
    d.scene.layers = vec![mask, art];
    let (json, warn) = lottie(&d, &ExportOpts::default());
    assert!(warn.is_empty(), "{warn:?}");
    let l: serde_json::Value = serde_json::from_str(&json).unwrap();
    let layers = l["layers"].as_array().unwrap();
    let matte = layers.iter().position(|x| x["td"] == 1).expect("a matte layer");
    assert_eq!(layers[matte + 1]["tt"], 1, "the masked content follows its matte");
}
