// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Action } from "../model/store";
import { TimelineObject } from "../model/types";
import Inspector from "./Inspector";

function textObj(): TimelineObject {
  return {
    id: "t1",
    name: "テロップ",
    kind: { type: "text", body: "hi" },
    start_frame: 0,
    end_frame: 100,
    params: [{ name: "x", value: { type: "number", value: 10 } }],
    keyframes: [
      { param: "x", frame: 10, value: { type: "number", value: 20 }, interpolation: "linear" },
    ],
  };
}

describe("Inspector", () => {
  afterEach(() => cleanup());

  it("パラメータ行と◆を表示する", () => {
    render(
      <Inspector object={textObj()} extraSelected={0} playhead={10} dispatch={vi.fn()} />,
    );
    expect(screen.getByText("X")).toBeTruthy();
    expect(screen.getByText("サイズ")).toBeTruthy();
    expect(document.querySelectorAll(".kf").length).toBeGreaterThan(0);
  });

  it("現フレームの◆はキーを削除する", () => {
    const dispatch = vi.fn((a: Action) => {
      void a;
    });
    render(<Inspector object={textObj()} extraSelected={0} playhead={10} dispatch={dispatch} />);
    const diamonds = document.querySelectorAll(".kf.here");
    expect(diamonds.length).toBeGreaterThan(0);
    fireEvent.click(diamonds[0]);
    expect(
      dispatch.mock.calls.some(
        (c) => c[0].type === "removeKey" && c[0].param === "x" && c[0].frame === 10,
      ),
    ).toBe(true);
  });

  it("数値編集は setParam を送る", () => {
    const dispatch = vi.fn((a: Action) => {
      void a;
    });
    render(<Inspector object={textObj()} extraSelected={0} playhead={5} dispatch={dispatch} />);
    const num = document.querySelector('input[type="number"]')!;
    fireEvent.change(num, { target: { value: "42" } });
    expect(
      dispatch.mock.calls.some((c) => c[0].type === "setParam" && c[0].name === "x"),
    ).toBe(true);
  });
});
