import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { isZcodeUnavailable, zcodeUnavailableReason, ZCODE_WINDOWS_REASON } from "../lib/zcode";
import { ZCodeBanner } from "./ZCodeBanner";

describe("ZCode unavailable banner logic", () => {
  it("treats only unavailable zcode as unavailable", () => {
    expect(isZcodeUnavailable({ id: "zcode", available: false })).toBe(true);
    expect(isZcodeUnavailable({ id: "zcode", available: true })).toBe(false);
    expect(isZcodeUnavailable({ id: "claude", available: false })).toBe(false);
  });

  it("uses the backend reason and never invents an install command", () => {
    expect(
      zcodeUnavailableReason({
        id: "zcode",
        available: false,
        unavailableReason: "ZCode is macOS-only on this host.",
      }),
    ).toBe("ZCode is macOS-only on this host.");
    expect(
      zcodeUnavailableReason(
        { id: "zcode", available: false, unavailableReason: null },
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
      ),
    ).toBe(ZCODE_WINDOWS_REASON);
    expect(ZCODE_WINDOWS_REASON.toLowerCase()).not.toMatch(/brew|winget|choco|install command/);
  });

  it("renders the banner with the disabled reason", () => {
    render(
      <ZCodeBanner
        tool={{
          id: "zcode",
          available: false,
          unavailableReason: "ZCode is only available on macOS.",
        }}
      />,
    );
    expect(screen.getByTestId("zcode-unavailable")).toBeInTheDocument();
    expect(screen.getByText("ZCode is only available on macOS.")).toBeInTheDocument();
    expect(screen.getByText("不提供安装猜测。")).toBeInTheDocument();
  });

  it("hides the banner when zcode is available", () => {
    const { container } = render(
      <ZCodeBanner tool={{ id: "zcode", available: true, unavailableReason: null }} />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});
