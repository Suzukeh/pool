// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Action, reducer } from "../../model/store";
import { fallbackProject } from "../../model/fallback";
import { activeScene } from "../../model/types";
import Timeline from "./Timeline";

function setup() {
  const state = { project: fallbackProject("t"), selection: [] as string[], playhead: 30, keySel: [], keyClipboard: null };
  const dispatch = vi.fn((a: Action) => {
    void a;
  });
  const withObjects = reducer(
    reducer(state, {
      type: "add",
      layerId: "layer-1",
      kind: { type: "text", body: "テロップ" },
      name: "テロップ",
      at: 0,
    }),
    { type: "playhead", frame: 30 },
  );
  const scene = activeScene(withObjects.project!)!;
  render(
    <Timeline
      scene={scene}
      selection={withObjects.selection}
      playhead={30}
      fps={30}
      keySel={[]}
      dispatch={dispatch}
    />,
  );
  return { dispatch, scene };
}

describe("Timeline render", () => {
  afterEach(() => cleanup());
  it("レイヤー行とオブジェクトバーを描画する", () => {
    setup();
    expect(screen.getByText("Layer1")).toBeTruthy();
    expect(screen.getByText("テロップ")).toBeTruthy();
    expect(screen.getByText("レイヤー編集")).toBeTruthy();
  });

  it("バーを押すと選択アクションを送る", () => {
    const { dispatch } = setup();
    const bar = document.querySelector(".tl-bar")!;
    fireEvent.pointerDown(bar, { clientX: 50, button: 0 });
    expect(dispatch.mock.calls.some((c) => c[0].type === "select")).toBe(true);
  });

  it("選択中バーに selected クラスが付く", () => {
    const { scene } = setup();
    const id = scene.layers[0].objects[0].id;
    const dispatch = vi.fn();
    render(
      <Timeline scene={scene} selection={[id]} playhead={30} fps={30} keySel={[]} dispatch={dispatch} />,
    );
    const selected = [...document.querySelectorAll(".tl-bar")].filter((b) =>
      b.classList.contains("selected"),
    );
    expect(selected.length).toBeGreaterThan(0);
  });

  it("Delete キーで削除アクションを送る", () => {
    const { dispatch } = setup();
    fireEvent.keyDown(document.querySelector(".tl-scroll")!, { key: "Delete" });
    expect(dispatch.mock.calls.some((c) => c[0].type === "delete")).toBe(true);
  });
});
