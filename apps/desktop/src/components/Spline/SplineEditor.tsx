import { useMemo, useRef, useState } from "react";
import { evalValue, sortedKeys } from "../../model/eval";
import { Action, KeySel } from "../../model/store";
import {
  EASE,
  EASE_IN,
  EASE_OUT,
  EASY_EASE,
  Frame,
  Interpolation,
  LoopMode,
  TimelineObject,
  Value,
} from "../../model/types";

interface Row {
  obj: TimelineObject;
  param: string;
}

interface Props {
  rows: Row[];
  playhead: Frame;
  keySel: KeySel[];
  dispatch: React.Dispatch<Action>;
}

const PARAM_COLORS = ["#6aa84f", "#4a86c8", "#c8a83a", "#c86a9a", "#9a6ac8", "#3aa6a6"];

const PRESETS: { label: string; interp: Interpolation }[] = [
  { label: "直線", interp: "linear" },
  { label: "停止", interp: "hold" },
  { label: "滑らか", interp: "smooth" },
  { label: "イーズ", interp: EASE },
  { label: "イン", interp: EASE_IN },
  { label: "アウト", interp: EASE_OUT },
  { label: "イージー", interp: EASY_EASE },
];

function numOf(v: Value): number | null {
  if (v.type === "number") return v.value;
  if (v.type === "integer") return v.value;
  return null;
}

const W = 800;
const H = 180;
const PAD = 8;

