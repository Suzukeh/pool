import { Project } from "./types";

/** ブラウザ開発用フォールバック（Rust Project::new と同形）。 */
export function fallbackProject(name: string): Project {
  return {
    version: 1,
    name,
    scenes: [
      {
        id: "scene-root",
        name: "Root",
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        sample_rate: 44100,
        bg_color: { r: 0, g: 0, b: 0, a: 1 },
        layers: [{ id: "layer-1", name: "Layer1", visible: true, locked: false, objects: [] }],
      },
    ],
    active_scene_id: "scene-root",
  };
}
