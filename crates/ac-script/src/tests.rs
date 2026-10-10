use super::*;
use ac_core::{Document, Editor};

fn opts() -> ScriptOptions {
    ScriptOptions { timeout: Duration::from_secs(20), base_dir: std::env::temp_dir(), ..Default::default() }
}

fn have(prog: &str) -> bool {
    Command::new(prog).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

#[test]
fn js_drives_the_editor_as_one_undo_step() {
    let mut ed = Editor::new(Document::default());
    let r = run_js(&mut ed, Language::Js.example(), &opts());
    assert!(r.ok(), "{:?} {:?}", r.error, r.log);
    assert_eq!(r.log, vec!["Stars: 5"]);
    assert_eq!(ed.timeline().layers.len(), 2);
    assert_eq!(ed.undo_label(), Some("Run Script"));
    // on a busy document the example only touches its own layer
    let mut demo = Editor::new(ac_core::demo::demo_document());
    let before: Vec<_> = demo.doc.scene.layers.iter().map(|l| l.keyframes.keys().copied().collect::<Vec<_>>()).collect();
    let r = run_js(&mut demo, Language::Js.example(), &opts());
    assert_eq!(r.log, vec!["Stars: 5"]);
    let after: Vec<_> = demo.doc.scene.layers.iter().skip(1).map(|l| l.keyframes.keys().copied().collect::<Vec<_>>()).collect();
    assert_eq!(before, after, "other layers untouched");
    ed.undo();
    assert_eq!(ed.timeline().layers.len(), 1, "one undo reverts the whole script");
}

#[test]
fn js_errors_are_reported_with_location() {
    let mut ed = Editor::new(Document::default());
    let r = run_js(&mut ed, "ac.draw.rect({ x: 1 });", &opts());
    let e = r.error.unwrap();
    assert!(e.starts_with("Error: draw.rect") && e.contains("missing"), "{e}");
    let r = run_js(&mut ed, "let x = ;", &opts());
    let e = r.error.unwrap();
    assert!(e.contains("SyntaxError"), "{e}");
}

#[test]
fn js_infinite_loop_is_stopped() {
    let mut ed = Editor::new(Document::default());
    let o = ScriptOptions { timeout: Duration::from_millis(300), ..opts() };
    let r = run_js(&mut ed, "while (true) {}", &o);
    assert!(r.error.unwrap().contains("longer than"));
    // the editor is intact afterwards
    assert!(run_js(&mut ed, "ac.draw.rect({x:0,y:0,w:5,h:5})", &opts()).ok());
}

#[test]
fn js_can_export() {
    let mut ed = Editor::new(ac_core::demo::demo_document());
    let dir = std::env::temp_dir().join("ac-script-test-export");
    let _ = std::fs::create_dir_all(&dir);
    let o = ScriptOptions { base_dir: dir.clone(), ..opts() };
    let r = run_js(&mut ed, "ac.export.png({ path: 'f.png', frame: 5 }); ac.doc.save({ path: 'd.acx.json' });", &o);
    assert!(r.ok(), "{:?}", r.error);
    assert!(dir.join("f.png").exists() && dir.join("d.acx.json").exists());
}

#[test]
fn python_bridge() {
    if !have("python3") && !have("python") {
        eprintln!("python not installed; skipping");
        return;
    }
    let mut ed = Editor::new(Document::default());
    let r = run_code(&mut ed, Language::Python, Language::Python.example(), &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
    assert!(r.log.iter().any(|l| l == "Stars: 5"), "{:?}", r.log);
    assert_eq!(ed.timeline().layers.len(), 2);
    assert!(r.commands >= 9);
    // Python exceptions come back as an error with the traceback in the log
    let r = run_code(&mut ed, Language::Python, "import animatecraft as ac\nac.draw.rect(x=1)\n", &opts());
    assert!(r.error.is_some());
    assert!(r.log.iter().any(|l| l.contains("missing number")), "{:?}", r.log);
}

#[test]
fn node_bridge() {
    if !have("node") {
        eprintln!("node not installed; skipping");
        return;
    }
    let mut ed = Editor::new(Document::default());
    let r = run_code(&mut ed, Language::Node, Language::Node.example(), &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
    assert!(r.log.iter().any(|l| l.contains("Layers: 2")), "{:?}", r.log);
}

#[test]
fn ruby_bridge() {
    if !have("ruby") {
        eprintln!("ruby not installed; skipping");
        return;
    }
    let mut ed = Editor::new(Document::default());
    let r = run_code(&mut ed, Language::Ruby, Language::Ruby.example(), &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
    assert!(r.log.iter().any(|l| l.contains("Layers: 2")), "{:?}", r.log);
}

#[test]
fn missing_interpreter_is_a_clear_error() {
    let mut ed = Editor::new(Document::default());
    let dir = std::env::temp_dir();
    let p = dir.join("ac-test.xyz");
    std::fs::write(&p, "x").unwrap();
    let mut o = opts();
    o.interpreters.push(("xyz".into(), vec![vec!["definitely-not-a-real-interpreter".into()]]));
    let r = run_file(&mut ed, &p, &o);
    assert!(r.error.unwrap().contains("couldn't start"));
}

#[test]
fn frame_scripts_control_playback() {
    let mut fp = FramePlayer::new().unwrap();
    assert_eq!(fp.run("this.stop();", 3, 10, &[]).unwrap(), vec![PlayAction::Stop]);
    let labels = vec![("loop".to_string(), 4)];
    assert_eq!(fp.run("this.gotoAndPlay('loop');", 9, 10, &labels).unwrap(), vec![PlayAction::Goto { frame: 4, play: true }]);
    assert_eq!(fp.run("gotoAndStop(2)", 9, 10, &labels).unwrap(), vec![PlayAction::Goto { frame: 2, play: false }]);
    // state persists between frames, like a published movie
    fp.run("this.count = (this.count || 0) + 1; trace('n', this.count);", 0, 10, &[]).unwrap();
    fp.run("this.count += 1; trace('n', this.count); if (this.currentFrame === 5) this.stop();", 5, 10, &[]).unwrap();
    assert_eq!(fp.take_log(), vec!["n 1", "n 2"]);
    assert!(fp.run("gotoAndPlay('nope')", 0, 10, &labels).unwrap_err().contains("nope"));
}

#[test]
fn frame_scripts_are_sandboxed() {
    let mut fp = FramePlayer::new().unwrap();
    // no editor API, no host I/O
    fp.run("trace(typeof ac, typeof __ac_call, typeof require, typeof std, typeof os, typeof fetch)", 0, 1, &[]).unwrap();
    assert_eq!(fp.take_log(), vec!["undefined undefined undefined undefined undefined undefined"]);
    // runaway loops and memory are stopped, and the player keeps working
    assert!(fp.run("for(;;){}", 0, 1, &[]).unwrap_err().contains("stopped"));
    assert!(fp.run("let a = []; for(;;) a.push(new Array(100000).fill(1));", 0, 1, &[]).is_err());
    assert_eq!(fp.run("stop()", 0, 1, &[]).unwrap(), vec![PlayAction::Stop]);
}
