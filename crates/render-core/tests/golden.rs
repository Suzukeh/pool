//! ゴールデンテスト：複合シーンの確定ピクセル検証。
//! GPU 差異に強いよう内側ピクセルのみ・許容誤差付き。

use pool_render_core::Renderer;
use pool_timeline_model::*;

fn num(name: &str, v: f64) -> Param {
    Param {
        name: name.to_string(),
        value: Value::Number(v),
    }
}

fn obj(id: &str, kind: ObjectKind, start: i64, end: i64, params: Vec<Param>) -> TimelineObject {
    TimelineObject {
        id: id.to_string(),
        name: id.to_string(),
        kind,
        start_frame: start,
        end_frame: end,
        params,
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

#[test]
fn composite_scene_golden_pixels() {
    let mut renderer = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("SKIP: {e}");
            return;
        }
    };
    let mut p = Project::new("golden");
    let scene = &mut p.scenes[0];
    scene.width = 480;
    scene.height = 270;
    scene.bg_color = Rgba {
        r: 0.1,
        g: 0.1,
        b: 0.12,
        a: 1.0,
    };
    scene.layers[0].objects.push(obj(
        "rect",
        ObjectKind::Shape {
            shape: ShapeKind::Rectangle,
        },
        0,
        90,
        vec![
            num("x", 140.0),
            num("y", 135.0),
            num("w", 200.0),
            num("h", 120.0),
            Param {
                name: "fill".to_string(),
                value: Value::Color(Rgba {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
            },
        ],
    ));
    scene.layers[0].objects.push(obj(
        "text",
        ObjectKind::Text {
            body: "Hi".to_string(),
        },
        0,
        90,
        vec![num("x", 240.0), num("y", 40.0), num("size", 44.0)],
    ));
    scene.layers.push(Layer {
        id: "layer-2".to_string(),
        name: "Layer2".to_string(),
        visible: true,
        locked: false,
        objects: vec![
            obj(
                "ellipse",
                ObjectKind::Shape {
                    shape: ShapeKind::Ellipse,
                },
                0,
                90,
                vec![
                    num("x", 350.0),
                    num("y", 135.0),
                    num("w", 160.0),
                    num("h", 160.0),
                    Param {
                        name: "fill".to_string(),
                        value: Value::Color(Rgba {
                            r: 0.0,
                            g: 0.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                    },
                ],
            ),
            obj(
                "bright",
                ObjectKind::Filter {
                    effect: "brightness".to_string(),
                },
                0,
                90,
                vec![num("brightness", 0.1)],
            ),
        ],
    });

    let rgba = renderer.render_scene(&p.scenes[0], 10, 480, 270).unwrap();
    // 背景角：(0.1+0.1) → (51,51,56)
    assert!(near(px(&rgba, 480, 5, 265), [51, 51, 56, 255], 10), "bg");
    // 矩形内：赤+0.1 → (255,26,26)
    assert!(
        near(px(&rgba, 480, 140, 150), [255, 26, 26, 255], 10),
        "rect"
    );
    // 楕円中心：青+0.1 → (26,26,255)
    assert!(
        near(px(&rgba, 480, 350, 135), [26, 26, 255, 255], 10),
        "ellipse"
    );
    // テキストにインクがあること
    let ink = rgba
        .chunks_exact(4)
        .filter(|p| p[0] > 200 && p[1] > 200 && p[2] > 200)
        .count();
    assert!(ink > 50, "text ink missing: {ink}");
}
