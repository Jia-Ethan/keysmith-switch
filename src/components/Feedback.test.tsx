import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n";
import { Feedback } from "./Feedback";

const submitFeedback = vi.fn();
const openExternal = vi.fn();
const pickFiles = vi.fn();
vi.mock("../api", () => ({ submitFeedback: (...args: unknown[]) => submitFeedback(...args) }));
vi.mock("../lib/runtime", () => ({
  openExternal: (...args: unknown[]) => openExternal(...args),
  pickFiles: (...args: unknown[]) => pickFiles(...args),
}));

describe("Feedback", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    pickFiles.mockResolvedValue([]);
  });

  it("requires both feature fields and shows a created issue link", async () => {
    submitFeedback.mockResolvedValue({ issueUrl: "https://github.com/Jia-Ethan/keysmith-switch/issues/42", fallbackUrl: null, reason: null });
    render(<Feedback />);
    fireEvent.click(screen.getByRole("button", { name: "Request a feature" }));
    const submit = screen.getByRole("button", { name: "Submit to GitHub" });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByRole("textbox", { name: "What do you need (required)" }), { target: { value: "Need filters" } });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByRole("textbox", { name: "Expected solution (required)" }), { target: { value: "Add a filter" } });
    fireEvent.click(submit);
    await waitFor(() => expect(submitFeedback).toHaveBeenCalledWith({ kind: "feature", description: "Need filters", solution: "Add a filter", contact: "", screenshots: [] }));
    fireEvent.click(await screen.findByRole("button", { name: "Open issue" }));
    expect(openExternal).toHaveBeenCalledWith("https://github.com/Jia-Ethan/keysmith-switch/issues/42");
  });

  it("keeps a bug draft and screenshot for the manual browser handoff", async () => {
    pickFiles.mockResolvedValue(["/tmp/screenshot.png"]);
    submitFeedback.mockResolvedValue({ issueUrl: null, fallbackUrl: "https://github.com/Jia-Ethan/keysmith-switch/issues/new?title=Bug", reason: "screenshotsManual" });
    render(<Feedback />);
    fireEvent.click(screen.getByRole("button", { name: "Report a problem" }));
    fireEvent.change(screen.getByRole("textbox", { name: "What happened (required)" }), { target: { value: "A button fails" } });
    fireEvent.click(screen.getByRole("button", { name: "Add screenshot" }));
    expect(await screen.findByText("screenshot.png")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Submit to GitHub" }));
    expect(await screen.findByText(/not a missing GitHub CLI/)).toBeInTheDocument();
    expect(submitFeedback).toHaveBeenCalledWith({ kind: "bug", description: "A button fails", solution: "", contact: "", screenshots: ["/tmp/screenshot.png"] });
    fireEvent.click(screen.getByRole("button", { name: "Open prefilled issue" }));
    expect(openExternal).toHaveBeenCalledTimes(1);
    fireEvent.click(within(screen.getByRole("dialog")).getAllByRole("button", { name: "Close" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Report a problem" }));
    expect(screen.getByRole("textbox", { name: "What happened (required)" })).toHaveValue("A button fails");
    expect(screen.getByText("screenshot.png")).toBeInTheDocument();
  });

  it("retains the draft on a CLI failure", async () => {
    submitFeedback.mockRejectedValue(new Error("backend unavailable"));
    render(<Feedback />);
    fireEvent.click(screen.getByRole("button", { name: "Report a problem" }));
    fireEvent.change(screen.getByRole("textbox", { name: "What happened (required)" }), { target: { value: "A problem" } });
    fireEvent.click(screen.getByRole("button", { name: "Submit to GitHub" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("backend unavailable");
    expect(screen.getByRole("textbox", { name: "What happened (required)" })).toHaveValue("A problem");
  });
});
