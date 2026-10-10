//! animatecraft-cli — headless access to the same engine the editor uses.
//!
//!   animatecraft-cli info   <doc.acx.json | demo>
//!   animatecraft-cli gif    <doc | demo> <out.gif> [scale]
//!   animatecraft-cli png    <doc | demo> <frame> <out.png> [scale]
//!   animatecraft-cli svg    <doc | demo> <frame> <out.svg>
//!   animatecraft-cli save-demo <out.acx.json>

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
        other => return Err(format!("unknown command '{other}'")),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: animatecraft-cli <info|gif|png|svg|save-demo> ...  (use 'demo' as the document for the built-in scene)");
        std::process::exit(2);
    }
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
