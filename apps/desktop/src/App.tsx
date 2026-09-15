import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import Inspector from "./components/Inspector";
import MediaExplorer from "./components/MediaExplorer";
import ObjectList from "./components/ObjectList";
import Panel from "./components/Panel";
import Preview from "./components/Preview";
import SceneList from "./components/SceneList";
import Timeline, { frameToTC } from "./components/Timeline/Timeline";
import { bootProject, getVersion, persistProject } from "./model/api";
import { Action, EditorState, reducer } from "./model/store";
import { activeScene } from "./model/types";

const initial: EditorState = { project: null, selection: [], playhead: 0, keySel: [], keyClipboard: null };

// 選択・再生ヘッド以外の変更は保存対象
const MUTATING = new Set([
  "move",
  "trim",
  "nudgeEdge",
  "split",
  "delete",
  "duplicate",
  "add",
  "align",
  "addScene",
  "setParam",
  "setKey",
  "removeKey",
  "setInterpolation",
  "setLoop",
  "pasteKeys",
]);

export default function App() {
  const [state, baseDispatch] = useReducer(reducer, initial);
  const [version, setVersion] = useState("…");
  const [saveMsg, setSaveMsg] = useState("起動中…");
  const [playing, setPlaying] = useState(false);
  const [exporting, setExporting] = useState<string | null>(null);
  const stateRef = useRef(state);
  stateRef.current = state;

  const dispatch = useCallback((action: Action) => {
    baseDispatch(action);
    if (MUTATING.has(action.type)) {
      setSaveMsg((m) => (m.startsWith("保存済み") ? "● 未保存の変更あり" : m));
    }
  }, []);

  useEffect(() => {
    getVersion().then(setVersion);
    bootProject("untitled")
      .then((project) => {
        baseDispatch({ type: "load", project });
        setSaveMsg("読込済み");
      })
      .catch((e) => setSaveMsg(`起動失敗: ${String(e)}`));
  }, []);

  const doSave = useCallback(async () => {
    const project = stateRef.current.project;
    if (!project) return;
    setSaveMsg("保存中…");
    try {
      const where = await persistProject(project);
      setSaveMsg(`保存済み ${new Date().toLocaleTimeString()} (${where})`);
    } catch (e) {
      setSaveMsg(`保存失敗: ${String(e)}`);
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<{ frame: number; total: number }>("export-progress", (e) => {
      setExporting(`${e.payload.frame}/${e.payload.total}f`);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  const doExport = useCallback(async (format: "mp4" | "pngseq") => {
    const project = stateRef.current.project;
    const scene = project ? activeScene(project) : undefined;
    if (!project || !scene) return;
    try {
      const { save, open } = await import("@tauri-apps/plugin-dialog");
      const { invoke } = await import("@tauri-apps/api/core");
      const prefs =
        (await invoke<{ last_export_dir?: string }>("get_prefs").catch(() => null)) ?? {};
      const scale = Math.min(1, 1280 / scene.width);
      const width = Math.max(16, Math.round((scene.width * scale) / 2) * 2);
      const height = Math.max(16, Math.round((scene.height * scale) / 2) * 2);
      let out: string | string[] | null = null;
      if (format === "mp4") {
        out = await save({
          defaultPath: prefs.last_export_dir,
          filters: [{ name: "MP4", extensions: ["mp4"] }],
        });
      } else {
        out = await open({ directory: true, multiple: false, defaultPath: prefs.last_export_dir });
      }
      if (!out || Array.isArray(out)) return;
      setExporting("開始…");
      setSaveMsg("書出し中…");
      const result = await invoke<string>("export_movie", {
        json: JSON.stringify(project),
        outPath: out,
        width,
        height,
        format,
      });
      const dir = out.includes("/") ? out.slice(0, out.lastIndexOf("/")) : out;
      await invoke("set_prefs", { prefs: { ...prefs, last_export_dir: dir } }).catch(() => {});
      setSaveMsg(`書出し完了: ${result}`);
    } catch (e) {
      setSaveMsg(`書出し失敗: ${String(e)}`);
    } finally {
      setExporting(null);
    }
  }, []);

  const scene = state.project ? activeScene(state.project) : undefined;
  const fps = scene ? scene.fps_num / scene.fps_den : 30;
  const duration = scene
    ? Math.max(1, ...scene.layers.flatMap((l) => l.objects.map((o) => o.end_frame)), 1)
    : 1;

  useEffect(() => {
    if (!playing) return;
    const id = window.setInterval(() => {
      const st = stateRef.current;
      const next = st.playhead + 1;
      baseDispatch({ type: "playhead", frame: next >= duration ? 0 : next });
    }, 1000 / fps);
    return () => window.clearInterval(id);
  }, [playing, fps, duration]);

  const firstEditableLayer = scene?.layers.find((l) => l.visible && !l.locked)?.id;
  const selectedObject =
    scene?.layers.flatMap((l) => l.objects).find((o) => state.selection.includes(o.id)) ?? null;

  return (
    <div
      className="app"
      onKeyDown={(e) => {
        const tag = (e.target as HTMLElement).tagName;
        if (tag === "INPUT" || tag === "TEXTAREA") return;
        if (e.code === "Space") {
          e.preventDefault();
          setPlaying((p) => !p);
        } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
          const d = (e.key === "ArrowRight" ? 1 : -1) * (e.shiftKey ? 10 : 1);
          baseDispatch({ type: "playhead", frame: stateRef.current.playhead + d });
        } else if ((e.ctrlKey || e.metaKey) && (e.key === "s" || e.key === "S")) {
          e.preventDefault();
          void doSave();
        }
      }}
    >
      <div className="toolbar">
        <span className="logo">pool</span>
        <span className="menu">ファイル 編集 表示 設定 その他</span>
        <button className="tab" onClick={() => void doSave()}>
          💾 保存
        </button>
        <button className="tab" onClick={() => void doExport("mp4")}>
          🎬MP4
        </button>
        <button className="tab" onClick={() => void doExport("pngseq")}>
          🖼連番
        </button>
        {exporting && <span className="dim">書出し {exporting}</span>}
        <span className="dim">{saveMsg}</span>
      </div>
      <div className="left">
        {state.project && <SceneList project={state.project} dispatch={dispatch} />}
        <MediaExplorer
          layerId={firstEditableLayer}
          at={state.playhead}
          dispatch={dispatch}
          onStatus={setSaveMsg}
        />
      </div>
      <div className="center">
        {scene && state.project ? (
          <Preview
            project={state.project}
            scene={scene}
            playhead={state.playhead}
            fps={fps}
            playing={playing}
            onStep={(d) => baseDispatch({ type: "playhead", frame: state.playhead + d })}
            onTogglePlay={() => setPlaying((p) => !p)}
          />
        ) : (
          <Panel title="プレビュー編集" />
        )}
      </div>
      <div className="right">
        {scene && (
          <ObjectList
            scene={scene}
            playhead={state.playhead}
            selection={state.selection}
            dispatch={dispatch}
          />
        )}
        <Inspector
          object={selectedObject}
          extraSelected={Math.max(0, state.selection.length - (selectedObject ? 1 : 0))}
          playhead={state.playhead}
          dispatch={dispatch}
        />
      </div>
      <div className="bottom">
        {scene ? (
          <Timeline
            scene={scene}
            selection={state.selection}
            playhead={state.playhead}
            fps={fps}
            keySel={state.keySel}
            dispatch={dispatch}
          />
        ) : (
          <Panel title="レイヤー編集" />
        )}
      </div>
      <footer className="statusbar">
        <span>
          {scene ? `${scene.name} | ${scene.width}x${scene.height} | ${fps}fps` : "…"} | f
          {state.playhead}/{duration}（{scene ? frameToTC(state.playhead, fps) : "…"}）
        </span>
        <span>v{version}</span>
      </footer>
    </div>
  );
}
