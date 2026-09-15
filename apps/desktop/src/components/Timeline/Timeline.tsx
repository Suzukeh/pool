import { useMemo, useRef, useState } from "react";
import { sortedKeys } from "../../model/eval";
import { Action, KeySel } from "../../model/store";
import { Frame, Scene, TimelineObject } from "../../model/types";
import SplineEditor from "../Spline/SplineEditor";

interface Props {
  scene: Scene;
  selection: string[];
  playhead: Frame;
  fps: number;
  keySel: KeySel[];
  dispatch: React.Dispatch<Action>;
}

interface MenuState {
  x: number;
  y: number;
  frame: Frame;
  layerId: string;
  objectId?: string;
}

const KIND_COLORS: Record<string, string> = {
  video: "#4a86c8",
  image: "#3aa6a6",
  audio: "#5aa64f",
  text: "#c8a83a",
  shape: "#9a6ac8",
  duplicator: "#c87a3a",
  filter: "#6a6a72",
  group_control: "#c86a9a",
  camera_control: "#c86a9a",
};

function kindLabel(o: TimelineObject): string {
  const k = o.kind;
  switch (k.type) {
    case "text":
      return o.name || k.body.slice(0, 12);
    case "video":
    case "image":
    case "audio":
      return o.name || k.path.split("/").pop() || k.type;
    case "shape":
      return o.name || (k.shape === "rectangle" ? "矩形" : "楕円");
    case "filter":
      return o.name || k.effect;
    default:
      return o.name || k.type;
  }
}

