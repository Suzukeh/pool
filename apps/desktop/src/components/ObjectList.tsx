import { Action } from "../model/store";
import { Frame, Scene } from "../model/types";

export default function ObjectList({
  scene,
  playhead,
  selection,
  dispatch,
}: {
  scene: Scene;
  playhead: Frame;
  selection: string[];
  dispatch: React.Dispatch<Action>;
}) {
  return (
    <section className="panel">
      <header className="panel-title">
        <span>オブジェクトリスト（現フレーム）</span>
      </header>
      <div className="panel-body">
        {scene.layers.map((layer) =>
          layer.objects
            .filter((o) => o.start_frame <= playhead && playhead < o.end_frame)
            .map((o) => (
              <div
                key={o.id}
                className={`list-item${selection.includes(o.id) ? " active" : ""}`}
                onClick={(e) =>
                  dispatch({ type: "select", ids: [o.id], additive: e.ctrlKey || e.metaKey })
                }
              >
                {layer.name} / {o.name}
                <span className="dim">
                  {" "}
                  [{o.start_frame}–{o.end_frame}]
                </span>
              </div>
            )),
        )}
      </div>
    </section>
  );
}
