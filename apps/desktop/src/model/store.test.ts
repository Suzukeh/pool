import { describe, expect, it } from "vitest";
import { EditorState, reducer } from "./store";
import { Project, TimelineObject, hasOverlap } from "./types";
import { fallbackProject } from "./fallback";

function obj(id: string, start: number, end: number): TimelineObject {
  return {
    id,
    name: id,
    kind: { type: "text", body: "hi" },
    start_frame: start,
    end_frame: end,
    params: [],
    keyframes: [],
  };
}

function stateWith(objects: TimelineObject[]): EditorState {
  const project: Project = fallbackProject("t");
  project.scenes[0].layers[0].objects = objects;
  return { project, selection: [], playhead: 0, keySel: [], keyClipboard: null };
}

describe("hasOverlap", () => {
  it("隣接は重なりではない", () => {
    expect(hasOverlap([obj("a", 0, 90), obj("b", 90, 150)])).toBe(false);
  });
  it("重なりを検出する", () => {
    expect(hasOverlap([obj("a", 0, 100), obj("b", 50, 120)])).toBe(true);
  });
  it("空区間を検出する", () => {
    expect(hasOverlap([obj("a", 30, 30)])).toBe(true);
  });
});

describe("reducer", () => {
  it("move は区間長を保ち、0 未満にしない", () => {
    const s = reducer(stateWith([obj("a", 10, 50)]), { type: "move", id: "a", delta: -30 });
    const o = s.project!.scenes[0].layers[0].objects[0];
    expect(o.start_frame).toBe(0);
    expect(o.end_frame).toBe(40);
  });

  it("move で重なる場合は棄却する", () => {
    const s = reducer(stateWith([obj("a", 0, 50), obj("b", 60, 100)]), {
      type: "move",
      id: "b",
      delta: -30,
    });
    const o = s.project!.scenes[0].layers[0].objects.find((x) => x.id === "b")!;
    expect(o.start_frame).toBe(60);
  });

  it("trim end を伸ばす", () => {
    const s = reducer(stateWith([obj("a", 0, 50)]), {
      type: "trim",
      id: "a",
      edge: "end",
      frame: 80,
    });
    expect(s.project!.scenes[0].layers[0].objects[0].end_frame).toBe(80);
  });

  it("split は2分割しキーフレームを振り分ける", () => {
    const base = obj("a", 0, 100);
    base.keyframes = [
      { param: "x", frame: 10, value: { type: "number", value: 1 }, interpolation: "linear" },
      { param: "x", frame: 70, value: { type: "number", value: 2 }, interpolation: "linear" },
    ];
    const s = reducer(stateWith([base]), { type: "split", id: "a", at: 40 });
    const objs = s.project!.scenes[0].layers[0].objects;
    expect(objs).toHaveLength(2);
    expect([objs[0].start_frame, objs[0].end_frame]).toEqual([0, 40]);
    expect([objs[1].start_frame, objs[1].end_frame]).toEqual([40, 100]);
    expect(objs[0].keyframes).toHaveLength(1);
    expect(objs[1].keyframes).toHaveLength(1);
    expect(s.selection).toHaveLength(2);
  });

  it("範囲外の split は無視する", () => {
    const s = reducer(stateWith([obj("a", 0, 100)]), { type: "split", id: "a", at: 100 });
    expect(s.project!.scenes[0].layers[0].objects).toHaveLength(1);
  });

  it("duplicate は末尾に複製する", () => {
    const s = reducer(stateWith([obj("a", 0, 50)]), { type: "duplicate", ids: ["a"] });
    const objs = s.project!.scenes[0].layers[0].objects;
    expect(objs).toHaveLength(2);
    expect(objs[1].start_frame).toBe(60);
    expect(objs[1].end_frame).toBe(110);
  });

  it("add は指定位置、重なれば末尾に置く", () => {
    let s = reducer(stateWith([obj("a", 0, 50)]), {
      type: "add",
      layerId: "layer-1",
      kind: { type: "shape", shape: "rectangle" },
      name: "rect",
      at: 0,
    });
    let objs = s.project!.scenes[0].layers[0].objects;
    expect(objs[1].start_frame).toBe(60); // 0-90 は重なるので末尾へ
    s = reducer(s, {
      type: "add",
      layerId: "layer-1",
      kind: { type: "shape", shape: "ellipse" },
      name: "e",
      at: 200,
      len: 30,
    });
    objs = s.project!.scenes[0].layers[0].objects;
    expect([objs[2].start_frame, objs[2].end_frame]).toEqual([200, 230]);
  });

  it("delete と選択加算トグル", () => {
    let s = reducer(stateWith([obj("a", 0, 50), obj("b", 60, 100)]), {
      type: "select",
      ids: ["a"],
      additive: false,
    });
    s = reducer(s, { type: "select", ids: ["b"], additive: true });
    expect(s.selection).toEqual(["a", "b"]);
    s = reducer(s, { type: "delete", ids: ["a"] });
    expect(s.project!.scenes[0].layers[0].objects.map((o) => o.id)).toEqual(["b"]);
    expect(s.selection).toEqual(["b"]);
  });
});

