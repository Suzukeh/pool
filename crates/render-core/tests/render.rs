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
