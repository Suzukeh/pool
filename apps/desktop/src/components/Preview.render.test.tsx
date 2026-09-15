// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(async (_cmd: unknown, _args: unknown) => "iVBORw0KGgo="),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { fallbackProject } from "../model/fallback";
import { activeScene } from "../model/types";
import Preview from "./Preview";

describe("Preview", () => {
  afterEach(() => cleanup());

  it("render_preview を呼んで img を表示する", async () => {
    vi.useFakeTimers();
    try {
      const project = fallbackProject("t");
      const scene = activeScene(project)!;
      render(
        <Preview
          project={project}
          scene={scene}
          playhead={10}
          fps={30}
          playing={false}
          onStep={() => {}}
          onTogglePlay={() => {}}
        />,
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(200);
      });
      expect(invokeMock).toHaveBeenCalledWith("render_preview", {
        json: JSON.stringify(project),
        frame: 10,
        width: 480,
        height: 270,
      });
      const img = screen.getByAltText("preview") as HTMLImageElement;
      expect(img.src).toContain("data:image/png;base64,");
    } finally {
      vi.useRealTimers();
    }
  });
});
