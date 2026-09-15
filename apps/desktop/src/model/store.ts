import {
  Frame,
  Interpolation,
  Keyframe,
  Layer,
  LoopMode,
  ObjectKind,
  Project,
  TimelineObject,
  Value,
  activeScene,
  hasOverlap,
} from "./types";

export interface EditorState {
  project: Project | null;
  selection: string[];
  playhead: Frame;
  keySel: KeySel[];
  keyClipboard: KeyClipboard | null;
}

export interface KeySel {
  objId: string;
  param: string;
  frame: Frame;
}

export interface KeyClipboard {
  objId: string;
  param: string;
  keys: Keyframe[];
}

export type Action =
  | { type: "load"; project: Project }
  | { type: "select"; ids: string[]; additive: boolean }
  | { type: "playhead"; frame: Frame }
  | { type: "move"; id: string; delta: Frame }
  | { type: "trim"; id: string; edge: "start" | "end"; frame: Frame }
  | { type: "nudgeEdge"; id: string; edge: "start" | "end"; delta: Frame }
  | { type: "split"; id: string; at: Frame }
  | { type: "delete"; ids: string[] }
  | { type: "duplicate"; ids: string[] }
  | { type: "add"; layerId: string; kind: ObjectKind; name: string; at: Frame; len?: Frame }
  | { type: "align"; ids: string[]; mode: "start" | "end" }
  | { type: "addScene"; name: string }
  | { type: "setActiveScene"; id: string }
  | { type: "setParam"; id: string; name: string; value: Value }
  | { type: "setKey"; id: string; param: string; frame: Frame; value: Value; interpolation: Interpolation }
  | { type: "removeKey"; id: string; param: string; frame?: Frame }
  | { type: "setInterpolation"; id: string; param: string; frame: Frame; interpolation: Interpolation }
  | { type: "setLoop"; id: string; param: string; mode: LoopMode }
  | { type: "setTextBody"; id: string; body: string }
  | { type: "selectKeys"; keys: KeySel[]; additive: boolean }
  | { type: "copyKeys" }
  | { type: "pasteKeys"; objId: string; param: string; at: Frame };

let counter = 0;
export function uid(prefix: string): string {
  counter += 1;
  return `${prefix}-${Date.now().toString(36)}-${counter}`;
}

function mapObject(
  project: Project,
  id: string,
  fn: (obj: TimelineObject, layer: Layer) => TimelineObject | null,
): Project {
  return {
    ...project,
    scenes: project.scenes.map((scene) => ({
      ...scene,
      layers: scene.layers.map((layer) => {
        if (layer.locked || !layer.objects.some((o) => o.id === id)) return layer;
        const next = layer.objects
          .map((o) => (o.id === id ? fn(o, layer) : o))
          .filter((o): o is TimelineObject => o !== null);
        // 重なり・空区間になる編集は棄却（M1: 失敗は無視）
        if (hasOverlap(next)) return layer;
        return { ...layer, objects: next };
      }),
    })),
  };
}

function findLayer(project: Project, objectId: string): Layer | undefined {
  for (const scene of project.scenes) {
    for (const layer of scene.layers) {
      if (layer.objects.some((o) => o.id === objectId)) return layer;
    }
  }
  return undefined;
}

function layerMaxEnd(layer: Layer): Frame {
  return layer.objects.reduce((m, o) => Math.max(m, o.end_frame), 0);
}

