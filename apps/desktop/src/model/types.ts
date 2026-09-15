// Rust `pool-timeline-model` の JSON スキーマと 1:1（キーは snake_case のまま）。
// 変更時は docs/spec/timeline-model.md と両方更新すること。

export type Frame = number;

export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

export type Value =
  | { type: "number"; value: number }
  | { type: "integer"; value: number }
  | { type: "boolean"; value: boolean }
  | { type: "text"; value: string }
  | { type: "color"; value: Rgba }
  | { type: "point"; value: [number, number] };

export type Interpolation =
  | "hold"
  | "linear"
  | "smooth"
  | { bezier: { x1: number; y1: number; x2: number; y2: number } };

export const EASY_EASE: Interpolation = { bezier: { x1: 0.42, y1: 0, x2: 0.58, y2: 1 } };
export const EASE: Interpolation = { bezier: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1 } };
export const EASE_IN: Interpolation = { bezier: { x1: 0.42, y1: 0, x2: 1, y2: 1 } };
export const EASE_OUT: Interpolation = { bezier: { x1: 0, y1: 0, x2: 0.58, y2: 1 } };

export type LoopMode = "off" | "loop" | "pingpong";

export interface Keyframe {
  param: string;
  frame: Frame;
  value: Value;
  interpolation: Interpolation;
}

export interface Param {
  name: string;
  value: Value;
}

export type ShapeKind = "rectangle" | "ellipse";

export type ObjectKind =
  | { type: "video"; path: string }
  | { type: "image"; path: string }
  | { type: "audio"; path: string }
  | { type: "text"; body: string }
  | { type: "shape"; shape: ShapeKind }
  | { type: "duplicator"; source: string }
  | { type: "filter"; effect: string }
  | { type: "group_control"; target_layers: number }
  | { type: "camera_control"; target_layers: number };

export interface TimelineObject {
  id: string;
  name: string;
  kind: ObjectKind;
  start_frame: Frame;
  end_frame: Frame;
  params: Param[];
  keyframes: Keyframe[];
  loops?: Record<string, LoopMode>;
}

export interface Layer {
  id: string;
  name: string;
  visible: boolean;
  locked?: boolean;
  objects: TimelineObject[];
}

export interface Scene {
  id: string;
  name: string;
  width: number;
  height: number;
  fps_num: number;
  fps_den: number;
  sample_rate: number;
  bg_color: Rgba;
  layers: Layer[];
}

export interface Project {
  version: number;
  name: string;
  scenes: Scene[];
  active_scene_id: string;
}

export const PROJECT_VERSION = 1;

export function activeScene(p: Project): Scene | undefined {
  return p.scenes.find((s) => s.id === p.active_scene_id);
}

/// 同一レイヤー内の時間重なりがあれば true。
export function hasOverlap(objects: TimelineObject[]): boolean {
  const spans = [...objects].sort((a, b) => a.start_frame - b.start_frame);
  for (let i = 0; i < spans.length; i++) {
    if (spans[i].end_frame <= spans[i].start_frame) return true;
    if (i > 0 && spans[i].start_frame < spans[i - 1].end_frame) return true;
  }
  return false;
}
