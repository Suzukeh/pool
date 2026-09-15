import { describe, expect, it } from "vitest";
import { evalNumber } from "./eval";
import { TimelineObject } from "./types";

function keyed(): TimelineObject {
  return {
    id: "k",
    name: "k",
    kind: { type: "text", body: "x" },
    start_frame: 0,
    end_frame: 100,
    params: [],
    keyframes: [
      { param: "x", frame: 0, value: { type: "number", value: 0 }, interpolation: "linear" },
      { param: "x", frame: 10, value: { type: "number", value: 10 }, interpolation: "linear" },
    ],
  };
}

describe("eval (Rust 一致)", () => {
  it("キーなし基本値ありは基本値を返す", () => {
    const o = keyed();
    o.keyframes = [];
    o.params = [{ name: "x", value: { type: "number", value: 3 } }];
    expect(evalNumber(o, "x", 5, 42)).toBeCloseTo(3, 9);
  });  it("linear 中点", () => {
    expect(evalNumber(keyed(), "x", 5, -1)).toBeCloseTo(5, 9);
  });
  it("範囲外はクランプ", () => {
    expect(evalNumber(keyed(), "x", 99, -1)).toBeCloseTo(10, 9);
  });
  it("loop で巻き戻る", () => {
    const o = keyed();
    o.loops = { x: "loop" };
    expect(evalNumber(o, "x", 25, -1)).toBeCloseTo(5, 9);
  });
  it("pingpong で折り返す", () => {
    const o = keyed();
    o.loops = { x: "pingpong" };
    expect(evalNumber(o, "x", 15, -1)).toBeCloseTo(5, 9);
  });
  it("bezier 恒等は linear と一致", () => {
    const o = keyed();
    o.keyframes[0].interpolation = { bezier: { x1: 0, y1: 0, x2: 1, y2: 1 } };
    expect(evalNumber(o, "x", 3, -1)).toBeCloseTo(3, 1);
  });
  it("easy_ease は対称で中点 0.5", () => {
    const o = keyed();
    o.keyframes[0].interpolation = { bezier: { x1: 0.42, y1: 0, x2: 0.58, y2: 1 } };
    expect(evalNumber(o, "x", 5, -1)).toBeCloseTo(5, 1);
  });
});
