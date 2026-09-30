import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ExtensionsView } from "../types";
import { ExtensionsProvider, extensionErrorCode, useExtensions } from "./ExtensionsProvider";

const extensionsState = vi.fn();
const extensionsRefresh = vi.fn();
const installExtension = vi.fn();
const uninstallExtension = vi.fn();

vi.mock("../api", () => ({
  extensionsState: (...args: unknown[]) => extensionsState(...args),
  extensionsRefresh: (...args: unknown[]) => extensionsRefresh(...args),
  installExtension: (...args: unknown[]) => installExtension(...args),
  uninstallExtension: (...args: unknown[]) => uninstallExtension(...args),
}));

const pack = {
  id: "a.pack",
  version: "0.2.0",
  minAppVersion: "0.2.5",
  name: "A",
  description: "d",
  tools: ["claude" as const],
  itemCount: 1,
  size: 100,
  official: true,
  compatible: true,
  installedVersion: "0.1.0",
  updateAvailable: true,
};
const withUpdate: ExtensionsView = { configured: true, packs: [pack], updates: 1, error: null, checkedAt: null };
const current: ExtensionsView = { ...withUpdate, packs: [{ ...pack, installedVersion: "0.2.0", updateAvailable: false }], updates: 0 };

function Harness() {
  const ext = useExtensions();
  return (
    <div>
      <output data-testid="updates">{ext.updates}</output>
      <output data-testid="packs">{ext.view?.packs.length ?? "none"}</output>
      <output data-testid="busy">{ext.busyId ?? "-"}</output>
      <button onClick={() => void ext.refresh()}>refresh</button>
      <button onClick={() => void ext.install("a.pack")}>install</button>
      <button onClick={() => void ext.setEnabled(true)}>enable</button>
    </div>
  );
}

const mount = (enabled: boolean, onEnabledChange = vi.fn().mockResolvedValue(undefined)) =>
  render(
    <ExtensionsProvider enabled={enabled} onEnabledChange={onEnabledChange}>
      <Harness />
    </ExtensionsProvider>,
  );

describe("ExtensionsProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    extensionsState.mockResolvedValue(withUpdate);
    extensionsRefresh.mockResolvedValue(withUpdate);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reads nothing, not even the cache, while extensions are off", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "Date"] });
    mount(false);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(7 * 60 * 60 * 1000);
      window.dispatchEvent(new Event("focus"));
    });
    expect(extensionsState).not.toHaveBeenCalled();
    expect(extensionsRefresh).not.toHaveBeenCalled();
    expect(screen.getByTestId("updates")).toHaveTextContent("0");
  });

  it("shows the cached list at once and looks at the network a moment later", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "Date"] });
    mount(true);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });
    expect(screen.getByTestId("updates")).toHaveTextContent("1");
    expect(extensionsRefresh).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(extensionsRefresh).toHaveBeenCalledTimes(1);
  });

  it("looks again on a schedule and when the window returns after an hour, not sooner", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "Date"] });
    mount(true);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(extensionsRefresh).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10 * 60 * 1000);
      window.dispatchEvent(new Event("focus"));
    });
    expect(extensionsRefresh).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
      window.dispatchEvent(new Event("focus"));
    });
    expect(extensionsRefresh).toHaveBeenCalledTimes(2);
  });

  it("stays quiet when a background look fails", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "Date"] });
    extensionsRefresh.mockRejectedValue(new Error("boom"));
    mount(true);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(screen.getByTestId("updates")).toHaveTextContent("1");
  });

  it("clears the badge count when switched off", async () => {
    const view = mount(true);
    await waitFor(() => expect(screen.getByTestId("updates")).toHaveTextContent("1"));
    view.rerender(
      <ExtensionsProvider enabled={false} onEnabledChange={vi.fn()}>
        <Harness />
      </ExtensionsProvider>,
    );
    expect(screen.getByTestId("updates")).toHaveTextContent("0");
    expect(screen.getByTestId("packs")).toHaveTextContent("none");
  });

  it("marks the pack busy while installing and takes the new view", async () => {
    let finish!: (value: unknown) => void;
    installExtension.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    mount(true);
    await waitFor(() => expect(screen.getByTestId("packs")).toHaveTextContent("1"));
    fireEvent.click(screen.getByText("install"));
    await waitFor(() => expect(screen.getByTestId("busy")).toHaveTextContent("a.pack"));
    await act(async () => finish({ view: current, report: { added: 0, updated: 1, copied: 0, linked: 0, kept: 1, removed: 0 } }));
    await waitFor(() => expect(screen.getByTestId("busy")).toHaveTextContent("-"));
    expect(screen.getByTestId("updates")).toHaveTextContent("0");
  });

  it("hands the switch to the caller", async () => {
    const onEnabledChange = vi.fn().mockResolvedValue(undefined);
    mount(false, onEnabledChange);
    fireEvent.click(screen.getByText("enable"));
    await waitFor(() => expect(onEnabledChange).toHaveBeenCalledWith(true));
  });

  it("reads the failure code out of the backend's message", () => {
    expect(extensionErrorCode(new Error("extensions:invalid-signature:"))).toBe("invalid-signature");
    expect(extensionErrorCode(new Error("Error: extensions:offline:could not resolve"))).toBe("offline");
    expect(extensionErrorCode(new Error("something else"))).toBe("default");
    expect(extensionErrorCode(undefined)).toBe("default");
  });
});
