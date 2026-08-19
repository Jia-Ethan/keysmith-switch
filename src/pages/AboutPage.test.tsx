import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AboutPage } from "./AboutPage";
import { PUBLIC_RELEASE_PAGE } from "../types";

const checkAppUpdate = vi.fn();
const installAppUpdate = vi.fn();
const getAbout = vi.fn();

vi.mock("../api", () => ({
  checkAppUpdate: (...args: unknown[]) => checkAppUpdate(...args),
  installAppUpdate: (...args: unknown[]) => installAppUpdate(...args),
  getAbout: (...args: unknown[]) => getAbout(...args),
  planOfficialAction: vi.fn(),
  confirmOfficialAction: vi.fn(),
}));

describe("AboutPage update button", () => {
  beforeEach(() => {
    checkAppUpdate.mockReset();
    installAppUpdate.mockReset();
    getAbout.mockReset();
    getAbout.mockResolvedValue({
      app: { name: "Keysmith Switch", version: "0.1.0", channel: "stable" },
      adapters: [],
      official: [],
    });
    checkAppUpdate.mockResolvedValue({
      available: true,
      currentVersion: "0.1.0",
      latestVersion: "0.1.1",
      notes: "fixture notes",
      size: 1024,
      channel: "stable",
      restartRequired: true,
      progress: null,
      error: null,
      releasePage: PUBLIC_RELEASE_PAGE,
    });
    installAppUpdate.mockResolvedValue({
      ok: true,
      restartRequired: true,
      error: null,
      releasePage: PUBLIC_RELEASE_PAGE,
    });
  });

  it("keeps 更新并重启 disabled until update is available and confirmed", async () => {
    render(<AboutPage channel="stable" />);

    const button = screen.getByTestId("install-update");
    expect(button).toBeDisabled();
    expect(installAppUpdate).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("check-update"));
    await waitFor(() => expect(checkAppUpdate).toHaveBeenCalled());
    expect(button).toBeDisabled();
    expect(installAppUpdate).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("confirm-update"));
    expect(button).not.toBeDisabled();

    fireEvent.click(button);
    await waitFor(() => expect(installAppUpdate).toHaveBeenCalledTimes(1));
  });

  it("never silently installs on mount", async () => {
    render(<AboutPage channel="stable" />);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(checkAppUpdate).not.toHaveBeenCalled();
    expect(installAppUpdate).not.toHaveBeenCalled();
    expect(screen.getByTestId("install-update")).toBeDisabled();
  });
});