export function frameToTC(frame: Frame, fps: number): string {
  const f = Math.max(0, Math.round(frame));
  const ff = f % fps;
  const s = Math.floor(f / fps) % 60;
  const m = Math.floor(f / (fps * 60)) % 60;
  const h = Math.floor(f / (fps * 3600));
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(h)}:${pad(m)}:${pad(s)}:${pad(ff)}`;
}

export default function Timeline({ scene, selection, playhead, fps, keySel, dispatch }: Props) {
  const [ppf, setPpf] = useState(4);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [tab, setTab] = useState<"シーン" | "グラフ" | "コード">("シーン");
  const drag = useRef<{
    id: string;
    mode: "move" | "start" | "end";
    startX: number;
    lastDelta: number;
  } | null>(null);

  const selected = useMemo(() => new Set(selection), [selection]);
  const duration = Math.max(
    300,
    ...scene.layers.flatMap((l) => l.objects.map((o) => o.end_frame)),
  );
  const width = duration * ppf;

  const step = useMemo(() => {
    const target = 80 / ppf;
    const steps = [1, 5, 10, 30, 60, 150, 300, 600, 1800];
    return steps.find((s) => s >= target) ?? 3600;
  }, [ppf]);

  const ticks = [];
  for (let f = 0; f <= duration; f += step) ticks.push(f);

  const onBarDown = (e: React.PointerEvent, o: TimelineObject) => {
    if (e.button === 2) return;
    e.stopPropagation();
    dispatch({ type: "select", ids: [o.id], additive: e.ctrlKey || e.metaKey });
    const rect = (e.target as HTMLElement).closest(".tl-bar")?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const w = rect.width;
    const mode = x < 8 ? "start" : x > w - 8 ? "end" : "move";
    drag.current = { id: o.id, mode, startX: e.clientX, lastDelta: 0 };
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture?.(e.pointerId);
    el.style.cursor = mode === "move" ? "grabbing" : "ew-resize";
  };

  const onBarMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const total = Math.round((e.clientX - d.startX) / ppf);
    const inc = total - d.lastDelta;
    if (inc === 0) return;
    d.lastDelta = total;
    if (d.mode === "move") {
      dispatch({ type: "move", id: d.id, delta: inc });
    } else {
      // 増分トリム（reducer が範囲外・重なりを棄却する）
      dispatch({ type: "nudgeEdge", id: d.id, edge: d.mode, delta: inc });
    }
  };

  const onBarUp = (e: React.PointerEvent) => {
    drag.current = null;
    (e.currentTarget as HTMLElement).style.cursor = "";
  };

  const openMenu = (e: React.MouseEvent, layerId: string, frame: Frame, objectId?: string) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, frame, layerId, objectId });
  };

  const closeMenu = () => setMenu(null);

  const addKind = (
    layerId: string,
    frame: Frame,
    kind: TimelineObject["kind"],
    name: string,
  ) => {
    dispatch({ type: "add", layerId, kind, name, at: frame });
    closeMenu();
  };

  const splineRows = useMemo(() => {
    const out: { obj: TimelineObject; param: string }[] = [];
    for (const layer of scene.layers) {
      for (const o of layer.objects) {
        if (!selection.includes(o.id)) continue;
        const params = [...new Set(o.keyframes.map((k) => k.param))];
        for (const p of params) {
          const ks = sortedKeys(o, p);
          const v = ks.length > 0 ? ks[0].value : null;
          if (v && (v.type === "number" || v.type === "integer")) out.push({ obj: o, param: p });
        }
      }
    }
    return out;
  }, [scene, selection]);

  return (
    <section className="panel timeline" onPointerDown={closeMenu}>
      <header className="panel-title">
        <span>レイヤー編集</span>
        <span className="tabs">
          {(["シーン", "グラフ", "コード"] as const).map((t) => (
            <button key={t} className={t === tab ? "tab active" : "tab"} onClick={() => setTab(t)}>
              {t}
            </button>
          ))}
          <button className="tab" onClick={() => setPpf((v) => Math.max(1, v / 2))}>−</button>
          <button className="tab" onClick={() => setPpf((v) => Math.min(32, v * 2))}>＋</button>
        </span>
      </header>
      {tab === "グラフ" ? (
        <div className="panel-body">
          <SplineEditor rows={splineRows} playhead={playhead} keySel={keySel} dispatch={dispatch} />
        </div>
      ) : tab === "コード" ? (
        <div className="panel-body">
          <span className="stub">コードエディタは M4（プラグイン）で実装</span>
        </div>
      ) : (
      <div
        className="panel-body tl-scroll"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Delete" || e.key === "Backspace") {
            dispatch({ type: "delete", ids: selection });
          } else if (e.key === "s" || e.key === "S") {
            for (const id of selection) dispatch({ type: "split", id, at: playhead });
          } else if ((e.ctrlKey || e.metaKey) && (e.key === "d" || e.key === "D")) {
            e.preventDefault();
            dispatch({ type: "duplicate", ids: selection });
          }
        }}
      >
        <div className="tl-ruler-row">
          <div className="tl-rowhead">＋</div>
          <div
            className="tl-ruler"
            style={{ width }}
            onPointerDown={(e) => {
              const rect = e.currentTarget.getBoundingClientRect();
              const frame = Math.max(0, Math.round((e.clientX - rect.left) / ppf));
              dispatch({ type: "playhead", frame });
              const move = (ev: PointerEvent) => {
                const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                dispatch({
                  type: "playhead",
                  frame: Math.max(0, Math.round((ev.clientX - r.left) / ppf)),
                });
              };
              const up = () => {
                window.removeEventListener("pointermove", move);
                window.removeEventListener("pointerup", up);
              };
              window.addEventListener("pointermove", move);
              window.addEventListener("pointerup", up);
            }}
          >
            {ticks.map((f) => (
              <span key={f} className="tl-tick" style={{ left: f * ppf }}>
                {frameToTC(f, fps)}
              </span>
            ))}
            <div className="tl-playhead" style={{ left: playhead * ppf }} />
          </div>
        </div>
        {scene.layers.map((layer) => (
          <div className="tl-row" key={layer.id}>
            <div className="tl-rowhead" title={layer.id}>
              {layer.name}
              {layer.locked ? " 🔒" : ""}
            </div>
            <div
              className="tl-lane"
              style={{ width }}
              onContextMenu={(e) => {
                const rect = e.currentTarget.getBoundingClientRect();
                openMenu(
                  e,
                  layer.id,
                  Math.max(0, Math.round((e.clientX - rect.left) / ppf)),
                );
              }}
            >
              {layer.objects.map((o) => (
                <div
                  key={o.id}
                  className={`tl-bar${selected.has(o.id) ? " selected" : ""}`}
                  style={{
                    left: o.start_frame * ppf,
                    width: Math.max(4, (o.end_frame - o.start_frame) * ppf),
                    background: KIND_COLORS[o.kind.type] ?? "#666",
                  }}
                  title={`${o.name} [${o.start_frame}–${o.end_frame}]`}
                  onPointerDown={(e) => onBarDown(e, o)}
                  onPointerMove={onBarMove}
                  onPointerUp={onBarUp}
                  onContextMenu={(e) => {
                    const lane = (e.target as HTMLElement).closest(".tl-lane");
                    const rect = lane?.getBoundingClientRect();
                    const frame = rect
                      ? Math.max(0, Math.round((e.clientX - rect.left) / ppf))
                      : o.start_frame;
                    openMenu(e, layer.id, frame, o.id);
                  }}
                >
                  <span className="tl-bar-label">{kindLabel(o)}</span>
                </div>
              ))}
              <div className="tl-playhead" style={{ left: playhead * ppf }} />
            </div>
          </div>
        ))}
      </div>
      )}
      {menu && (
        <div className="ctxmenu" style={{ left: menu.x, top: menu.y }}>
          {menu.objectId ? (
            <>
              <button
                onClick={() => {
                  dispatch({ type: "split", id: menu.objectId!, at: playhead });
                  closeMenu();
                }}
              >
                分割 (S)
              </button>
              <button
                onClick={() => {
                  dispatch({ type: "duplicate", ids: [menu.objectId!] });
                  closeMenu();
                }}
              >
                複製 (Ctrl+D)
              </button>
              <button
                onClick={() => {
                  const ids = selection.includes(menu.objectId!)
                    ? selection
                    : [menu.objectId!];
                  dispatch({ type: "align", ids, mode: "start" });
                  closeMenu();
                }}
              >
                左揃え
              </button>
              <button
                onClick={() => {
                  const ids = selection.includes(menu.objectId!)
                    ? selection
                    : [menu.objectId!];
                  dispatch({ type: "align", ids, mode: "end" });
                  closeMenu();
                }}
              >
                右揃え
              </button>
              <button
                onClick={() => {
                  dispatch({ type: "delete", ids: [menu.objectId!] });
                  closeMenu();
                }}
              >
                削除 (Del)
              </button>
            </>
          ) : (
            <>
              <button
                onClick={() =>
                  addKind(menu.layerId, menu.frame, { type: "text", body: "テキスト" }, "テキスト")
                }
              >
                テキスト追加
              </button>
              <button
                onClick={() =>
                  addKind(menu.layerId, menu.frame, { type: "shape", shape: "rectangle" }, "矩形")
                }
              >
                矩形追加
              </button>
              <button
                onClick={() =>
                  addKind(menu.layerId, menu.frame, { type: "shape", shape: "ellipse" }, "楕円")
                }
              >
                楕円追加
              </button>
            </>
          )}
        </div>
      )}
    </section>
  );
}
