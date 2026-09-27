import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetHarnessStatuses } from "../lib/harnessState";

const createPastedPrompt = vi.fn();
const planActivate = vi.fn();
const activate = vi.fn();
const planDeactivate = vi.fn();
const deactivate = vi.fn();
const getHarnessState = vi.fn();
const listTools = vi.fn();
const toolStatus = vi.fn();

vi.mock("../components/MarkdownEditor", () => ({
  MarkdownEditor: ({ value, onChange, ariaLabel }: { value: string; onChange: (value: string) => void; ariaLabel?: string }) => (
    <textarea aria-label={ariaLabel} value={value} onChange={(event) => onChange(event.target.value)} />
  ),
}));

vi.mock("../api", () => ({
  createPastedPrompt: (...args: unknown[]) => createPastedPrompt(...args),
  planActivate: (...args: unknown[]) => planActivate(...args),
  activate: (...args: unknown[]) => activate(...args),
  planDeactivate: (...args: unknown[]) => planDeactivate(...args),
  deactivate: (...args: unknown[]) => deactivate(...args),
  getHarnessState: (...args: unknown[]) => getHarnessState(...args),
  listTools: (...args: unknown[]) => listTools(...args),
  toolStatus: (...args: unknown[]) => toolStatus(...args),
}));

const envelope = {
  schema: "keysmith-switch/adapter-v1",
  tool: "claude",
  command: "plan-activate",
  ok: true,
  preview: true,
  available: true,
  unavailableReason: null,
  adapterVersion: "7.1",
  cliPath: null,
  argv: [],
  exitCode: 0,
  status: "inactive",
  recoveryRequired: false,
  scopes: [],
  targetPaths: [],
  plannedFiles: [],
  backups: [],
  conflicts: [],
  warnings: [],
  blockers: [],
  currentFingerprint: null,
  targetFingerprint: null,
  doctor: { ok: true, checks: [] },
  reloadRequired: false,
  reloadHint: null,
  error: null,
  redactedStderr: "",
} as const;

function setup() {
  listTools.mockResolvedValue({
    tools: [
      {
        id: "claude",
        name: "Claude Code",
        adapterVersion: "7.1",
        available: true,
        unavailableReason: null,
        supportedScopes: ["user", "project", "local"],
        cliPath: null,
      },
      {
        id: "codex",
        name: "Codex",
        adapterVersion: "0.3.8",
        available: true,
        unavailableReason: null,
        supportedScopes: ["user"],
        cliPath: null,
      },
    ],
  });
  toolStatus.mockResolvedValue({ ...envelope, tool: "claude", scopes: [{ id: "project", supported: true, reason: null }] });
  getHarnessState.mockResolvedValue({ tool: "codex", deployed: false, error: null });
  createPastedPrompt.mockResolvedValue({ id: "prompt-1", title: "My rules" });
  planActivate.mockResolvedValue({ operationId: "op-1", envelope });
  activate.mockResolvedValue({ envelope: { ...envelope, ok: true, preview: false, command: "activate" } });
  planDeactivate.mockResolvedValue({ operationId: "op-2", envelope: { ...envelope, command: "plan-deactivate" } });
  deactivate.mockResolvedValue({ envelope: { ...envelope, ok: true, preview: false, command: "deactivate" } });
}

