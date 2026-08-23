import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PromptEditPage } from "./PromptEditPage";
import type { ToastApi } from "../hooks/useToasts";
import type { PromptDetail } from "../types";

const createPrompt = vi.fn();
const updatePrompt = vi.fn();
const getPrompt = vi.fn();

vi.mock("../api", () => ({
  createPrompt: (...args: unknown[]) => createPrompt(...args),
  updatePrompt: (...args: unknown[]) => updatePrompt(...args),
  getPrompt: (...args: unknown[]) => getPrompt(...args),
}));

const mockToast: ToastApi = {
  toasts: [],
  dismiss: vi.fn(),
  info: vi.fn(),
  ok: vi.fn(),
  err: vi.fn(),
};

const mockDetail: PromptDetail = {
  id: "prompt-1",
  tool: "claude",
  title: "Existing Prompt",
  content: "Existing content",
  tags: ["existing"],
  createdAt: "2026-08-19T00:00:00Z",
  updatedAt: "2026-08-19T00:00:00Z",
  active: false,
  lastUsedAt: null,
  excerpt: "Existing content",
};

describe("PromptEditPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders in creation mode with empty fields", () => {
    render(
      <PromptEditPage
        tool="claude"
        detail={null}
        creating={true}
        toast={mockToast}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );

    expect(screen.getByTestId("prompt-form-title")).toHaveValue("");
    expect(screen.getByTestId("prompt-form-save")).toBeInTheDocument();
  });

  it("renders in edit mode with existing prompt data", () => {
    render(
      <PromptEditPage
        tool="claude"
        detail={mockDetail}
        creating={false}
        toast={mockToast}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );

    expect(screen.getByTestId("prompt-form-title")).toHaveValue("Existing Prompt");
  });

  it("loads an existing prompt by route id and reports dirty changes", async () => {
    const onDirtyChange = vi.fn();
    getPrompt.mockResolvedValue(mockDetail);
    render(
      <PromptEditPage
        tool="claude"
        promptId="prompt-1"
        creating={false}
        toast={mockToast}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        onDirtyChange={onDirtyChange}
      />,
    );

    expect(await screen.findByTestId("prompt-form-title")).toHaveValue("Existing Prompt");
    fireEvent.change(screen.getByTestId("prompt-form-title"), {
      target: { value: "Changed Prompt" },
    });
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(true));
  });

  it("shows a retryable error when route detail loading fails", async () => {
    getPrompt.mockRejectedValueOnce(new Error("read failed")).mockResolvedValueOnce(mockDetail);
    render(
      <PromptEditPage
        tool="claude"
        promptId="prompt-1"
        creating={false}
        toast={mockToast}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "重试" }));
    expect(await screen.findByTestId("prompt-form-title")).toHaveValue("Existing Prompt");
    expect(getPrompt).toHaveBeenCalledTimes(2);
  });

  it("validates required title before saving", async () => {
    render(
      <PromptEditPage
        tool="claude"
        detail={null}
        creating={true}
        toast={mockToast}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );

    const saveButton = screen.getByTestId("prompt-form-save");
    expect(saveButton).toBeDisabled();

    fireEvent.change(screen.getByTestId("prompt-form-title"), {
      target: { value: "New Prompt" },
    });

    await waitFor(() => {
      expect(saveButton).not.toBeDisabled();
    });
  });

  it("calls createPrompt when saving a new prompt", async () => {
    const onSaved = vi.fn();
    createPrompt.mockResolvedValue({ id: "new-prompt-id" });

    render(
      <PromptEditPage
        tool="claude"
        detail={null}
        creating={true}
        toast={mockToast}
        onClose={vi.fn()}
        onSaved={onSaved}
      />,
    );

    fireEvent.change(screen.getByTestId("prompt-form-title"), {
      target: { value: "New Prompt" },
    });

    fireEvent.click(screen.getByTestId("prompt-form-save"));

    await waitFor(() => {
      expect(createPrompt).toHaveBeenCalledWith({
        tool: "claude",
        title: "New Prompt",
        content: "",
        tags: [],
      });
    });
    expect(onSaved).toHaveBeenCalledWith("new-prompt-id");
  });

  it("calls updatePrompt when saving an existing prompt", async () => {
    const onSaved = vi.fn();
    updatePrompt.mockResolvedValue({ id: "prompt-1" });

    render(
      <PromptEditPage
        tool="claude"
        detail={mockDetail}
        creating={false}
        toast={mockToast}
        onClose={vi.fn()}
        onSaved={onSaved}
      />,
    );

    fireEvent.change(screen.getByTestId("prompt-form-title"), {
      target: { value: "Updated Prompt" },
    });

    fireEvent.click(screen.getByTestId("prompt-form-save"));

    await waitFor(() => {
      expect(updatePrompt).toHaveBeenCalledWith({
        id: "prompt-1",
        title: "Updated Prompt",
        content: "Existing content",
        tags: ["existing"],
      });
    });
    expect(onSaved).toHaveBeenCalledWith("prompt-1");
  });

  it("keeps the draft open and unlocks controls after saving fails", async () => {
    updatePrompt.mockRejectedValue(new Error("save failed"));
    const onSaved = vi.fn();
    const onClose = vi.fn();

    render(
      <PromptEditPage
        tool="claude"
        detail={mockDetail}
        creating={false}
        toast={mockToast}
        onClose={onClose}
        onSaved={onSaved}
      />,
    );

    fireEvent.change(screen.getByTestId("prompt-form-title"), {
      target: { value: "Unsaved Prompt" },
    });
    fireEvent.click(screen.getByTestId("prompt-form-save"));

    await waitFor(() => expect(mockToast.err).toHaveBeenCalled());
    expect(screen.getByTestId("prompt-form-title")).toHaveValue("Unsaved Prompt");
    expect(screen.getByTestId("prompt-form-title")).not.toBeDisabled();
    expect(screen.getByTestId("prompt-form-save")).not.toBeDisabled();
    expect(onSaved).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("locks every close path while a save is pending", async () => {
    let resolveSave!: (value: { id: string }) => void;
    updatePrompt.mockReturnValue(new Promise((resolve) => { resolveSave = resolve; }));
    const onSaved = vi.fn();
    const onClose = vi.fn();

    render(
      <PromptEditPage
        tool="claude"
        detail={mockDetail}
        creating={false}
        toast={mockToast}
        onClose={onClose}
        onSaved={onSaved}
      />,
    );

    fireEvent.change(screen.getByTestId("prompt-form-title"), {
      target: { value: "Saving Prompt" },
    });
    fireEvent.click(screen.getByTestId("prompt-form-save"));

    await waitFor(() => expect(updatePrompt).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId("fullscreen-back")).toBeDisabled();
    expect(screen.getByTestId("prompt-form-save")).toBeDisabled();
    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.click(screen.getByTestId("fullscreen-back"));
    expect(onClose).not.toHaveBeenCalled();

    resolveSave({ id: "prompt-1" });
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith("prompt-1"));
  });

  it("confirms before closing with unsaved changes", () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    const onClose = vi.fn();

    render(
      <PromptEditPage
        tool="claude"
        detail={null}
        creating={true}
        toast={mockToast}
        onClose={onClose}
        onSaved={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByTestId("prompt-form-title"), {
      target: { value: "Draft Title" },
    });

    fireEvent.click(screen.getByRole("button", { name: /取消|cancel/i }));

    expect(confirmSpy).toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();

    confirmSpy.mockRestore();
  });
});
