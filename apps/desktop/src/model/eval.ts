// Rust timeline-model の評価と同一アルゴリズム（要：挙動一致）。
import { Frame, Interpolation, Keyframe, Rgba, TimelineObject, Value } from "./types";

export function cubicBezier(t: number, x1: number, y1: number, x2: number, y2: number): number {
  t = Math.min(1, Math.max(0, t));
  let lo = 0;
  let hi = 1;
  let s = t;
  for (let i = 0; i < 12; i++) {
    s = (lo + hi) / 2;
    const x = 3 * (1 - s) * (1 - s) * s * x1 + 3 * (1 - s) * s * s * x2 + s * s * s;
    if (x < t) lo = s;
    else hi = s;
  }
  const y = 3 * (1 - s) * (1 - s) * s * y1 + 3 * (1 - s) * s * s * y2 + s * s * s;
  return Math.min(1, Math.max(0, y));
}

export function lerpValue(a: Value, b: Value, t: number): Value {
  if (a.type === "number" && b.type === "number") {
    return { type: "number", value: a.value + (b.value - a.value) * t };
  }
  if (a.type === "integer" && b.type === "integer") {
    return { type: "integer", value: Math.round(a.value + (b.value - a.value) * t) };
  }
  if (a.type === "color" && b.type === "color") {
    const l = (x: number, y: number) => x + (y - x) * t;
    return {
      type: "color",
      value: {
        r: l(a.value.r, b.value.r),
        g: l(a.value.g, b.value.g),
        b: l(a.value.b, b.value.b),
        a: l(a.value.a, b.value.a),
      },
    };
  }
  if (a.type === "point" && b.type === "point") {
    return {
      type: "point",
      value: [a.value[0] + (b.value[0] - a.value[0]) * t, a.value[1] + (b.value[1] - a.value[1]) * t],
    };
  }
  return a;
}

function remEuclid(a: number, n: number): number {
  return ((a % n) + n) % n;
}

export function sortedKeys(obj: TimelineObject, param: string): Keyframe[] {
  return obj.keyframes
    .filter((k) => k.param === param)
    .sort((a, b) => a.frame - b.frame);
}

function mapFrame(
  mode: string | undefined,
  frame: Frame,
  first: Frame,
  last: Frame,
): Frame {
  const period = last - first;
  if (period <= 0) return first;
  if (mode === "loop") return first + remEuclid(frame - first, period);
  if (mode === "pingpong") {
    const m = remEuclid(frame - first, period * 2);
    return m <= period ? first + m : last - (m - period);
  }
  return Math.min(last, Math.max(first, frame));
}

export function evalValue(obj: TimelineObject, param: string, frame: Frame, def: Value): Value {
  const base = obj.params.find((p) => p.name === param)?.value;
  const keys = sortedKeys(obj, param);
  if (keys.length === 0) return base ?? def;
  const first = keys[0].frame;
  const last = keys[keys.length - 1].frame;
  const f = mapFrame(obj.loops?.[param], frame, first, last);
  let idx = 0;
  while (idx + 1 < keys.length && keys[idx + 1].frame <= f) idx++;
  if (idx + 1 >= keys.length) return keys[idx].value;
  const k0 = keys[idx];
  const k1 = keys[idx + 1];
  if (k1.frame <= k0.frame) return k1.value;
  const t = (f - k0.frame) / (k1.frame - k0.frame);
  const interp: Interpolation = k0.interpolation;
  if (interp === "hold") return k0.value;
  let e: number;
  if (interp === "linear") e = t;
  else if (interp === "smooth") e = t * t * (3 - 2 * t);
  else e = cubicBezier(t, interp.bezier.x1, interp.bezier.y1, interp.bezier.x2, interp.bezier.y2);
  return lerpValue(k0.value, k1.value, Math.min(1, Math.max(0, e)));
}

export function evalNumber(obj: TimelineObject, param: string, frame: Frame, def: number): number {
  const v = evalValue(obj, param, frame, { type: "number", value: def });
  if (v.type === "number") return v.value;
  if (v.type === "integer") return v.value;
  return def;
}

export function evalColor(obj: TimelineObject, param: string, frame: Frame, def: Rgba): Rgba {
  const v = evalValue(obj, param, frame, { type: "color", value: def });
  return v.type === "color" ? v.value : def;
}
