import { Action } from "../model/store";
import { evalColor, evalNumber } from "../model/eval";
import { Frame, ObjectKind, Rgba, TimelineObject, Value } from "../model/types";

interface ParamDef {
  name: string;
  label: string;
  type: "number" | "integer" | "color";
  min?: number;
  max?: number;
  step?: number;
}

const COMMON: ParamDef[] = [
  { name: "x", label: "X", type: "number", step: 1 },
  { name: "y", label: "Y", type: "number", step: 1 },
  { name: "rotation", label: "回転°", type: "number", min: -720, max: 720, step: 1 },
  { name: "anchor_x", label: "基点X", type: "number", step: 1 },
  { name: "anchor_y", label: "基点Y", type: "number", step: 1 },
  { name: "scale", label: "拡大率", type: "number", min: 0.01, max: 10, step: 0.01 },
  { name: "opacity", label: "透明度", type: "number", min: 0, max: 1, step: 0.01 },
];

function schemaFor(kind: ObjectKind): ParamDef[] {
  switch (kind.type) {
    case "text":
      return [
        ...COMMON,
        { name: "size", label: "サイズ", type: "number", min: 1, max: 500, step: 1 },
        { name: "fill", label: "文字色", type: "color" },
      ];
    case "shape":
      return [
        ...COMMON,
        { name: "fill", label: "色", type: "color" },
        { name: "w", label: "幅", type: "number", min: 1, max: 4000, step: 1 },
        { name: "h", label: "高さ", type: "number", min: 1, max: 4000, step: 1 },
      ];
    case "video":
    case "image":
      return [
        ...COMMON,
        { name: "w", label: "幅", type: "number", min: 1, max: 8000, step: 1 },
        { name: "h", label: "高さ", type: "number", min: 1, max: 8000, step: 1 },
      ];
    case "audio":
      return [
        { name: "volume", label: "音量", type: "number", min: 0, max: 2, step: 0.05 },
        { name: "offset", label: "開始位置s", type: "number", min: 0, max: 3600, step: 0.1 },
      ];
    case "filter":
      return kind.effect === "blur"
        ? [{ name: "blur", label: "ぼかし", type: "number", min: 0, max: 100, step: 0.5 }]
        : [{ name: "brightness", label: "明るさ", type: "number", min: -1, max: 1, step: 0.01 }];
    case "duplicator":
      return [
        ...COMMON,
        { name: "cols", label: "列数", type: "integer", min: 1, max: 32, step: 1 },
        { name: "rows", label: "行数", type: "integer", min: 1, max: 32, step: 1 },
        { name: "spacing_x", label: "間隔X", type: "number", step: 1 },
        { name: "spacing_y", label: "間隔Y", type: "number", step: 1 },
        { name: "stagger", label: "時間差", type: "integer", min: 0, max: 300, step: 1 },
        { name: "wobble_amp", label: "揺れ幅", type: "number", min: 0, max: 500, step: 1 },
        { name: "wobble_speed", label: "揺れ速", type: "number", min: 0, max: 10, step: 0.1 },
      ];
    default:
      return COMMON;
  }
}

function rgbaToHex(c: Rgba): string {
  const h = (v: number) =>
    Math.round(Math.min(1, Math.max(0, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${h(c.r)}${h(c.g)}${h(c.b)}`;
}

function hexToRgba(hex: string, a: number): Rgba {
  const n = parseInt(hex.slice(1), 16);
  return { r: ((n >> 16) & 255) / 255, g: ((n >> 8) & 255) / 255, b: (n & 255) / 255, a };
}

export default function Inspector({
  object,
  extraSelected,
  playhead,
  dispatch,
}: {
  object: TimelineObject | null;
  extraSelected: number;
  playhead: Frame;
  dispatch: React.Dispatch<Action>;
}) {
  if (!object) {
    return (
      <section className="panel">
        <header className="panel-title"><span>オブジェクト設定</span></header>
        <div className="panel-body"><span className="stub">未選択</span></div>
      </section>
    );
  }
  const defs = schemaFor(object.kind);
  return (
    <section className="panel">
      <header className="panel-title">
        <span>オブジェクト設定：{object.name}{extraSelected > 0 ? `（他${extraSelected}件）` : ""}</span>
      </header>
      <div className="panel-body">
        {object.kind.type === "text" && (
          <div className="insp-row">
            <span className="insp-label">本文</span>
            <input
              className="insp-text"
              value={object.kind.body}
              onChange={(e) => dispatch({ type: "setTextBody", id: object.id, body: e.target.value })}
            />
          </div>
        )}
        {defs.map((def) => {
          const hasKeys = object.keyframes.some((k) => k.param === def.name);
          const keyHere = object.keyframes.some((k) => k.param === def.name && k.frame === playhead);
          const set = (value: Value) =>
            dispatch({ type: "setParam", id: object.id, name: def.name, value });
          return (
            <div className="insp-row" key={def.name}>
              <button
                className={`kf${hasKeys ? " on" : ""}${keyHere ? " here" : ""}`}
                title={keyHere ? "このフレームのキーを削除" : "このフレームにキーを打つ"}
                onClick={() => {
                  if (keyHere) {
                    dispatch({ type: "removeKey", id: object.id, param: def.name, frame: playhead });
                  } else {
                    const cur =
                      def.type === "color"
                        ? { type: "color" as const, value: evalColor(object, def.name, playhead, { r: 1, g: 1, b: 1, a: 1 }) }
                        : def.type === "integer"
                          ? { type: "integer" as const, value: Math.round(evalNumber(object, def.name, playhead, 0)) }
                          : { type: "number" as const, value: evalNumber(object, def.name, playhead, 0) };
                    dispatch({ type: "setKey", id: object.id, param: def.name, frame: playhead, value: cur, interpolation: "linear" });
                  }
                }}
              >
                ◆
              </button>
              <span className="insp-label">{def.label}</span>
              {def.type === "color" ? (
                <input
                  type="color"
                  value={rgbaToHex(evalColor(object, def.name, playhead, { r: 1, g: 1, b: 1, a: 1 }))}
                  onChange={(e) => {
                    const cur = evalColor(object, def.name, playhead, { r: 1, g: 1, b: 1, a: 1 });
                    set({ type: "color", value: hexToRgba(e.target.value, cur.a) });
                  }}
                />
              ) : def.type === "integer" ? (
                <input
                  type="number"
                  className="insp-num"
                  min={def.min}
                  max={def.max}
                  step={def.step}
                  value={Math.round(evalNumber(object, def.name, playhead, 0))}
                  onChange={(e) => set({ type: "integer", value: Math.round(Number(e.target.value)) })}
                />
              ) : (
                <>
                  <input
                    type="range"
                    className="insp-range"
                    min={def.min}
                    max={def.max}
                    step={def.step ?? 1}
                    value={evalNumber(object, def.name, playhead, 0)}
                    onChange={(e) => set({ type: "number", value: Number(e.target.value) })}
                  />
                  <input
                    type="number"
                    className="insp-num"
                    step={def.step ?? 1}
                    value={Math.round(evalNumber(object, def.name, playhead, 0) * 100) / 100}
                    onChange={(e) => set({ type: "number", value: Number(e.target.value) })}
                  />
                </>
              )}
            </div>
          );
        })}
        <div className="dim" style={{ marginTop: 8 }}>
          ◆＝キーあり（発光＝現フレームにキー）。値の編集はキーありなら打鍵、なければ基本値。
        </div>
      </div>
    </section>
  );
}
