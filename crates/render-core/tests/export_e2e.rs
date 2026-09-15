//! 書出し E2E：render-core → Mp4Writer → probe。ffmpeg 必須。

use pool_ffmpeg_io::{Mp4Writer, find_ffmpeg, probe};
use pool_render_core::Renderer;
use pool_timeline_model::*;

#[test]
fn timeline_to_mp4() {
    if find_ffmpeg().is_err() {
        eprintln!("SKIP: ffmpeg not found");
        return;
    }
    let mut renderer = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("SKIP: {e}");
            return;
        }
    };
    let mut p = Project::new("e2e");
    p.scenes[0].layers[0].objects.push(TimelineObject {
        id: "a".to_string(),
        name: "a".to_string(),
        kind: ObjectKind::Shape {
            shape: ShapeKind::Rectangle,
        },
        start_frame: 0,
        end_frame: 5,
        params: vec![
            Param {
                name: "x".to_string(),
                value: Value::Number(80.0),
            },
            Param {
                name: "y".to_string(),
                value: Value::Number(45.0),
            },
            Param {
                name: "w".to_string(),
                value: Value::Number(100.0),
            },
            Param {
                name: "h".to_string(),
                value: Value::Number(50.0),
            },
        ],
        keyframes: vec![],
        loops: Default::default(),
    });
    let out = std::env::temp_dir().join(format!("pool-e2e-{}.mp4", std::process::id()));
    let (w, h) = (160u32, 90u32);
    let mut writer = Mp4Writer::new(&out, w, h, 30.0).unwrap();
    for f in 0..5 {
        let rgba = renderer.render_scene(&p.scenes[0], f, w, h).unwrap();
        writer.write_frame(&rgba).unwrap();
    }
    assert_eq!(writer.finish().unwrap(), 5);
    let info = probe(&out).unwrap();
    assert_eq!((info.width, info.height), (w, h));
    assert!(info.duration_secs > 0.0);
    let _ = std::fs::remove_file(&out);
}
