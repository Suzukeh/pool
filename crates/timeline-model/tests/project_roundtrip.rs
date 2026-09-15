//! JSON 仕様の適合テスト：手書き JSON が読め、保存→読込が一致する。

use pool_timeline_model::*;

const SAMPLE: &str = r#"{
  "version": 1,
  "name": "sample",
  "scenes": [
    {
      "id": "scene-root",
      "name": "Root",
      "width": 1920,
      "height": 1080,
      "fps_num": 30,
      "fps_den": 1,
      "sample_rate": 44100,
      "bg_color": { "r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0 },
      "layers": [
        {
          "id": "layer-1",
          "name": "Layer1",
          "visible": true,
          "locked": false,
          "objects": [
            {
              "id": "obj-video",
              "name": "car.mp4",
              "kind": { "type": "video", "path": "car.mp4" },
              "start_frame": 0,
              "end_frame": 150,
              "params": [
                { "name": "opacity", "value": { "type": "number", "value": 1.0 } }
              ],
              "keyframes": [
                {
                  "param": "opacity",
                  "frame": 120,
                  "value": { "type": "number", "value": 0.0 },
                  "interpolation": "linear"
                }
              ]
            }
          ]
        },
        {
          "id": "layer-2",
          "name": "Layer2",
          "visible": true,
          "locked": false,
          "objects": [
            {
              "id": "obj-text",
              "name": "テロップ",
              "kind": { "type": "text", "body": "こんにちは" },
              "start_frame": 30,
              "end_frame": 120,
              "params": [
                { "name": "size", "value": { "type": "number", "value": 40.0 } },
                { "name": "fill", "value": { "type": "color", "value": { "r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0 } } }
              ],
              "keyframes": []
            }
          ]
        }
      ]
    }
  ],
  "active_scene_id": "scene-root"
}"#;

#[test]
fn sample_json_loads_and_validates() {
    let project = Project::from_json(SAMPLE).unwrap();
    assert_eq!(project.name, "sample");
    assert_eq!(project.active_scene().unwrap().duration_frames(), 150);
}

#[test]
fn save_load_is_stable() {
    let project = Project::from_json(SAMPLE).unwrap();
    let json = project.to_json().unwrap();
    let reloaded = Project::from_json(&json).unwrap();
    assert_eq!(project, reloaded);
}

#[test]
fn overlapping_sample_is_rejected() {
    let mut project = Project::from_json(SAMPLE).unwrap();
    // Layer1 の obj-video(0-150) と重なるオブジェクトを追加
    project.scenes[0].layers[0].objects.push(TimelineObject {
        id: "obj-bad".to_string(),
        name: "bad".to_string(),
        kind: ObjectKind::Shape {
            shape: ShapeKind::Rectangle,
        },
        start_frame: 100,
        end_frame: 200,
        params: Vec::new(),
        keyframes: Vec::new(),
        loops: Default::default(),
    });
    assert!(project.validate().is_err());
}
