import { Action } from "../model/store";
import { Project, activeScene } from "../model/types";

export default function SceneList({
  project,
  dispatch,
}: {
  project: Project;
  dispatch: React.Dispatch<Action>;
}) {
  const active = activeScene(project);
  return (
    <section className="panel">
      <header className="panel-title">
        <span>シーンリスト</span>
        <button className="tab" onClick={() => dispatch({ type: "addScene", name: "" })}>
          ＋
        </button>
      </header>
      <div className="panel-body">
        {project.scenes.map((s) => (
          <div
            key={s.id}
            className={`list-item${s.id === active?.id ? " active" : ""}`}
            onClick={() => dispatch({ type: "setActiveScene", id: s.id })}
          >
            {s.name} <span className="dim">({s.width}x{s.height})</span>
          </div>
        ))}
      </div>
    </section>
  );
}
