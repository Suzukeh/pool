//! GPU レンダーテスト。アダプタがなければスキップ（CI の headless 対策）。

use pool_render_core::Renderer;
use pool_timeline_model::*;

fn renderer_or_skip() -> Option<Renderer> {
    match Renderer::new() {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("SKIP GPU tests: {e}");
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rect(
    id: &str,
    start: i64,
    end: i64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: Rgba,
) -> TimelineObject {
    TimelineObject {
        id: id.to_string(),
        name: id.to_string(),
        kind: ObjectKind::Shape {
            shape: ShapeKind::Rectangle,
        },
        start_frame: start,
        end_frame: end,
        params: vec![
            Param {
                name: "x".to_string(),
                value: Value::Number(x),
            },
            Param {
                name: "y".to_string(),
                value: Value::Number(y),
            },
            Param {
                name: "w".to_string(),
                value: Value::Number(w),
            },
            Param {
                name: "h".to_string(),
                value: Value::Number(h),
            },
            Param {
                name: "fill".to_string(),
                value: Value::Color(fill),
            },
        ],
        keyframes: vec![],
        loops: Default::default(),
    }
}

fn px(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= tol)
}

const RED: Rgba = Rgba {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Rgba = Rgba {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

#[test]
fn red_rect_center_pixel() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    p.scenes[0].width = 320;
    p.scenes[0].height = 180;
    p.scenes[0].layers[0]
        .objects
        .push(rect("a", 0, 90, 160.0, 90.0, 200.0, 100.0, RED));
    let rgba = r.render_scene(&p.scenes[0], 10, 320, 180).unwrap();
    assert_eq!(rgba.len(), 320 * 180 * 4);
    assert!(
        near(px(&rgba, 320, 160, 90), [255, 0, 0, 255], 6),
        "center should be red"
    );
    assert!(
        near(px(&rgba, 320, 5, 5), [0, 0, 0, 255], 6),
        "corner should stay bg, got {:?}",
        px(&rgba, 320, 5, 5)
    );
}

#[test]
fn lower_layer_wins() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    // Layer1（上）に青、Layer2（下）に赤 → 下が手前なので赤が勝つ
    p.scenes[0].layers[0]
        .objects
        .push(rect("blue", 0, 90, 160.0, 90.0, 320.0, 180.0, BLUE));
    p.scenes[0].layers.push(Layer {
        id: "layer-2".to_string(),
        name: "Layer2".to_string(),
        visible: true,
        locked: false,
        objects: vec![rect("red", 0, 90, 160.0, 90.0, 320.0, 180.0, RED)],
    });
    let rgba = r.render_scene(&p.scenes[0], 10, 320, 180).unwrap();
    assert!(near(px(&rgba, 320, 160, 90), [255, 0, 0, 255], 6));
}

#[test]
fn text_produces_ink() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    p.scenes[0].layers[0].objects.push(TimelineObject {
        id: "t".to_string(),
        name: "t".to_string(),
        kind: ObjectKind::Text {
            body: "Hi".to_string(),
        },
        start_frame: 0,
        end_frame: 90,
        params: vec![
            Param {
                name: "x".to_string(),
                value: Value::Number(160.0),
            },
            Param {
                name: "y".to_string(),
                value: Value::Number(90.0),
            },
            Param {
                name: "size".to_string(),
                value: Value::Number(60.0),
            },
        ],
        keyframes: vec![],
        loops: Default::default(),
    });
    let rgba = r.render_scene(&p.scenes[0], 10, 320, 180).unwrap();
    let ink = rgba.chunks_exact(4).filter(|p| p[0] > 128).count();
    assert!(ink > 100, "text should draw white pixels, got {ink}");
}

#[test]
fn brightness_filter_lifts_black() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    p.scenes[0].layers.push(Layer {
        id: "layer-2".to_string(),
        name: "Layer2".to_string(),
        visible: true,
        locked: false,
        objects: vec![TimelineObject {
            id: "f".to_string(),
            name: "f".to_string(),
            kind: ObjectKind::Filter {
                effect: "brightness".to_string(),
            },
            start_frame: 0,
            end_frame: 90,
            params: vec![Param {
                name: "brightness".to_string(),
                value: Value::Number(0.5),
            }],
            keyframes: vec![],
            loops: Default::default(),
        }],
    });
    let rgba = r.render_scene(&p.scenes[0], 10, 320, 180).unwrap();
    assert!(near(px(&rgba, 320, 160, 90), [128, 128, 128, 255], 8));
}

#[test]
fn blur_softens_edge() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    let mut o = rect("a", 0, 90, 160.0, 90.0, 100.0, 100.0, Rgba::WHITE);
    o.params.push(Param {
        name: "blur".to_string(),
        value: Value::Number(8.0),
    });
    p.scenes[0].layers[0].objects.push(o);
    let rgba = r.render_scene(&p.scenes[0], 10, 320, 180).unwrap();
    // 矩形右端(x=210)の4px外：ブラーで中間値になるはず
    let edge = px(&rgba, 320, 214, 90)[0];
    assert!(
        edge > 20 && edge < 235,
        "edge should be softened, got {edge}"
    );
}

