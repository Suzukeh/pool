// @vitest-environment jsdom
import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Action } from "../../model/store";
import { TimelineObject } from "../../model/types";
import SplineEditor from "./SplineEditor";

function keyedObj(): TimelineObject {
  return {
    id: "k1",
    name: "移動",
    kind: { type: "shape", shape: "rectangle" },
    start_frame: 0,
    end_frame: 100,
    params: [],
    keyframes: [
      { param: "x", frame: 0, value: { type: "number", value: 0 }, interpolation: "linear" },
      { param: "x", frame: 30, value: { type: "number", value: 90 }, interpolation: "linear" },
    ],
  };
}

describe("SplineEditor", () => {
  afterEach(() => cleanup());

  it("カーブとキー◆を描画する", () => {
    const obj = keyedObj();
    render(
      <SplineEditor
        rows={[{ obj, param: "x" }]}
        playhead={5}
        keySel={[]}
        dispatch={vi.fn()}
      />,
    );
    expect(document.querySelectorAll("polygon.spline-key")).toHaveLength(2);
    expect(document.querySelector("path")).toBeTruthy();
  });

  it("プリセットは選択キーに補間を適用する", () => {
    const dispatch = vi.fn((a: Action) => {
      void a;
    });
    const obj = keyedObj();
    render(
      <SplineEditor
        rows={[{ obj, param: "x" }]}
        playhead={5}
        keySel={[{ objId: "k1", param: "x", frame: 0 }]}
        dispatch={dispatch}
      />,
    );
    const btns = [...document.querySelectorAll(".spline-presets .tab")];
    const ease = btns.find((b) => b.textContent === "イージー")!;
    fireEvent.click(ease);
    expect(
      dispatch.mock.calls.some(
        (c) =>
          c[0].type === "setInterpolation" &&
          c[0].frame === 0 &&
          typeof c[0].interpolation === "object",
      ),
    ).toBe(true);
  });
});
