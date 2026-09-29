import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n";
import { Feedback } from "./Feedback";

const submitFeedback = vi.fn();
const openExternal = vi.fn();
vi.mock("../api", () => ({ submitFeedback: (...args: unknown[]) => submitFeedback(...args) }));
vi.mock("../lib/runtime", () => ({
  openExternal: (...args: unknown[]) => openExternal(...args),
}));

describe("Feedback", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
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

  it("opens the prefilled issue form in the browser and keeps the draft", async () => {
    const form = "https://github.com/Jia-Ethan/keysmith-switch/issues/new?template=bug-report.yml&title=Bug";
    submitFeedback.mockResolvedValue({ issueUrl: null, fallbackUrl: form, reason: "browser", truncated: false });
    render(<Feedback />);
    fireEvent.click(screen.getByRole("button", { name: "Report a problem" }));
    expect(screen.getByTestId("feedback-how")).toHaveTextContent("prefilled GitHub form");
    expect(screen.getByTestId("feedback-screenshot-tip")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "What happened (required)" }), { target: { value: "A button fails" } });
    fireEvent.click(screen.getByRole("button", { name: "Submit to GitHub" }));
    await waitFor(() => expect(openExternal).toHaveBeenCalledWith(form));
    expect(submitFeedback).toHaveBeenCalledWith({ kind: "bug", description: "A button fails", solution: "", contact: "", screenshots: [] });
    expect(await screen.findByText(/prefilled form is open in your browser/)).toBeInTheDocument();
    expect(screen.queryByText(/only the first part was prefilled/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open the form again" }));
    expect(openExternal).toHaveBeenCalledTimes(2);
    fireEvent.click(within(screen.getByRole("dialog")).getAllByRole("button", { name: "Close" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Report a problem" }));
    expect(screen.getByRole("textbox", { name: "What happened (required)" })).toHaveValue("A button fails");
  });

  it("says when a long draft was only partly prefilled", async () => {
    submitFeedback.mockResolvedValue({ issueUrl: null, fallbackUrl: "https://github.com/x", reason: "createFailed", truncated: true });
    render(<Feedback />);
    fireEvent.click(screen.getByRole("button", { name: "Report a problem" }));
    fireEvent.change(screen.getByRole("textbox", { name: "What happened (required)" }), { target: { value: "Long" } });
    fireEvent.click(screen.getByRole("button", { name: "Submit to GitHub" }));
    expect(await screen.findByText(/could not create the issue/)).toBeInTheDocument();
    expect(screen.getByText(/only the first part was prefilled/)).toBeInTheDocument();
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