describe("reducer keys", () => {
  it("setKey 追加と上書き", () => {
    let s = stateWith([obj("a", 0, 100)]);
    s = reducer(s, {
      type: "setKey",
      id: "a",
      param: "x",
      frame: 10,
      value: { type: "number", value: 5 },
      interpolation: "linear",
    });
    s = reducer(s, {
      type: "setKey",
      id: "a",
      param: "x",
      frame: 10,
      value: { type: "number", value: 7 },
      interpolation: "hold",
    });
    const keys = s.project!.scenes[0].layers[0].objects[0].keyframes;
    expect(keys).toHaveLength(1);
    expect(keys[0].value).toEqual({ type: "number", value: 7 });
    expect(keys[0].interpolation).toBe("hold");
  });

  it("setParam はキーありなら再生ヘッドに打鍵", () => {
    let s = stateWith([obj("a", 0, 100)]);
    s = reducer(s, {
      type: "setKey",
      id: "a",
      param: "x",
      frame: 0,
      value: { type: "number", value: 0 },
      interpolation: "linear",
    });
    s = reducer(s, { type: "playhead", frame: 30 });
    s = reducer(s, { type: "setParam", id: "a", name: "x", value: { type: "number", value: 9 } });
    const keys = s.project!.scenes[0].layers[0].objects[0].keyframes;
    expect(keys.map((k) => k.frame).sort((x, y) => x - y)).toEqual([0, 30]);
  });

  it("removeKey と loop 設定", () => {
    let s = stateWith([obj("a", 0, 100)]);
    s = reducer(s, {
      type: "setKey",
      id: "a",
      param: "x",
      frame: 10,
      value: { type: "number", value: 5 },
      interpolation: "linear",
    });
    s = reducer(s, { type: "setLoop", id: "a", param: "x", mode: "loop" });
    expect(s.project!.scenes[0].layers[0].objects[0].loops).toEqual({ x: "loop" });
    s = reducer(s, { type: "removeKey", id: "a", param: "x", frame: 10 });
    expect(s.project!.scenes[0].layers[0].objects[0].keyframes).toHaveLength(0);
  });

  it("copy/paste で相対オフセット維持", () => {
    let s = stateWith([obj("a", 0, 100)]);
    s = reducer(s, {
      type: "setKey",
      id: "a",
      param: "x",
      frame: 10,
      value: { type: "number", value: 1 },
      interpolation: "linear",
    });
    s = reducer(s, {
      type: "setKey",
      id: "a",
      param: "x",
      frame: 20,
      value: { type: "number", value: 2 },
      interpolation: "linear",
    });
    s = reducer(s, {
      type: "selectKeys",
      keys: [
        { objId: "a", param: "x", frame: 10 },
        { objId: "a", param: "x", frame: 20 },
      ],
      additive: false,
    });
    s = reducer(s, { type: "copyKeys" });
    s = reducer(s, { type: "pasteKeys", objId: "a", param: "x", at: 50 });
    const frames = s.project!.scenes[0].layers[0].objects[0].keyframes
      .map((k) => k.frame)
      .sort((x, y) => x - y);
    expect(frames).toEqual([10, 20, 50, 60]);
  });
});