export function reducer(state: EditorState, action: Action): EditorState {
  const { project } = state;
  switch (action.type) {
    case "load":
      return { project: action.project, selection: [], playhead: 0, keySel: [], keyClipboard: null };
    case "select": {
      if (!action.additive) return { ...state, selection: [...action.ids] };
      const set = new Set(state.selection);
      for (const id of action.ids) {
        if (set.has(id)) set.delete(id);
        else set.add(id);
      }
      return { ...state, selection: [...set] };
    }
    case "playhead":
      return { ...state, playhead: Math.max(0, Math.round(action.frame)) };
    case "setActiveScene":
      if (!project) return state;
      return {
        ...state,
        project: { ...project, active_scene_id: action.id },
        selection: [],
      };
    case "move": {
      if (!project) return state;
      return {
        ...state,
        project: mapObject(project, action.id, (o) => {
          const len = o.end_frame - o.start_frame;
          const start = Math.max(0, o.start_frame + action.delta);
          return { ...o, start_frame: start, end_frame: start + len };
        }),
      };
    }
    case "trim": {
      if (!project) return state;
      const frame = Math.round(action.frame);
      return {
        ...state,
        project: mapObject(project, action.id, (o) => {
          if (action.edge === "start") {
            if (frame < 0 || frame >= o.end_frame) return o;
            return { ...o, start_frame: frame };
          }
          if (frame <= o.start_frame) return o;
          return { ...o, end_frame: frame };
        }),
      };
    }
    case "nudgeEdge": {
      if (!project) return state;
      return {
        ...state,
        project: mapObject(project, action.id, (o) => {
          if (action.edge === "start") {
            const s = Math.max(0, o.start_frame + action.delta);
            if (s >= o.end_frame) return o;
            return { ...o, start_frame: s };
          }
          const e = o.end_frame + action.delta;
          if (e <= o.start_frame) return o;
          return { ...o, end_frame: e };
        }),
      };
    }
    case "split": {
      if (!project) return state;
      const at = Math.round(action.at);
      const layer = findLayer(project, action.id);
      if (!layer || layer.locked) return state;
      const target = layer.objects.find((o) => o.id === action.id);
      if (!target || !(target.start_frame < at && at < target.end_frame)) return state;
      const left: TimelineObject = {
        ...structuredClone(target),
        id: uid("obj"),
        end_frame: at,
        keyframes: target.keyframes.filter((k) => k.frame < at),
      };
      const right: TimelineObject = {
        ...structuredClone(target),
        id: uid("obj"),
        name: `${target.name}+`,
        start_frame: at,
        keyframes: target.keyframes.filter((k) => k.frame >= at),
      };
      const objects = layer.objects.flatMap((o) =>
        o.id === action.id ? [left, right] : [o],
      );
      if (hasOverlap(objects)) return state;
      return {
        ...state,
        project: {
          ...project,
          scenes: project.scenes.map((scene) => ({
            ...scene,
            layers: scene.layers.map((l) => (l.id === layer.id ? { ...l, objects } : l)),
          })),
        },
        selection: [left.id, right.id],
      };
    }
    case "delete": {
      if (!project) return state;
      const gone = new Set(action.ids);
      return {
        ...state,
        project: {
          ...project,
          scenes: project.scenes.map((scene) => ({
            ...scene,
            layers: scene.layers.map((layer) =>
              layer.locked
                ? layer
                : { ...layer, objects: layer.objects.filter((o) => !gone.has(o.id)) },
            ),
          })),
        },
        selection: state.selection.filter((id) => !gone.has(id)),
      };
    }
    case "duplicate": {
      if (!project) return state;
      const copies: { layerId: string; obj: TimelineObject }[] = [];
      for (const id of action.ids) {
        const layer = findLayer(project, id);
        const src = layer?.objects.find((o) => o.id === id);
        if (!layer || !src || layer.locked) continue;
        const len = src.end_frame - src.start_frame;
        const start = layerMaxEnd(layer) + 10;
        copies.push({
          layerId: layer.id,
          obj: { ...structuredClone(src), id: uid("obj"), start_frame: start, end_frame: start + len },
        });
      }
      if (copies.length === 0) return state;
      const byLayer = new Map<string, TimelineObject[]>();
      for (const c of copies) {
        if (!byLayer.has(c.layerId)) byLayer.set(c.layerId, []);
        byLayer.get(c.layerId)!.push(c.obj);
      }
      return {
        ...state,
        project: {
          ...project,
          scenes: project.scenes.map((scene) => ({
            ...scene,
            layers: scene.layers.map((layer) => {
              const add = byLayer.get(layer.id);
              return add ? { ...layer, objects: [...layer.objects, ...add] } : layer;
            }),
          })),
        },
        selection: copies.map((c) => c.obj.id),
      };
    }
    case "add": {
      if (!project) return state;
      const len = action.len ?? 90;
      const scene = activeScene(project);
      const layer = scene?.layers.find((l) => l.id === action.layerId);
      if (!layer || layer.locked) return state;
      let start = Math.max(0, Math.round(action.at));
      let obj: TimelineObject = {
        id: uid("obj"),
        name: action.name,
        kind: structuredClone(action.kind),
        start_frame: start,
        end_frame: start + len,
        params: [],
        keyframes: [],
      };
      if (hasOverlap([...layer.objects, obj])) {
        start = layerMaxEnd(layer) + 10;
        obj = { ...obj, start_frame: start, end_frame: start + len };
      }
      return {
        ...state,
        project: {
          ...project,
          scenes: project.scenes.map((s) => ({
            ...s,
            layers: s.layers.map((l) =>
              l.id === layer.id ? { ...l, objects: [...l.objects, obj] } : l,
            ),
          })),
        },
        selection: [obj.id],
      };
    }
    case "align": {
      if (!project || action.ids.length === 0) return state;
      const objs = action.ids
        .map((id) => ({ id, layer: findLayer(project, id) }))
        .filter((x): x is { id: string; layer: Layer } => !!x.layer && !x.layer.locked);
      if (objs.length === 0) return state;
      const refs = objs.map(({ id, layer }) => layer.objects.find((o) => o.id === id)!);
      const target =
        action.mode === "start"
          ? Math.min(...refs.map((o) => o.start_frame))
          : Math.max(...refs.map((o) => o.end_frame));
      let next = project;
      for (const o of refs) {
        const delta =
          action.mode === "start"
            ? target - o.start_frame
            : target - o.end_frame;
        if (delta === 0) continue;
        next = mapObject(next, o.id, (cur) => {
          const len = cur.end_frame - cur.start_frame;
          const start = Math.max(0, cur.start_frame + delta);
          return { ...cur, start_frame: start, end_frame: start + len };
        });
      }
      return { ...state, project: next };
    }
    case "addScene": {
      if (!project) return state;
      const id = uid("scene");
      const n = project.scenes.length + 1;
      return {
        ...state,
        project: {
          ...project,
          scenes: [
            ...project.scenes,
            {
              id,
              name: action.name || `Scene${n}`,
              width: 1920,
              height: 1080,
              fps_num: 30,
              fps_den: 1,
              sample_rate: 44100,
              bg_color: { r: 0, g: 0, b: 0, a: 1 },
              layers: [
                { id: uid("layer"), name: "Layer1", visible: true, locked: false, objects: [] },
              ],
            },
          ],
          active_scene_id: id,
        },
        selection: [],
      };
    }
    case "setParam": {
      if (!project) return state;
      const at = state.playhead;
      return {
        ...state,
        project: mapObject(project, action.id, (o) => {
          if (o.keyframes.some((k) => k.param === action.name)) {
            const cur = o.keyframes.find((k) => k.param === action.name && k.frame === at);
            const interpolation = cur?.interpolation ?? "linear";
            return {
              ...o,
              keyframes: [
                ...o.keyframes.filter((k) => !(k.param === action.name && k.frame === at)),
                { param: action.name, frame: at, value: structuredClone(action.value), interpolation },
              ],
            };
          }
          const params = o.params.some((p) => p.name === action.name)
            ? o.params.map((p) =>
                p.name === action.name ? { ...p, value: structuredClone(action.value) } : p,
              )
            : [...o.params, { name: action.name, value: structuredClone(action.value) }];
          return { ...o, params };
        }),
      };
    }
    case "setKey": {
      if (!project) return state;
      const frame = Math.round(action.frame);
      return {
        ...state,
        project: mapObject(project, action.id, (o) => ({
          ...o,
          keyframes: [
            ...o.keyframes.filter((k) => !(k.param === action.param && k.frame === frame)),
            {
              param: action.param,
              frame,
              value: structuredClone(action.value),
              interpolation: action.interpolation,
            },
          ],
        })),
      };
    }
    case "removeKey": {
      if (!project) return state;
      return {
        ...state,
        project: mapObject(project, action.id, (o) => ({
          ...o,
          keyframes: o.keyframes.filter(
            (k) =>
              k.param !== action.param ||
              (action.frame !== undefined && k.frame !== Math.round(action.frame)),
          ),
        })),
        keySel: state.keySel.filter(
          (s) =>
            !(s.objId === action.id && s.param === action.param &&
              (action.frame === undefined || s.frame === Math.round(action.frame))),
        ),
      };
    }
    case "setInterpolation": {
      if (!project) return state;
      const frame = Math.round(action.frame);
      return {
        ...state,
        project: mapObject(project, action.id, (o) => ({
          ...o,
          keyframes: o.keyframes.map((k) =>
            k.param === action.param && k.frame === frame
              ? { ...k, interpolation: action.interpolation }
              : k,
          ),
        })),
      };
    }
    case "setLoop": {
      if (!project) return state;
      return {
        ...state,
        project: mapObject(project, action.id, (o) => ({
          ...o,
          loops: { ...(o.loops ?? {}), [action.param]: action.mode },
        })),
      };
    }
    case "setTextBody": {
      if (!project) return state;
      return {
        ...state,
        project: mapObject(project, action.id, (o) => {
          if (o.kind.type !== "text") return o;
          return { ...o, kind: { type: "text", body: action.body }, name: action.body.slice(0, 12) || "テキスト" };
        }),
      };
    }
    case "selectKeys": {
      if (!action.additive) return { ...state, keySel: [...action.keys] };
      const list = [...state.keySel];
      for (const k of action.keys) {
        const i = list.findIndex(
          (s) => s.objId === k.objId && s.param === k.param && s.frame === k.frame,
        );
        if (i >= 0) list.splice(i, 1);
        else list.push(k);
      }
      return { ...state, keySel: list };
    }
    case "copyKeys": {
      if (!project || state.keySel.length === 0) return state;
      const first = state.keySel[0];
      const layer = findLayer(project, first.objId);
      const obj = layer?.objects.find((o) => o.id === first.objId);
      if (!obj) return state;
      const keys = obj.keyframes.filter((k) =>
        state.keySel.some((s) => s.param === k.param && s.frame === k.frame),
      );
      if (keys.length === 0) return state;
      return {
        ...state,
        keyClipboard: {
          objId: first.objId,
          param: first.param,
          keys: structuredClone(keys),
        },
      };
    }
    case "pasteKeys": {
      if (!project || !state.keyClipboard) return state;
      const cb = state.keyClipboard;
      if (cb.keys.length === 0) return state;
      const min = Math.min(...cb.keys.map((k) => k.frame));
      const at = Math.round(action.at);
      return {
        ...state,
        project: mapObject(project, action.objId, (o) => {
          let next = [...o.keyframes];
          for (const k of cb.keys) {
            const frame = at + (k.frame - min);
            next = next.filter((x) => !(x.param === k.param && x.frame === frame));
            next.push({ param: k.param, frame, value: structuredClone(k.value), interpolation: k.interpolation });
          }
          return { ...o, keyframes: next };
        }),
        keySel: cb.keys.map((k) => ({
          objId: action.objId,
          param: k.param,
          frame: at + (k.frame - min),
        })),
      };
    }
  }
}
