import { useEffect, useState } from "react";
import { Action } from "../model/store";
import { Frame, ObjectKind } from "../model/types";

const STUBS: { label: string; kind: ObjectKind; name: string }[] = [
  { label: "🔤 テキスト", kind: { type: "text", body: "テキスト" }, name: "テキスト" },
  { label: "⬛ 矩形", kind: { type: "shape", shape: "rectangle" }, name: "矩形" },
  { label: "⬤ 楕円", kind: { type: "shape", shape: "ellipse" }, name: "楕円" },
];

interface Imported {
  path: string;
  thumb: string;
  width: number;
  height: number;
}

interface PluginEntry {
  name: string;
  version: string;
  description: string;
}

async function tauri<T>(fn: () => Promise<T>): Promise<T | null> {
  try {
    return await fn();
  } catch {
    return null;
  }
}

export default function MediaExplorer({
  layerId,
  at,
  dispatch,
  onStatus,
}: {
  layerId: string | undefined;
  at: Frame;
  dispatch: React.Dispatch<Action>;
  onStatus: (msg: string) => void;
}) {
  const [imports, setImports] = useState<Imported[]>([]);
  const [plugins, setPlugins] = useState<PluginEntry[]>([]);
  const [lastDir, setLastDir] = useState<string | undefined>(undefined);

  useEffect(() => {
    void (async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const list = await tauri(() => invoke<PluginEntry[]>("list_plugins"));
      if (list) setPlugins(list);
      const prefs = await tauri(() =>
        invoke<{ last_import_dir?: string }>("get_prefs"),
      );
      if (prefs?.last_import_dir) setLastDir(prefs.last_import_dir);
    })();
  }, []);

  const addObject = (kind: ObjectKind, name: string) => {
    if (layerId) dispatch({ type: "add", layerId, kind, name, at });
  };

  const importMedia = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const { invoke } = await import("@tauri-apps/api/core");
    const picked = await tauri(() =>
      open({
        multiple: true,
        directory: false,
        defaultPath: lastDir,
        filters: [{ name: "動画・画像", extensions: ["mp4", "mov", "mkv", "webm", "png", "jpg", "webp"] }],
      }),
    );
    if (!picked) return;
    const paths = (Array.isArray(picked) ? picked : [picked]) as string[];
    for (const path of paths) {
      const item = await tauri(() =>
        invoke<{ path: string; thumb: string; info: { width: number; height: number } }>(
          "import_media",
          { path },
        ),
      );
      if (item) {
        setImports((prev) => [
          ...prev,
          { path: item.path, thumb: item.thumb, width: item.info.width, height: item.info.height },
        ]);
        const dir = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : undefined;
        if (dir) {
          setLastDir(dir);
          const prefs = (await tauri(() => invoke<{ last_import_dir?: string }>("get_prefs"))) ?? {};
          await tauri(() => invoke("set_prefs", { prefs: { ...prefs, last_import_dir: dir } }));
        }
      }
    }
    onStatus(`取込: ${paths.length}件`);
  };

  const installPlugin = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const { invoke } = await import("@tauri-apps/api/core");
    const picked = await tauri(() =>
      open({ multiple: false, directory: false, filters: [{ name: "ZIP", extensions: ["zip"] }] }),
    );
    if (!picked || Array.isArray(picked)) return;
    const m = await tauri(() => invoke<PluginEntry>("install_plugin_zip", { zipPath: picked }));
    if (m) {
      const list = await tauri(() => invoke<PluginEntry[]>("list_plugins"));
      if (list) setPlugins(list);
      onStatus(`導入: ${m.name} v${m.version}`);
    } else {
      onStatus("導入に失敗（Tauri外では不可）");
    }
  };

  return (
    <section className="panel">
      <header className="panel-title">
        <span>メディアエクスプローラー</span>
        <span className="tabs">
          <button className="tab" onClick={() => void importMedia()}>取込</button>
          <button className="tab" onClick={() => void installPlugin()}>プラグイン導入</button>
        </span>
      </header>
      <div className="panel-body">
        {imports.map((m) => (
          <div
            key={m.path}
            className="list-item media-item"
            title="ダブルクリックで再生ヘッド位置に配置"
            onDoubleClick={() =>
              addObject({ type: "video", path: m.path }, m.path.split("/").pop() ?? "video")
            }
          >
            {m.thumb && <img className="media-thumb" src={m.thumb} alt="" />}
            <span>{m.path.split("/").pop()}</span>
          </div>
        ))}
        {STUBS.map((item) => (
          <div
            key={item.label}
            className="list-item"
            title="ダブルクリックで再生ヘッド位置に配置"
            onDoubleClick={() => addObject(item.kind, item.name)}
          >
            {item.label}
          </div>
        ))}
        <div className="dim" style={{ marginTop: 8 }}>
          プラグイン（{plugins.length}）
        </div>
        {plugins.map((p) => (
          <div key={p.name} className="list-item" title={p.description}>
            🧩 {p.name} <span className="dim">v{p.version}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