export default function SplineEditor({ rows, playhead, keySel, dispatch }: Props) {
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const svgRef = useRef<SVGSVGElement>(null);

  const visibleRows = useMemo(
    () => rows.filter((r) => !hidden.has(`${r.obj.id}:${r.param}`)),
    [rows, hidden],
  );

  const domain = useMemo(() => {
    let f0 = Infinity;
    let f1 = -Infinity;
    let v0 = Infinity;
    let v1 = -Infinity;
    for (const r of visibleRows) {
      for (const k of sortedKeys(r.obj, r.param)) {
        f0 = Math.min(f0, k.frame);
        f1 = Math.max(f1, k.frame);
        const v = numOf(k.value);
        if (v !== null) {
          v0 = Math.min(v0, v);
          v1 = Math.max(v1, v);
        }
      }
    }
    if (!isFinite(f0)) return null;
    if (f1 - f0 < 60) {
      const c = (f0 + f1) / 2;
      f0 = Math.max(0, Math.floor(c - 30));
      f1 = Math.ceil(c + 30);
    } else {
      f0 = Math.max(0, Math.floor(f0 - (f1 - f0) * 0.1));
      f1 = Math.ceil(f1 + (f1 - f0) * 0.1);
    }
    if (v1 - v0 < 1e-6) {
      v0 -= 1;
      v1 += 1;
    } else {
      const p = (v1 - v0) * 0.15;
      v0 -= p;
      v1 += p;
    }
    return { f0, f1, v0, v1 };
  }, [visibleRows]);

  const toSvg = (f: number, v: number) => {
    if (!domain) return { x: 0, y: 0 };
    return {
      x: PAD + ((f - domain.f0) / (domain.f1 - domain.f0)) * (W - PAD * 2),
      y: 10 + (1 - (v - domain.v0) / (domain.v1 - domain.v0)) * (H - 30),
    };
  };
  const fromSvg = (x: number, y: number) => {
    if (!domain) return { f: 0, v: 0 };
    return {
      f: domain.f0 + ((x - PAD) / (W - PAD * 2)) * (domain.f1 - domain.f0),
      v: domain.v0 + (1 - (y - 10) / (H - 30)) * (domain.v1 - domain.v0),
    };
  };

  const svgPoint = (e: React.PointerEvent) => {
    const rect = svgRef.current!.getBoundingClientRect();
    return {
      x: ((e.clientX - rect.left) / rect.width) * W,
      y: ((e.clientY - rect.top) / rect.height) * H,
    };
  };

  const isSel = (objId: string, param: string, frame: Frame) =>
    keySel.some((s) => s.objId === objId && s.param === param && s.frame === frame);

  const dragKey = (
    e: React.PointerEvent,
    obj: TimelineObject,
    param: string,
    frame: Frame,
  ) => {
    e.stopPropagation();
    const el = e.currentTarget as Element;
    el.setPointerCapture?.(e.pointerId);
    const key = obj.keyframes.find((k) => k.param === param && k.frame === frame);
    if (!key) return;
    if (!isSel(obj.id, param, frame)) {
      dispatch({ type: "selectKeys", keys: [{ objId: obj.id, param, frame }], additive: e.ctrlKey || e.metaKey });
    }
    let cur = frame;
    const move = (ev: PointerEvent) => {
      const r = svgRef.current!.getBoundingClientRect();
      const x = ((ev.clientX - r.left) / r.width) * W;
      const y = ((ev.clientY - r.top) / r.height) * H;
      const { f, v } = fromSvg(x, y);
      const nf = Math.max(0, Math.round(f));
      if (nf === cur) return;
      const nv =
        key.value.type === "number"
          ? { type: "number" as const, value: v }
          : key.value.type === "integer"
            ? { type: "integer" as const, value: Math.round(v) }
            : key.value;
      dispatch({ type: "removeKey", id: obj.id, param, frame: cur });
      dispatch({ type: "setKey", id: obj.id, param, frame: nf, value: nv, interpolation: key.interpolation });
      cur = nf;
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const dragHandle = (
    e: React.PointerEvent,
    obj: TimelineObject,
    param: string,
    frame: Frame,
    which: "p1" | "p2",
  ) => {
    e.stopPropagation();
    const keys = sortedKeys(obj, param);
    const i = keys.findIndex((k) => k.frame === frame);
    if (i < 0 || i + 1 >= keys.length) return;
    const k0 = keys[i];
    const k1 = keys[i + 1];
    if (typeof k0.interpolation !== "object" || !("bezier" in k0.interpolation)) return;
    const el = e.currentTarget as Element;
    el.setPointerCapture?.(e.pointerId);
    const v0 = numOf(k0.value);
    const v1 = numOf(k1.value);
    if (v0 === null || v1 === null) return;
    const dt = k1.frame - k0.frame;
    const dv = v1 - v0;
    const move = (ev: PointerEvent) => {
      const r = svgRef.current!.getBoundingClientRect();
      const x = ((ev.clientX - r.left) / r.width) * W;
      const y = ((ev.clientY - r.top) / r.height) * H;
      const { f, v } = fromSvg(x, y);
      let nx = (f - k0.frame) / Math.max(1, dt);
      let ny: number;
      if (Math.abs(dv) < 1e-9) ny = (toSvg(k0.frame, v0).y - y) / 50;
      else ny = (v - v0) / dv;
      nx = Math.min(1, Math.max(0, nx));
      const b = k0.interpolation as { bezier: { x1: number; y1: number; x2: number; y2: number } };
      const nb =
        which === "p1"
          ? { x1: nx, y1: ny, x2: b.bezier.x2, y2: b.bezier.y2 }
          : { x1: b.bezier.x1, y1: b.bezier.y1, x2: nx, y2: ny };
      dispatch({
        type: "setInterpolation",
        id: obj.id,
        param,
        frame: k0.frame,
        interpolation: { bezier: nb },
      });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const applyPreset = (interp: Interpolation) => {
    for (const s of keySel) {
      dispatch({ type: "setInterpolation", id: s.objId, param: s.param, frame: s.frame, interpolation: interp });
    }
  };

  if (rows.length === 0) {
    return (
      <div className="panel-body">
        <span className="stub">キーのあるオブジェクトを選択するとカーブを表示（数値パラメータのみ）</span>
      </div>
    );
  }

  return (
    <div className="spline">
      <div className="spline-tree">
        {rows.map((r) => {
          const id = `${r.obj.id}:${r.param}`;
          const mode = r.obj.loops?.[r.param] ?? "off";
          return (
            <div className="spline-row" key={id}>
              <input
                type="checkbox"
                checked={!hidden.has(id)}
                onChange={() =>
                  setHidden((prev) => {
                    const next = new Set(prev);
                    if (next.has(id)) next.delete(id);
                    else next.add(id);
                    return next;
                  })
                }
              />
              <span
                className="spline-dot"
                style={{ background: PARAM_COLORS[rows.indexOf(r) % PARAM_COLORS.length] }}
              />
              <span className="spline-name" title={r.obj.id}>
                {r.obj.name}/{r.param}
              </span>
              <select
                value={mode}
                onChange={(e) =>
                  dispatch({
                    type: "setLoop",
                    id: r.obj.id,
                    param: r.param,
                    mode: e.target.value as LoopMode,
                  })
                }
                title="範囲外の振る舞い"
              >
                <option value="off">―</option>
                <option value="loop">繰返</option>
                <option value="pingpong">往復</option>
              </select>
            </div>
          );
        })}
        <div className="spline-presets">
          {PRESETS.map((p) => (
            <button key={p.label} className="tab" onClick={() => applyPreset(p.interp)}>
              {p.label}
            </button>
          ))}
          <button className="tab" onClick={() => dispatch({ type: "copyKeys" })}>複写</button>
          <button
            className="tab"
            onClick={() => {
              const s = keySel[0] ?? (visibleRows[0] ? { objId: visibleRows[0].obj.id, param: visibleRows[0].param, frame: 0 } : null);
              if (s) dispatch({ type: "pasteKeys", objId: s.objId, param: s.param, at: playhead });
            }}
          >
            貼付
          </button>
        </div>
      </div>
      <div className="spline-graph">
        {domain && (
          <svg ref={svgRef} viewBox={`0 0 ${W} ${H}`} className="spline-svg"
            onPointerDown={(e) => {
              const p = svgPoint(e);
              dispatch({ type: "playhead", frame: Math.max(0, Math.round(fromSvg(p.x, p.y).f)) });
            }}
          >
            {[0.25, 0.5, 0.75].map((t) => (
              <line key={t} x1={PAD} x2={W - PAD} y1={10 + t * (H - 30)} y2={10 + t * (H - 30)} className="spline-grid" />
            ))}
            {visibleRows.map((r, ri) => {
              const color = PARAM_COLORS[ri % PARAM_COLORS.length];
              const keys = sortedKeys(r.obj, r.param);
              const v0 = keys.length > 0 ? numOf(keys[0].value) : null;
              if (v0 === null) return null;
              let d = "";
              for (let i = 0; i <= 120; i++) {
                const f = domain.f0 + ((domain.f1 - domain.f0) * i) / 120;
                const vv = numOf(evalValue(r.obj, r.param, Math.round(f), { type: "number", value: 0 }));
                if (vv === null) continue;
                const p = toSvg(f, vv);
                d += `${i === 0 ? "M" : "L"}${p.x.toFixed(1)},${p.y.toFixed(1)}`;
              }
              return <path key={`${r.obj.id}:${r.param}`} d={d} fill="none" stroke={color} strokeWidth={1.5} />;
            })}
            {visibleRows.map((r) =>
              sortedKeys(r.obj, r.param).map((k) => {
                const v = numOf(k.value);
                if (v === null) return null;
                const p = toSvg(k.frame, v);
                const sel = isSel(r.obj.id, r.param, k.frame);
                return (
                  <g key={`${r.obj.id}:${r.param}:${k.frame}`}>
                    <polygon
                      points={`${p.x},${p.y - 6} ${p.x + 5},${p.y} ${p.x},${p.y + 6} ${p.x - 5},${p.y}`}
                      className={`spline-key${sel ? " selected" : ""}`}
                      onPointerDown={(e) => dragKey(e, r.obj, r.param, k.frame)}
                    />
                    {(() => {
                      if (typeof k.interpolation !== "object" || !("bezier" in k.interpolation)) return null;
                      if (!sel) return null;
                      const idx = sortedKeys(r.obj, r.param).findIndex((x) => x.frame === k.frame);
                      const ks = sortedKeys(r.obj, r.param);
                      if (idx < 0 || idx + 1 >= ks.length) return null;
                      const k1 = ks[idx + 1];
                      const v1 = numOf(k1.value);
                      if (v1 === null) return null;
                      const dt = k1.frame - k.frame;
                      const dv = v1 - v;
                      const b = (k.interpolation as { bezier: { x1: number; y1: number; x2: number; y2: number } }).bezier;
                      const p1 = toSvg(k.frame + b.x1 * dt, v + b.y1 * (Math.abs(dv) < 1e-9 ? 50 : dv));
                      const p2 = toSvg(k.frame + b.x2 * dt, v + b.y2 * (Math.abs(dv) < 1e-9 ? 50 : dv));
                      return (
                        <g>
                          <line x1={p.x} y1={p.y} x2={p1.x} y2={p1.y} className="spline-handle" />
                          <line x1={p.x} y1={p.y} x2={p2.x} y2={p2.y} className="spline-handle" />
                          <circle cx={p1.x} cy={p1.y} r={4} className="spline-knob"
                            onPointerDown={(e) => dragHandle(e, r.obj, r.param, k.frame, "p1")} />
                          <circle cx={p2.x} cy={p2.y} r={4} className="spline-knob"
                            onPointerDown={(e) => dragHandle(e, r.obj, r.param, k.frame, "p2")} />
                        </g>
                      );
                    })()}
                  </g>
                );
              }),
            )}
            {(() => {
              const p = toSvg(playhead, (domain.v0 + domain.v1) / 2);
              return <line x1={p.x} x2={p.x} y1={0} y2={H} className="spline-playhead" />;
            })()}
          </svg>
        )}
      </div>
    </div>
  );
}