#[test]
fn png_output_is_valid() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    p.scenes[0].layers[0]
        .objects
        .push(rect("a", 0, 90, 160.0, 90.0, 100.0, 100.0, RED));
    let png = r.render_png(&p.scenes[0], 10, 160, 90).unwrap();
    assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (160, 90));
}

#[test]
fn rotation_turns_bar_vertical() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut base = rect("a", 0, 90, 160.0, 90.0, 200.0, 100.0, RED);
    base.params.push(Param {
        name: "rotation".to_string(),
        value: Value::Number(90.0),
    });
    let mut p = Project::new("t");
    p.scenes[0].layers[0].objects.push(base);
    let rgba = r.render_scene(&p.scenes[0], 10, 320, 180).unwrap();
    // 90°回転で縦棒になる：真上(160,30)は入り、右(250,90)は抜ける
    assert!(
        near(px(&rgba, 320, 160, 30), [255, 0, 0, 255], 12),
        "top should fill"
    );
    assert!(
        near(px(&rgba, 320, 250, 90), [0, 0, 0, 255], 12),
        "right should clear"
    );
}

#[test]
fn japanese_text_renders_with_system_font() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    p.scenes[0].layers[0].objects.push(TimelineObject {
        id: "jp".to_string(),
        name: "jp".to_string(),
        kind: ObjectKind::Text {
            body: "こんにちは".to_string(),
        },
        start_frame: 0,
        end_frame: 90,
        params: vec![
            Param {
                name: "x".to_string(),
                value: Value::Number(240.0),
            },
            Param {
                name: "y".to_string(),
                value: Value::Number(135.0),
            },
            Param {
                name: "size".to_string(),
                value: Value::Number(64.0),
            },
        ],
        keyframes: vec![],
        loops: Default::default(),
    });
    let rgba = r.render_scene(&p.scenes[0], 10, 480, 270).unwrap();
    let ink = rgba.chunks_exact(4).filter(|p| p[3] > 128).count();
    // tofu（.notdef の箱）でもインクは出るため、厳密判定は環境変数指定時のみ。
    // 目安：本物グリフなら 5 文字で数千 px
    if std::env::var_os("POOL_TEST_REQUIRE_JP").is_some() {
        assert!(ink > 2000, "japanese ink too small: {ink}");
    } else {
        eprintln!("JP ink pixels: {ink} (strict check needs POOL_TEST_REQUIRE_JP=1)");
    }
}

#[test]
fn video_frame_renders() {
    if pool_ffmpeg_io::find_ffmpeg().is_err() {
        eprintln!("SKIP: ffmpeg not found");
        return;
    }
    let src = std::env::temp_dir().join(format!("pool-vid-{}.mp4", std::process::id()));
    let st = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=160x90:d=1:r=30",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&src)
        .status()
        .unwrap();
    assert!(st.success());
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    p.scenes[0].layers[0].objects.push(TimelineObject {
        id: "v".to_string(),
        name: "v".to_string(),
        kind: ObjectKind::Video {
            path: src.to_string_lossy().into_owned(),
        },
        start_frame: 0,
        end_frame: 30,
        params: vec![],
        keyframes: vec![],
        loops: Default::default(),
    });
    let rgba = r.render_scene(&p.scenes[0], 5, 160, 90).unwrap();
    // testsrc はカラフル：明るいピクセルが多数あるはず
    let bright = rgba
        .chunks_exact(4)
        .filter(|p| p[0] > 100 || p[1] > 100 || p[2] > 100)
        .count();
    assert!(
        bright > 3000,
        "video should show colorful frame, got {bright}"
    );
    let _ = std::fs::remove_file(&src);
}

#[test]
fn perf_100_objects_report() {
    let Some(mut r) = renderer_or_skip() else {
        return;
    };
    let mut p = Project::new("t");
    // 100 レイヤー×各1オブジェクト＝100 同時描画（ worst case ）
    for i in 0..100 {
        p.scenes[0].layers.push(Layer {
            id: format!("layer-{i}"),
            name: format!("Layer{i}"),
            visible: true,
            locked: false,
            objects: vec![rect(
                &format!("o{i}"),
                0,
                90,
                ((i * 37) % 480) as f64,
                ((i * 53) % 270) as f64,
                100.0,
                100.0,
                Rgba {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
            )],
        });
    }
    // ウォームアップ
    let _ = r.render_scene(&p.scenes[0], 10, 480, 270).unwrap();
    let t0 = std::time::Instant::now();
    let n = 10;
    for _ in 0..n {
        let _ = r.render_scene(&p.scenes[0], 10, 480, 270).unwrap();
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
    eprintln!(
        "PERF 100 objects @480x270: {ms:.1}ms/frame ({:.1}fps)",
        1000.0 / ms
    );
    assert!(ms < 10_000.0, "absurdly slow: {ms}ms");
}
