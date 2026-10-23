//! Manual driver for the P6 render smoke on a real project folder (ignored by default).
//!
//! `P6_PROJECT=<dir> P6_OUT=<file> P6_WIDTH=960 P6_TIMES=1.0,2.5 cargo test -p ferrocast-core --test p6_real -- --ignored`
//! writes one `FrameParams` JSON line per time (the fields `render.frame` takes next to
//! `project`, `t`, `width`). With `P6_EXPORT=1` it writes the NDJSON params file of an export
//! (`P6_FPS`, default 30) and `<P6_OUT>.plan.json` with `frames`, `size`, `segments`, `audio`.

use ferrocast_core::capture::{
    compose::{CameraFx, Composer},
    events::Events,
    export::{self, Codec, ExportOptions, Format, Quality},
    project::Project,
};
use std::path::PathBuf;

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

#[test]
#[ignore = "manual: needs P6_PROJECT and P6_OUT"]
fn dump_params_for_a_real_project() {
    let (Some(dir), Some(out)) = (env("P6_PROJECT"), env("P6_OUT")) else { return };
    let dir = PathBuf::from(dir);
    let project = Project::load(&dir).expect("project");
    let events = Events::decode(&std::fs::read_to_string(dir.join("events.json")).expect("events.json")).expect("events");
    let width: u32 = env("P6_WIDTH").and_then(|w| w.parse().ok()).unwrap_or(960);
    let source = export::source_pixels(&events);
    if env("P6_EXPORT").is_some() {
        let fps: u32 = env("P6_FPS").and_then(|w| w.parse().ok()).unwrap_or(30);
        let opts = ExportOptions { format: Format::Mp4, codec: Codec::H264, width, fps, quality: Quality::Standard };
        let plan = export::export_plan(&project, source, &opts).expect("plan");
        let fx = CameraFx::prepare(&project, &dir, &events, plan.size).expect("fx");
        let mut w = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
        export::write_frame_params_fx(&project, &events, &plan, fx, &mut w).unwrap();
        drop(w);
        let segs: Vec<_> =
            plan.segments.iter().map(|s| serde_json::json!({"src_start": s.src_start, "src_end": s.src_end, "speed": s.speed})).collect();
        let audio = serde_json::to_value(&plan.audio).unwrap();
        let body = serde_json::json!({"frames": plan.frames, "size": plan.size, "fps": plan.fps, "duration": plan.duration, "segments": segs, "audio": audio});
        std::fs::write(format!("{out}.plan.json"), body.to_string()).unwrap();
        return;
    }
    let canvas = export::preview_canvas(&project, source, width);
    let fx = CameraFx::prepare(&project, &dir, &events, canvas).expect("fx");
    let composer = Composer::new(&project, &events).with_camera_fx(fx);
    let times = env("P6_TIMES").unwrap_or_else(|| "0".into());
    let lines: Vec<String> =
        times.split(',').map(|t| serde_json::to_string(&composer.at(t.trim().parse().expect("time"))).unwrap()).collect();
    std::fs::write(out, lines.join("\n") + "\n").unwrap();
}
