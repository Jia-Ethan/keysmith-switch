import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateCheck, UpdateInstall } from "../types";
import { UpdateProvider, useUpdate } from "./UpdateProvider";

const checkAppUpdate = vi.fn();
const installAppUpdate = vi.fn();

vi.mock("../api", () => ({
  checkAppUpdate: (...args: unknown[]) => checkAppUpdate(...args),
  installAppUpdate: (...args: unknown[]) => installAppUpdate(...args),
}));

vi.mock("../lib/runtime", () => ({
  isTauriRuntime: vi.fn().mockReturnValue(false),
}));

const availableUpdate: UpdateCheck = {
  available: true,
  currentVersion: "0.1.1",
  latestVersion: "0.1.2",
  notes: "Release notes",
  size: 1_048_576,
  channel: "stable",
  restartRequired: true,
  progress: null,
  error: null,
  releasePage: "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.2",
};

function UpdateHarness() {
  const updater = useUpdate();
  return (
    <div>
      <button type="button" onClick={() => void updater.check()}>check</button>
      <button type="button" onClick={() => void updater.install()}>install</button>
      <output>{updater.update?.latestVersion ?? "none"}</output>
    </div>
  );
}

describe("UpdateProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("prevents duplicate update checks while one is pending", async () => {
    let resolveCheck!: (value: UpdateCheck) => void;
    checkAppUpdate.mockReturnValue(new Promise((resolve) => { resolveCheck = resolve; }));

    render(
      <UpdateProvider channel="stable" autoCheck={false}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    fireEvent.click(screen.getByRole("button", { name: "check" }));
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);

    await act(async () => resolveCheck(availableUpdate));
    expect(screen.getByText("0.1.2")).toBeInTheDocument();
  });

  it("prevents duplicate installs after an available update is confirmed", async () => {
    let resolveInstall!: (value: UpdateInstall) => void;
    checkAppUpdate.mockResolvedValue(availableUpdate);
    installAppUpdate.mockReturnValue(new Promise((resolve) => { resolveInstall = resolve; }));

    render(
      <UpdateProvider channel="stable" autoCheck={false}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByText("0.1.2")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "install" }));
    fireEvent.click(screen.getByRole("button", { name: "install" }));
    expect(installAppUpdate).toHaveBeenCalledTimes(1);

    await act(async () => resolveInstall({
      ok: true,
      restartRequired: true,
      error: null,
      releasePage: availableUpdate.releasePage,
    }));
  });
});
