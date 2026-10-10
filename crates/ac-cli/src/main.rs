//! animationcraft-cli — headless access to the same engine the editor uses.
//!
//!   animationcraft-cli info   <doc.acx.json | demo>
//!   animationcraft-cli gif    <doc | demo> <out.gif> [scale]
//!   animationcraft-cli png    <doc | demo> <frame> <out.png> [scale]
//!   animationcraft-cli svg    <doc | demo> <frame> <out.svg>
//!   animationcraft-cli png-seq <doc | demo> <out-dir> [scale]
//!   animationcraft-cli video  <doc | demo> <out.mp4|.webm|.mov> [scale]   (needs ffmpeg)
//!   animationcraft-cli save-demo <out.acx.json>
//!   animationcraft-cli run <script.js|.py|.mjs|.rb> [--doc <doc|demo>] [--save out.acx.json] [--gif out.gif]
//!   animationcraft-cli commands            (list scriptable commands)

use ac_core::{demo::demo_document, Document, ElementKind, Tween};
use std::path::Path;

fn load(src: &str) -> Result<Document, String> {
    if src == "demo" {
        return Ok(demo_document());
    }
    let s = std::fs::read_to_string(src).map_err(|e| format!("{src}: {e}"))?;
    Document::from_json(&s).map_err(|e| format!("{src}: {e}"))
}

fn run(args: &[String]) -> Result<(), String> {
    let arg = |i: usize| args.get(i).map(String::as_str).ok_or_else(|| format!("missing argument {i}; run with no args for usage"));
    match arg(0)? {
        "info" => {
            let d = load(arg(1)?)?;
            println!("Stage {}x{} @ {} fps, {} frames", d.width, d.height, d.fps, d.scene.frame_count());
            println!("Layers (top first):");
            for l in &d.scene.layers {
                let kfs: Vec<String> = l.keyframes.iter().map(|(f, k)| {
                    let t = match k.tween { Tween::None => "", Tween::Classic { .. } => " classic→", Tween::Shape { .. } => " shape→" };
                    format!("{}{}", f + 1, t)
                }).collect();
                println!("  {:<8} frames 1-{:<3} keyframes [{}]", l.name, l.end, kfs.join(", "));
            }
            println!("Library:");
            for s in &d.library {
                let insts: usize = s.timeline.layers.iter().flat_map(|l| l.keyframes.values()).flat_map(|k| &k.elements).filter(|e| matches!(e.kind, ElementKind::Instance(_))).count();
                println!("  {:<10} {:?}, {} frames, {} layers, {} nested instances", s.name, s.kind, s.timeline.frame_count(), s.timeline.layers.len(), insts);
            }
        }
        "gif" => {
            let d = load(arg(1)?)?;
            let scale = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            ac_render::export_gif(&d, scale, Path::new(arg(2)?))?;
            println!("wrote {}", arg(2)?);
        }
        "png-seq" => {
            let d = load(arg(1)?)?;
            let o = ac_render::ExportOpts { scale: args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0), ..Default::default() };
            let n = ac_render::export_png_sequence(&d, &o, Path::new(arg(2)?), "frame", &mut |_, _| true)?;
            println!("wrote {n} frames to {}", arg(2)?);
        }
        "video" => {
            let d = load(arg(1)?)?;
            let o = ac_render::ExportOpts { scale: args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0), ..Default::default() };
            ac_render::export_video(&d, &o, Path::new(arg(2)?), "ffmpeg", &mut |a, b| { eprint!("\rframe {a}/{b}"); true })?;
            eprintln!();
            println!("wrote {}", arg(2)?);
        }
        "png" => {
            let d = load(arg(1)?)?;
            let f: u32 = arg(2)?.parse().map_err(|_| "frame must be a number (1-based)")?;
            let scale = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0);
            ac_render::export_png(&d, f.saturating_sub(1), scale, Path::new(arg(3)?))?;
            println!("wrote {}", arg(3)?);
        }
        "svg" => {
            let d = load(arg(1)?)?;
            let f: u32 = arg(2)?.parse().map_err(|_| "frame must be a number (1-based)")?;
            std::fs::write(arg(3)?, ac_render::export_svg(&d, f.saturating_sub(1))).map_err(|e| e.to_string())?;
            println!("wrote {}", arg(3)?);
        }
        "save-demo" => {
            std::fs::write(arg(1)?, demo_document().to_json()).map_err(|e| e.to_string())?;
            println!("wrote {}", arg(1)?);
        }
        "commands" => {
            print!("{}", ac_script::reference());
            println!("\n{}", ac_script::PROTOCOL);
        }
        "run" => {
            let script = std::path::PathBuf::from(arg(1)?);
            let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
            let doc = match flag("--doc") {
                Some(d) => load(d)?,
                None => ac_core::Document::default(),
            };
            let mut ed = ac_core::Editor::new(doc);
            let rep = ac_script::run_file(&mut ed, &script, &ac_script::ScriptOptions::default());
            for line in &rep.log {
                println!("{line}");
            }
            eprintln!("[{} commands in {:.2?}]", rep.commands, rep.elapsed);
            if let Some(e) = rep.error {
                return Err(e);
            }
            if let Some(p) = flag("--save") {
                std::fs::write(p, ed.doc.to_json()).map_err(|e| e.to_string())?;
                eprintln!("saved {p}");
            }
            if let Some(p) = flag("--gif") {
                ac_render::export_gif(&ed.doc, 1.0, Path::new(p))?;
                eprintln!("wrote {p}");
            }
        }
        other => return Err(format!("unknown command '{other}'")),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: animationcraft-cli <info|gif|png|png-seq|video|svg|save-demo|run|commands> ...  (use 'demo' as the document for the built-in scene)");
        std::process::exit(2);
    }
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
