import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
  installMode: "inApp",
  reason: null,
};

function UpdateHarness() {
  const updater = useUpdate();
  return (
    <div>
      <button type="button" onClick={() => void updater.check()}>check</button>
      <button type="button" onClick={() => void updater.check({ silent: true })}>silent</button>
      <button type="button" onClick={() => void updater.install()}>install</button>
      <output data-testid="update-version">{updater.update?.latestVersion ?? "none"}</output>
      <output data-testid="install-mode">{updater.update?.installMode ?? "none"}</output>
      <output data-testid="manual-reason">{updater.update?.reason ?? "none"}</output>
      <output data-testid="update-detail">{updater.update?.detail?.message ?? "none"}</output>
      <output data-testid="update-error">{updater.error ?? "none"}</output>
      <output data-testid="check-count">{updater.checkCount}</output>
      <output data-testid="checking">{String(updater.checking)}</output>
      <output data-testid="has-update">{String(updater.hasUpdate)}</output>
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
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    fireEvent.click(screen.getByRole("button", { name: "check" }));
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);

    await act(async () => resolveCheck(availableUpdate));
    expect(screen.getByText("0.1.2")).toBeInTheDocument();
  });

  it("holds the checking state for the minimum time so a fast answer still reads as an action", async () => {
    checkAppUpdate.mockResolvedValue({ ...availableUpdate, available: false, installMode: "none" as const });

    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={250}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(checkAppUpdate).toHaveBeenCalledTimes(1));
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByTestId("checking")).toHaveTextContent("true");

    await waitFor(() => expect(screen.getByTestId("checking")).toHaveTextContent("false"));
    expect(screen.getByTestId("check-count")).toHaveTextContent("1");
  });

  it("refreshes the completion event for two identical up-to-date checks", async () => {
    const current = { ...availableUpdate, available: false, latestVersion: "0.1.4", currentVersion: "0.1.4", installMode: "none" as const };
    checkAppUpdate.mockResolvedValue(current);
    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByTestId("check-count")).toHaveTextContent("1"));
    expect(screen.getByTestId("update-version")).toHaveTextContent("0.1.4");
    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByTestId("check-count")).toHaveTextContent("2"));
    expect(screen.getByTestId("update-version")).toHaveTextContent("0.1.4");
    expect(checkAppUpdate).toHaveBeenCalledTimes(2);
  });

  it("clears a prior success while checking and reports a returned request error", async () => {
    let resolveCheck!: (value: UpdateCheck) => void;
    checkAppUpdate.mockResolvedValueOnce(availableUpdate).mockReturnValueOnce(new Promise((resolve) => { resolveCheck = resolve; }));
    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByTestId("check-count")).toHaveTextContent("1"));
    fireEvent.click(screen.getByRole("button", { name: "check" }));
    expect(screen.getByTestId("checking")).toHaveTextContent("true");
    expect(screen.getByTestId("update-version")).toHaveTextContent("none");
    await act(async () => resolveCheck({ ...availableUpdate, available: false, error: "offline: network unavailable" }));
    expect(screen.getByTestId("update-error")).toHaveTextContent("offline: network unavailable");
    expect(screen.getByTestId("check-count")).toHaveTextContent("2");
  });

  it("prevents duplicate installs after an available update is confirmed", async () => {
    let resolveInstall!: (value: UpdateInstall) => void;
    checkAppUpdate.mockResolvedValue(availableUpdate);
    installAppUpdate.mockReturnValue(new Promise((resolve) => { resolveInstall = resolve; }));

    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
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
      installMode: "inApp",
      reason: null,
    }));
  });

  it("switches to a structured manual state when installation detects a signing key mismatch", async () => {
    checkAppUpdate.mockResolvedValue(availableUpdate);
    installAppUpdate.mockResolvedValue({
      ok: false,
      restartRequired: false,
      error: null,
      releasePage: availableUpdate.releasePage,
      installMode: "manual",
      reason: "signatureKeyMismatch",
      detail: {
        code: "signature_key_mismatch",
        message: "The signature was created with a different key than the one provided",
      },
    } satisfies UpdateInstall);

    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByText("0.1.2")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "install" }));

    await waitFor(() => expect(screen.getByTestId("install-mode")).toHaveTextContent("manual"));
    expect(screen.getByTestId("manual-reason")).toHaveTextContent("signatureKeyMismatch");
    expect(screen.getByTestId("update-error")).toHaveTextContent("none");
    expect(screen.getByTestId("update-detail")).toHaveTextContent(
      "The signature was created with a different key than the one provided",
    );
    expect(screen.queryByText(/secret backend detail/)).not.toBeInTheDocument();
  });

  it("does not call the install command for an update already marked manual", async () => {
    checkAppUpdate.mockResolvedValue({
      ...availableUpdate,
      installMode: "manual",
      reason: "bootstrapRequired",
      restartRequired: false,
    });

    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByTestId("install-mode")).toHaveTextContent("manual"));
    fireEvent.click(screen.getByRole("button", { name: "install" }));
    expect(installAppUpdate).not.toHaveBeenCalled();
  });

  it("clears a stale manual update when a later check fails", async () => {
    checkAppUpdate
      .mockResolvedValueOnce({
        ...availableUpdate,
        installMode: "manual",
        reason: "bootstrapRequired",
        restartRequired: false,
      })
      .mockRejectedValueOnce(new Error("network unavailable"));

    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByTestId("install-mode")).toHaveTextContent("manual"));

    fireEvent.click(screen.getByRole("button", { name: "check" }));
    await waitFor(() => expect(screen.getByTestId("update-error")).toHaveTextContent("network unavailable"));
    expect(screen.getByTestId("update-version")).toHaveTextContent("none");
    expect(screen.getByTestId("install-mode")).toHaveTextContent("none");
  });
});

