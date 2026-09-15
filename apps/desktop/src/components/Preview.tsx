import { useEffect, useState } from "react";
import { Frame, Project, Scene } from "../model/types";
import { frameToTC } from "./Timeline/Timeline";

export default function Preview({
  project,
  scene,
  playhead,
  fps,
  playing,
  onStep,
  onTogglePlay,
}: {
  project: Project;
  scene: Scene;
  playhead: Frame;
  fps: number;
  playing: boolean;
  onStep: (d: number) => void;
  onTogglePlay: () => void;
}) {
  const [img, setImg] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const t = window.setTimeout(async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const width = 480;
        const height = Math.max(16, Math.round((480 * scene.height) / scene.width));
        const b64 = await invoke<string>("render_preview", {
          json: JSON.stringify(project),
          frame: playhead,
          width,
          height,
        });
        if (alive) setImg(`data:image/png;base64,${b64}`);
      } catch {
        /* ブラウザ開発時は何もしない */
      }
    }, 120);
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [project, scene, playhead]);

  const count = scene.layers.reduce(
    (n, l) => n + l.objects.filter((o) => o.start_frame <= playhead && playhead < o.end_frame).length,
    0,
  );
  return (
    <section className="panel">
      <header className="panel-title">
        <span>プレビュー編集</span>
        <span className="transport">
          <button className="tab" onClick={() => onStep(-10)}>⏮</button>
          <button className="tab" onClick={() => onStep(-1)}>◀</button>
          <button className="tab" onClick={onTogglePlay}>{playing ? "⏸" : "▶"}</button>
          <button className="tab" onClick={() => onStep(1)}>▶</button>
          <button className="tab" onClick={() => onStep(10)}>⏭</button>
          <span className="dim">{frameToTC(playhead, fps)}</span>
        </span>
      </header>
      <div className="panel-body checker">
        {img ? (
          <img className="preview-img" src={img} alt="preview" />
        ) : (
          <div className="preview-meta">
            {scene.name} {scene.width}x{scene.height} / f{playhead} / オブジェクト{count}個
            <br />
            <span className="dim">描画待機中…</span>
          </div>
        )}
      </div>
    </section>
  );
}
