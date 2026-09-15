# timeline-model 仕様（v1）

実装：`crates/timeline-model`。UI・GPU 非依存。

## モデル

- `Project { version, name, scenes, active_scene_id }`
  - `version` は本仕様版（現行 `1`）。不一致は読込拒否。
- `Scene { id, name, width, height, fps_num/fps_den, sample_rate, bg_color, layers }`
  - AE の Comp 相当。時刻の単位はフレーム（`i64`）。
- `Layer { id, name, visible, locked, objects }`
  - `objects[0]` が最上位、**下に行くほど手前**（AviUtl2 踏襲）。
  - **同一レイヤー内のオブジェクトは時間重なり禁止**（検証で拒否）。
- `TimelineObject { id, name, kind, start_frame, end_frame, params, keyframes }`
  - 区間は `[start_frame, end_frame)`、`start < end` 必須。
  - `kind` は `video/image/audio/text/shape/duplicator/filter/group_control/camera_control`。
- `Keyframe { param, frame, value, interpolation }`
  - `interpolation`: `hold / linear / smooth`（ベジェ詳細は M3）。

## 永続化

- `Project::to_json`（pretty）/ `from_json`。適合テストは
  `crates/timeline-model/tests/project_roundtrip.rs`。