describe("Quick Deploy page", () => {
  beforeEach(() => {
    resetHarnessStatuses();
    vi.clearAllMocks();
    setup();
  });

  it("renders an editable prompt form and reads the machine once per run", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    const { unmount } = render(<HarnessPage tool="codex" />);
    expect(await screen.findByRole("heading", { name: "快速部署" })).toBeInTheDocument();
    await waitFor(() => expect(getHarnessState).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId("quick-deploy-title")).toHaveValue("粘贴的提示词");
    expect(screen.getByRole("textbox", { name: "提示词内容" })).toBeInTheDocument();
    expect(screen.getByTestId("quick-deploy-submit")).toHaveTextContent("生成部署预览");
    unmount();
    render(<HarnessPage tool="codex" />);
    expect(getHarnessState).toHaveBeenCalledTimes(1);
  });

  it("requires pasted content before creating a prompt", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    expect(createPastedPrompt).not.toHaveBeenCalled();
    expect(screen.getByTestId("quick-deploy-status")).toHaveTextContent("请输入标题和提示词内容。");
  });

  it("saves, previews, and confirms a pasted prompt", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    fireEvent.change(screen.getByTestId("quick-deploy-title"), { target: { value: "My rules" } });
    fireEvent.change(screen.getByRole("textbox", { name: "提示词内容" }), { target: { value: "Be concise." } });
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    await waitFor(() => expect(createPastedPrompt).toHaveBeenCalledWith({ tool: "codex", title: "My rules", content: "Be concise." }));
    expect(planActivate).toHaveBeenCalledWith({ promptId: "prompt-1", scope: "user", projectDir: undefined });
    fireEvent.click(await screen.findByTestId("quick-deploy-confirm"));
    await waitFor(() => expect(activate).toHaveBeenCalledWith("op-1"));
    await waitFor(() => expect(screen.getByTestId("quick-deploy-remove")).toBeInTheDocument());
    expect(getHarnessState).toHaveBeenCalledTimes(1);
  });

  it("shows remove after a deployed state and uses the standard deactivation plan", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null });
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    fireEvent.click(await screen.findByTestId("quick-deploy-remove"));
    expect(await screen.findByTestId("quick-deploy-confirm")).toBeInTheDocument();
    expect(planDeactivate).toHaveBeenCalledWith({ tool: "codex", scope: "user", projectDir: undefined });
  });

  it("blocks a preview with adapter blockers before execution", async () => {
    planActivate.mockResolvedValue({ operationId: "blocked", envelope: { ...envelope, ok: false, blockers: ["conflict"] } });
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    fireEvent.change(screen.getByTestId("quick-deploy-title"), { target: { value: "My rules" } });
    fireEvent.change(screen.getByRole("textbox", { name: "提示词内容" }), { target: { value: "Be concise." } });
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    const confirm = await screen.findByTestId("quick-deploy-confirm");
    expect(confirm).toBeDisabled();
    expect(activate).not.toHaveBeenCalled();
  });

  it("keeps a saved draft clean after preview cancellation and failed retry", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    const onDirtyChange = vi.fn();
    render(<HarnessPage tool="codex" onDirtyChange={onDirtyChange} />);
    fireEvent.change(screen.getByRole("textbox", { name: "提示词内容" }), { target: { value: "Be concise." } });
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(true));
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    fireEvent.click(await screen.findByRole("button", { name: "取消" }));
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(false));
    planActivate.mockRejectedValueOnce(new Error("preview unavailable"));
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    await waitFor(() => expect(planActivate).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(false));
    expect(createPastedPrompt).toHaveBeenCalledTimes(2);
  });

  it("shows reused library metadata in the confirmation", async () => {
    createPastedPrompt.mockResolvedValue({ id: "existing-1", title: "Original import" });
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    fireEvent.change(screen.getByTestId("quick-deploy-title"), { target: { value: "New title" } });
    fireEvent.change(screen.getByRole("textbox", { name: "提示词内容" }), { target: { value: "Be concise." } });
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    await waitFor(() => expect(planActivate).toHaveBeenCalledWith({ promptId: "existing-1", scope: "user", projectDir: undefined }));
    expect(screen.getByRole("dialog")).toHaveTextContent("Original import");
  });

  it("refreshes user status explicitly without a second initial read", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    await waitFor(() => expect(screen.getByTestId("quick-deploy-status")).toHaveTextContent("当前未部署"));
    expect(getHarnessState).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByTestId("harness-refresh")).toBeEnabled());
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null });
    fireEvent.click(screen.getByTestId("harness-refresh"));
    await waitFor(() => expect(getHarnessState).toHaveBeenCalledTimes(2));
    expect(await screen.findByTestId("quick-deploy-remove")).toBeInTheDocument();
  });

  it("supports project scope with an explicit directory", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="claude" />);
    await waitFor(() => expect(screen.getByRole("radiogroup", { name: "当前目标范围" })).toBeInTheDocument());
    fireEvent.click(screen.getByRole("radio", { name: "项目" }));
    fireEvent.change(screen.getByTestId("scope-project-dir"), { target: { value: "/tmp/example" } });
    fireEvent.change(screen.getByTestId("quick-deploy-title"), { target: { value: "Project rules" } });
    fireEvent.change(screen.getByRole("textbox", { name: "提示词内容" }), { target: { value: "Use tests." } });
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    await waitFor(() => expect(planActivate).toHaveBeenCalledWith({ promptId: "prompt-1", scope: "project", projectDir: "/tmp/example" }));
  });
});
