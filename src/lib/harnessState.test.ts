import { beforeEach, describe, expect, it, vi } from "vitest";

const getHarnessState = vi.fn();

vi.mock("../api", () => ({
  getHarnessState: (...args: unknown[]) => getHarnessState(...args),
}));

async function load() {
  return import("./harnessState");
}

describe("harnessState", () => {
  beforeEach(async () => {
    const store = await load();
    store.resetHarnessStatuses();
    getHarnessState.mockReset();
    getHarnessState.mockResolvedValue({ tool: "claude", deployed: false, error: null });
  });

  it("reads a tool once and reuses the answer afterwards", async () => {
    const store = await load();
    await store.loadHarnessStatus("claude");
    await store.loadHarnessStatus("claude");

    expect(getHarnessState).toHaveBeenCalledTimes(1);
    expect(store.getHarnessStatus("claude")).toEqual({ machine: "undeployed", error: null });
  });

  it("keeps each tool's answer separate", async () => {
    const store = await load();
    getHarnessState.mockImplementation((tool: string) =>
      Promise.resolve({ tool, deployed: tool === "codex", error: null }),
    );

    await store.loadHarnessStatus("claude");
    await store.loadHarnessStatus("codex");

    expect(store.getHarnessStatus("claude")?.machine).toBe("undeployed");
    expect(store.getHarnessStatus("codex")?.machine).toBe("deployed");
    expect(getHarnessState).toHaveBeenCalledTimes(2);
  });

  it("re-reads only when forced", async () => {
    const store = await load();
    await store.loadHarnessStatus("claude");
    getHarnessState.mockResolvedValue({ tool: "claude", deployed: true, error: null });

    await store.loadHarnessStatus("claude", true);

    expect(getHarnessState).toHaveBeenCalledTimes(2);
    expect(store.getHarnessStatus("claude")?.machine).toBe("deployed");
  });

  it("shares one read between concurrent callers", async () => {
    const store = await load();
    let release: (value: { tool: string; deployed: boolean; error: null }) => void = () => undefined;
    getHarnessState.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    );

    const first = store.loadHarnessStatus("claude");
    const second = store.loadHarnessStatus("claude");
    release({ tool: "claude", deployed: true, error: null });
    await Promise.all([first, second]);

    expect(getHarnessState).toHaveBeenCalledTimes(1);
  });

  it("records deploy and remove results without another read", async () => {
    const store = await load();
    store.applyHarnessOutcome("grok", "deploy");
    expect(store.getHarnessStatus("grok")).toEqual({ machine: "deployed", error: null });

    store.applyHarnessOutcome("grok", "remove");
    expect(store.getHarnessStatus("grok")).toEqual({ machine: "undeployed", error: null });
    expect(getHarnessState).not.toHaveBeenCalled();
  });

  it("surfaces a failed read as an error on an undeployed machine", async () => {
    const store = await load();
    getHarnessState.mockRejectedValue(new Error("no backend"));

    const entry = await store.loadHarnessStatus("zcode");

    expect(entry).toEqual({ machine: "undeployed", error: "no backend" });
    expect(store.getHarnessStatus("zcode")?.error).toBe("no backend");
  });

  it("starts empty, so a fresh launch has nothing remembered", async () => {
    const store = await load();
    expect(store.getHarnessStatus("claude")).toBeUndefined();
  });
});