describe("UpdateProvider background checks", () => {
  beforeEach(() => {
    checkAppUpdate.mockReset();
    installAppUpdate.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("learns of a release without ever showing the checking state", async () => {
    checkAppUpdate.mockResolvedValue(availableUpdate);
    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );
    fireEvent.click(screen.getByText("silent"));
    await waitFor(() => expect(screen.getByTestId("update-version")).toHaveTextContent("0.1.2"));
    expect(screen.getByTestId("checking")).toHaveTextContent("false");
    expect(screen.getByTestId("check-count")).toHaveTextContent("0");
  });

  it("keeps a known release and shows no error when a background look fails", async () => {
    checkAppUpdate.mockResolvedValueOnce(availableUpdate);
    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );
    fireEvent.click(screen.getByText("silent"));
    await waitFor(() => expect(screen.getByTestId("update-version")).toHaveTextContent("0.1.2"));

    checkAppUpdate.mockRejectedValueOnce(new Error("offline"));
    fireEvent.click(screen.getByText("silent"));
    await waitFor(() => expect(checkAppUpdate).toHaveBeenCalledTimes(2));
    checkAppUpdate.mockResolvedValueOnce({ ...availableUpdate, available: false, error: "feed unreachable" });
    fireEvent.click(screen.getByText("silent"));
    await waitFor(() => expect(checkAppUpdate).toHaveBeenCalledTimes(3));

    expect(screen.getByTestId("update-version")).toHaveTextContent("0.1.2");
    expect(screen.getByTestId("update-error")).toHaveTextContent("none");
  });

  it("looks again when the window regains focus after an hour, not sooner", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "Date"] });
    checkAppUpdate.mockResolvedValue({ ...availableUpdate, available: false });
    render(
      <UpdateProvider channel="stable" autoCheck minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10 * 60 * 1000);
      window.dispatchEvent(new Event("focus"));
    });
    expect(checkAppUpdate).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
      window.dispatchEvent(new Event("focus"));
    });
    expect(checkAppUpdate).toHaveBeenCalledTimes(2);
  });

  it("checks on a schedule while the app stays open", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "Date"] });
    checkAppUpdate.mockResolvedValue({ ...availableUpdate, available: false });
    render(
      <UpdateProvider channel="stable" autoCheck minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    const afterStart = checkAppUpdate.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(6 * 60 * 60 * 1000);
    });
    expect(checkAppUpdate.mock.calls.length).toBe(afterStart + 1);
  });

  it("does not look in the background when automatic checks are off", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "Date"] });
    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(7 * 60 * 60 * 1000);
      window.dispatchEvent(new Event("focus"));
    });
    expect(checkAppUpdate).not.toHaveBeenCalled();
  });

  it("keeps knowing a release is out while a new check runs, and drops it once one says otherwise", async () => {
    checkAppUpdate.mockResolvedValueOnce(availableUpdate);
    render(
      <UpdateProvider channel="stable" autoCheck={false} minCheckMs={0}>
        <UpdateHarness />
      </UpdateProvider>,
    );
    fireEvent.click(screen.getByText("silent"));
    await waitFor(() => expect(screen.getByTestId("has-update")).toHaveTextContent("true"));

    let release!: (value: UpdateCheck) => void;
    checkAppUpdate.mockReturnValueOnce(new Promise<UpdateCheck>((resolve) => { release = resolve; }));
    fireEvent.click(screen.getByText("check"));
    await waitFor(() => expect(screen.getByTestId("checking")).toHaveTextContent("true"));
    expect(screen.getByTestId("update-version")).toHaveTextContent("none");
    expect(screen.getByTestId("has-update")).toHaveTextContent("true");

    await act(async () => release({ ...availableUpdate, available: false }));
    await waitFor(() => expect(screen.getByTestId("has-update")).toHaveTextContent("false"));
  });
});
