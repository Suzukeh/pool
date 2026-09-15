import { invoke } from "@tauri-apps/api/core";
import { Project, hasOverlap } from "./types";
import { fallbackProject } from "./fallback";

const LS_KEY = "pool.project.v1";

async function tauri<T>(fn: () => Promise<T>): Promise<T | null> {
  try {
    return await fn();
  } catch {
    return null; // ブラウザ開発時
  }
}

export async function getVersion(): Promise<string> {
  return (await tauri(() => invoke<string>("get_app_version"))) ?? "0.1.0 (browser)";
}

/** 起動時：保存済み → 新規 → フォールバックの順で取得 */
export async function bootProject(name: string): Promise<Project> {
  const loaded = await tauri(() => invoke<string>("load_project"));
  if (loaded) return JSON.parse(loaded) as Project;
  try {
    const ls = localStorage.getItem(LS_KEY);
    if (ls) {
      const p = JSON.parse(ls) as Project;
      if (p.version === 1) return p;
    }
  } catch {
    /* ignore */
  }
  const created = await tauri(() => invoke<string>("new_project", { name }));
  return created ? (JSON.parse(created) as Project) : fallbackProject(name);
}

function validateLocal(p: Project): void {
  if (p.version !== 1) throw new Error("unsupported version");
  for (const scene of p.scenes) {
    if (scene.fps_den === 0) throw new Error("fps_den is zero");
    for (const layer of scene.layers) {
      if (hasOverlap(layer.objects)) throw new Error(`overlap in ${layer.id}`);
    }
  }
}

/** 保存：Tauri 検証→保存、不可ならローカル検証→localStorage */
export async function persistProject(p: Project): Promise<string> {
  const saved = await tauri(async () => {
    await invoke("validate_project", { json: JSON.stringify(p) });
    return invoke<string>("save_project", { json: JSON.stringify(p) });
  });
  if (saved) return saved;
  validateLocal(p);
  localStorage.setItem(LS_KEY, JSON.stringify(p));
  return "browser localStorage";
}
