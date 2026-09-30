import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetHarnessStatuses } from "../lib/harnessState";
import type { PromptSummary } from "../types";

const createPastedPrompt = vi.fn();
const planActivate = vi.fn();
const activate = vi.fn();
const planDeactivate = vi.fn();
const deactivate = vi.fn();
const getHarnessState = vi.fn();
const listTools = vi.fn();
const toolStatus = vi.fn();
const listPrompts = vi.fn();
const listActivations = vi.fn();
const listOperations = vi.fn();
const recoverTool = vi.fn();
const confirmRecover = vi.fn();

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
  listPrompts: (...args: unknown[]) => listPrompts(...args),
  listActivations: (...args: unknown[]) => listActivations(...args),
  listOperations: (...args: unknown[]) => listOperations(...args),
  recoverTool: (...args: unknown[]) => recoverTool(...args),
  confirmRecover: (...args: unknown[]) => confirmRecover(...args),
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

function prompt(overrides: Partial<PromptSummary>): PromptSummary {
  return {
    id: "p1",
    tool: "codex",
    title: "Alpha",
    tags: [],
    active: false,
    lastUsedAt: null,
    updatedAt: "2026-08-21T00:00:00Z",
    createdAt: "2026-08-21T00:00:00Z",
    excerpt: null,
    ...overrides,
  };
}

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
  listPrompts.mockResolvedValue({ prompts: [] });
  listActivations.mockResolvedValue({ activations: [] });
  listOperations.mockResolvedValue({ operations: [] });
}

async function renderPage(props: Partial<Parameters<typeof import("./WorkspacePage").WorkspacePage>[0]> = {}) {
  const { WorkspacePage } = await import("./WorkspacePage");
  return render(<WorkspacePage tool="codex" {...props} />);
}

async function openComposer() {
  await waitFor(() => expect(screen.getByTestId("quick-deploy-open")).toBeEnabled());
  fireEvent.click(screen.getByTestId("quick-deploy-open"));
  return screen.getByTestId("quick-deploy-panel");
}

function fillDraft(title: string, content: string) {
  fireEvent.change(screen.getByTestId("quick-deploy-title"), { target: { value: title } });
  fireEvent.change(screen.getByRole("textbox", { name: "提示词内容" }), { target: { value: content } });
}

describe("Workspace: Quick Deploy composer", () => {
  beforeEach(() => {
    resetHarnessStatuses();
    vi.clearAllMocks();
    setup();
  });

  it("reads the machine once per run and opens an editable composer", async () => {
    const { unmount } = await renderPage();
    expect(await screen.findByRole("heading", { name: "Codex" })).toBeInTheDocument();
    await waitFor(() => expect(getHarnessState).toHaveBeenCalledTimes(1));
    await openComposer();
    expect(screen.getByTestId("quick-deploy-title")).toHaveValue("粘贴的提示词");
    expect(screen.getByRole("textbox", { name: "提示词内容" })).toBeInTheDocument();
    expect(screen.getByTestId("quick-deploy-submit")).toHaveTextContent("生成部署预览");
    unmount();
    await renderPage();
    expect(getHarnessState).toHaveBeenCalledTimes(1);
  });

  it("requires pasted content before creating a prompt", async () => {
    await renderPage();
    await openComposer();
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    expect(createPastedPrompt).not.toHaveBeenCalled();
    expect(screen.getByTestId("quick-deploy-status")).toHaveTextContent("请输入标题和提示词内容。");
  });

  it("saves, previews, confirms, and celebrates a pasted prompt", async () => {
    await renderPage();
    await openComposer();
    fillDraft("My rules", "Be concise.");
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    await waitFor(() => expect(createPastedPrompt).toHaveBeenCalledWith({ tool: "codex", title: "My rules", content: "Be concise." }));
    expect(planActivate).toHaveBeenCalledWith({ promptId: "prompt-1", scope: "user", projectDir: undefined });
    fireEvent.click(await screen.findByTestId("quick-deploy-confirm"));
    await waitFor(() => expect(activate).toHaveBeenCalledWith("op-1"));
    await waitFor(() => expect(screen.getByTestId("quick-deploy-remove")).toBeInTheDocument());
    expect(screen.getByTestId("deploy-celebration")).toHaveTextContent("已部署到 Codex");
    expect(screen.queryByTestId("quick-deploy-panel")).not.toBeInTheDocument();
    expect(screen.getByTestId("agent-status")).toHaveTextContent("当前已部署");
    expect(getHarnessState).toHaveBeenCalledTimes(1);
  });

  it("saves a draft to the library without planning a deployment", async () => {
    const toast = { ok: vi.fn(), err: vi.fn(), info: vi.fn(), toasts: [], dismiss: vi.fn() };
    await renderPage({ toast });
    await openComposer();
    fillDraft("Keep me", "Draft body");
    fireEvent.click(screen.getByTestId("quick-deploy-save"));
    await waitFor(() => expect(createPastedPrompt).toHaveBeenCalledWith({ tool: "codex", title: "Keep me", content: "Draft body" }));
    expect(planActivate).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByTestId("quick-deploy-panel")).not.toBeInTheDocument());
    expect(toast.ok).toHaveBeenCalledWith("已保存到提示词库");
    await waitFor(() => expect(listPrompts.mock.calls.length).toBeGreaterThanOrEqual(2));
  });

  it("imports a dropped-in markdown file into an empty draft and names it", async () => {
    await renderPage();
    await openComposer();
    const file = new File(["# Imported\n\nRules."], "team-rules.md", { type: "text/markdown" });
    fireEvent.change(screen.getByTestId("quick-deploy-file"), { target: { files: [file] } });
    await waitFor(() => expect(screen.getByRole("textbox", { name: "提示词内容" })).toHaveValue("# Imported\n\nRules."));
    expect(screen.getByTestId("quick-deploy-title")).toHaveValue("team rules");
    expect(screen.getByTestId("quick-deploy-stats")).toHaveTextContent("18 字符 · 3 行");
  });

  it("rejects files that are not text prompts", async () => {
    await renderPage();
    await openComposer();
    const file = new File(["binary"], "logo.png", { type: "image/png" });
    fireEvent.change(screen.getByTestId("quick-deploy-file"), { target: { files: [file] } });
    expect(await screen.findByRole("alert")).toHaveTextContent("只支持 .md / .markdown / .txt 文本文件。");
    expect(screen.getByRole("textbox", { name: "提示词内容" })).toHaveValue("");
  });

  it("has no keyboard shortcuts for the composer", async () => {
    await renderPage();
    await waitFor(() => expect(screen.getByTestId("quick-deploy-open")).toBeEnabled());
    fireEvent.keyDown(window, { key: "n", ctrlKey: true });
    fireEvent.keyDown(window, { key: "n", metaKey: true });
    expect(screen.queryByTestId("quick-deploy-panel")).not.toBeInTheDocument();

    const panel = await openComposer();
    fillDraft("My rules", "Be concise.");
    fireEvent.keyDown(within(panel).getByRole("textbox", { name: "提示词内容" }), { key: "Enter", ctrlKey: true });
    fireEvent.keyDown(within(panel).getByRole("textbox", { name: "提示词内容" }), { key: "Enter", metaKey: true });
    expect(planActivate).not.toHaveBeenCalled();
    expect(within(panel).getByTestId("quick-deploy-submit")).not.toHaveTextContent("↵");
  });

  it("shows remove after a deployed state and uses the standard deactivation plan", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null });
    await renderPage();
    fireEvent.click(await screen.findByTestId("quick-deploy-remove"));
    expect(await screen.findByTestId("quick-deploy-confirm")).toBeInTheDocument();
    expect(planDeactivate).toHaveBeenCalledWith({ tool: "codex", scope: "user", projectDir: undefined });
  });

  it("blocks a preview with adapter blockers before execution", async () => {
    planActivate.mockResolvedValue({ operationId: "blocked", envelope: { ...envelope, ok: false, blockers: ["conflict"] } });
    await renderPage();
    await openComposer();
    fillDraft("My rules", "Be concise.");
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    const confirm = await screen.findByTestId("quick-deploy-confirm");
    expect(confirm).toBeDisabled();
    expect(activate).not.toHaveBeenCalled();
  });

  it("offers recovery inside a plan that found drift", async () => {
    planActivate.mockResolvedValue({ operationId: "drift", envelope: { ...envelope, status: "drift" } });
    recoverTool.mockResolvedValue({ operationId: "op-r", envelope: { ...envelope, command: "plan-recover" } });
    await renderPage();
    await openComposer();
    fillDraft("My rules", "Be concise.");
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    expect(await screen.findByTestId("quick-deploy-confirm")).toBeDisabled();
    fireEvent.click(screen.getByTestId("plan-recover"));
    await waitFor(() => expect(recoverTool).toHaveBeenCalledWith({ tool: "codex", scope: "user", projectDir: undefined }));
    await waitFor(() => expect(screen.getByTestId("quick-deploy-confirm")).toBeEnabled());
    fireEvent.click(screen.getByTestId("quick-deploy-confirm"));
    await waitFor(() => expect(confirmRecover).toHaveBeenCalledWith("op-r"));
  });

  it("keeps a saved draft clean after preview cancellation and failed retry", async () => {
    const onDirtyChange = vi.fn();
    await renderPage({ onDirtyChange });
    await openComposer();
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

  it("keeps an unsent draft when the composer is closed", async () => {
    const onDirtyChange = vi.fn();
    await renderPage({ onDirtyChange });
    await openComposer();
    fillDraft("Later", "Not yet.");
    fireEvent.click(screen.getByTestId("quick-deploy-close"));
    expect(screen.queryByTestId("quick-deploy-panel")).not.toBeInTheDocument();
    expect(onDirtyChange).toHaveBeenLastCalledWith(true);
    fireEvent.click(screen.getByTestId("quick-deploy-open"));
    expect(screen.getByTestId("quick-deploy-title")).toHaveValue("Later");
  });

  it("shows reused library metadata in the confirmation", async () => {
    createPastedPrompt.mockResolvedValue({ id: "existing-1", title: "Original import" });
    await renderPage();
    await openComposer();
    fillDraft("New title", "Be concise.");
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    await waitFor(() => expect(planActivate).toHaveBeenCalledWith({ promptId: "existing-1", scope: "user", projectDir: undefined }));
    expect(screen.getByRole("dialog", { name: "快速部署预览" })).toHaveTextContent("Original import");
  });

  it("refreshes user status explicitly without a second initial read", async () => {
    await renderPage();
    await waitFor(() => expect(screen.getByTestId("agent-status")).toHaveTextContent("当前未部署"));
    expect(getHarnessState).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByTestId("harness-refresh")).toBeEnabled());
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null });
    fireEvent.click(screen.getByTestId("harness-refresh"));
    await waitFor(() => expect(getHarnessState).toHaveBeenCalledTimes(2));
    expect(await screen.findByTestId("quick-deploy-remove")).toBeInTheDocument();
  });

  it("deploys machine-wide and shows no scope controls, even for Claude", async () => {
    await renderPage({ tool: "claude" });
    await waitFor(() => expect(screen.getByTestId("agent-status")).toHaveTextContent("当前未部署"));
    expect(screen.queryByRole("radiogroup", { name: "当前目标范围" })).not.toBeInTheDocument();
    expect(screen.queryByTestId("scope-project-dir")).not.toBeInTheDocument();
    expect(screen.getByTestId("agent-hero")).not.toHaveTextContent("范围");
    await openComposer();
    expect(screen.getByTestId("quick-deploy-panel")).not.toHaveTextContent("范围");
    fillDraft("Global rules", "Use tests.");
    fireEvent.click(screen.getByTestId("quick-deploy-submit"));
    await waitFor(() => expect(planActivate).toHaveBeenCalledWith({ promptId: "prompt-1", scope: "user" }));
    expect(toolStatus).not.toHaveBeenCalled();
  });

  it("shows the remove button as working while its plan is prepared", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null, promptId: "a", promptTitle: "Live one" });
    let release!: (value: unknown) => void;
    planDeactivate.mockReturnValue(new Promise((resolve) => { release = resolve; }));
    await renderPage();
    const remove = await screen.findByTestId("quick-deploy-remove");
    expect(remove).not.toHaveAttribute("aria-busy");
    fireEvent.click(remove);
    await waitFor(() => expect(screen.getByTestId("quick-deploy-remove")).toHaveAttribute("aria-busy", "true"));
    fireEvent.click(screen.getByTestId("quick-deploy-remove"));
    expect(planDeactivate).toHaveBeenCalledTimes(1);
    await act(async () => release({ operationId: "op-2", envelope: { ...envelope, command: "plan-deactivate" } }));
    expect(await screen.findByTestId("quick-deploy-confirm")).toBeInTheDocument();
    expect(screen.getByTestId("quick-deploy-remove")).not.toHaveAttribute("aria-busy");
  });

  it("confirms a removal with its own quiet animation and clears the live prompt", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null, promptId: "a", promptTitle: "Live one" });
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "a", title: "Live one" })] });
    await renderPage();
    expect(await screen.findByTestId("agent-deployed-title")).toHaveTextContent("Live one");
    fireEvent.click(await screen.findByTestId("quick-deploy-remove"));
    fireEvent.click(await screen.findByTestId("quick-deploy-confirm"));
    await waitFor(() => expect(deactivate).toHaveBeenCalledWith("op-2"));
    const done = await screen.findByTestId("deploy-celebration");
    expect(done).toHaveAttribute("data-variant", "removed");
    expect(done).toHaveTextContent("已从 Codex 移除");
    await waitFor(() => expect(screen.getByTestId("workspace-page")).toHaveAttribute("data-machine", "undeployed"));
    expect(screen.queryByTestId("agent-deployed-title")).not.toBeInTheDocument();
    expect(screen.queryByTestId("quick-deploy-remove")).not.toBeInTheDocument();
  });
});

describe("Workspace: prompt library", () => {
  beforeEach(() => {
    resetHarnessStatuses();
    vi.clearAllMocks();
    setup();
  });

  it("leaves the loading state after listPrompts resolves", async () => {
    await renderPage();
    expect(screen.getByTestId("prompt-list-loading")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByTestId("prompt-list-empty")).toBeInTheDocument());
    expect(screen.queryByTestId("prompt-list-loading")).not.toBeInTheDocument();
  });

  it("routes prompt selection even when scoped activation state is unreadable", async () => {
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "p1" })] });
    listActivations.mockRejectedValue(new Error("activation table unavailable"));
    const onNavigate = vi.fn();
    await renderPage({ tool: "claude", onNavigate });
    expect(await screen.findByTestId("prompt-activation-unknown")).toBeInTheDocument();
    fireEvent.click(await screen.findByTestId("prompt-item-p1"));
    expect(onNavigate).toHaveBeenCalledWith({
      kind: "prompt-view",
      tool: "claude",
      promptId: "p1",
      scope: "user",
      projectDir: "",
    });
  });

  it("pins the deployed prompt and names it in the hero", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null });
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "a", title: "Live one" }), prompt({ id: "b", title: "Spare" })] });
    listActivations.mockResolvedValue({
      activations: [
        { id: "x", tool: "codex", promptId: "a", promptTitle: "Live one", scope: "user", projectDir: null, active: true, createdAt: "", fingerprint: null },
      ],
    });
    await renderPage();
    expect(await screen.findByTestId("agent-deployed-title")).toHaveTextContent("Live one");
    const liveGroup = (await screen.findByText("当前部署")).closest("section");
    expect(liveGroup).toContainElement(screen.getByTestId("prompt-item-a"));
    expect(screen.queryByTestId("prompt-deploy-a")).not.toBeInTheDocument();
    expect(screen.getByTestId("prompt-deploy-b")).toBeInTheDocument();
  });

  it("deploys a library prompt in one click through a reviewed plan", async () => {
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "b", title: "Spare" })] });
    await renderPage();
    fireEvent.click(await screen.findByTestId("prompt-deploy-b"));
    await waitFor(() => expect(planActivate).toHaveBeenCalledWith({ promptId: "b", scope: "user" }));
    expect(createPastedPrompt).not.toHaveBeenCalled();
    const dialog = await screen.findByRole("dialog", { name: "快速部署预览" });
    expect(dialog).toHaveTextContent("Spare");
    fireEvent.click(screen.getByTestId("quick-deploy-confirm"));
    await waitFor(() => expect(activate).toHaveBeenCalledWith("op-1"));
    await waitFor(() => expect(screen.getByTestId("workspace-page")).toHaveAttribute("data-machine", "deployed"));
    expect(await screen.findByTestId("deploy-celebration")).toHaveTextContent("Spare");
  });

  it("filters by tag chips and has no slash shortcut for search", async () => {
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "a", tags: ["ops"] })] });
    await renderPage();
    const chip = await screen.findByRole("button", { name: "#ops" });
    fireEvent.click(chip);
    await waitFor(() => expect(listPrompts).toHaveBeenLastCalledWith({ tool: "codex", query: undefined, tag: "ops", sort: "lastUsed" }));
    expect(chip).toHaveAttribute("aria-pressed", "true");
    fireEvent.keyDown(window, { key: "/" });
    expect(screen.getByTestId("prompt-search")).not.toHaveFocus();
  });

  it("names the live prompt from the machine read when no activation record exists", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null, promptId: "b", promptTitle: "Spare" });
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "a", title: "Other" }), prompt({ id: "b", title: "Spare" })] });
    listActivations.mockResolvedValue({ activations: [] });
    await renderPage();
    expect(await screen.findByTestId("agent-deployed-title")).toHaveTextContent("Spare");
    const liveGroup = (await screen.findByText("当前部署")).closest("section");
    expect(liveGroup).toContainElement(screen.getByTestId("prompt-item-b"));
    expect(screen.queryByTestId("prompt-deploy-b")).not.toBeInTheDocument();
    expect(screen.getByTestId("prompt-deploy-a")).toBeInTheDocument();
  });

  it("says so when something is deployed but no library prompt can be matched", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null });
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "a", title: "Other" })] });
    await renderPage();
    await waitFor(() => expect(screen.getByTestId("agent-status")).toHaveTextContent("当前已部署"));
    expect(await screen.findByTestId("agent-deployed-unknown")).toHaveTextContent("来源未记录");
    expect(screen.queryByTestId("agent-deployed-title")).not.toBeInTheDocument();
  });

  it("shows a live card when the machine is deployed but the library is empty", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null });
    listPrompts.mockResolvedValue({ prompts: [] });
    await renderPage();
    const card = await screen.findByTestId("prompt-live-unrecorded");
    expect(card).toHaveTextContent("部署中");
    expect(screen.queryByTestId("prompt-list-empty")).not.toBeInTheDocument();
  });

  it("shows no live card when the live prompt is in the library or nothing is deployed", async () => {
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: true, error: null, promptId: "b", promptTitle: "Spare" });
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "b", title: "Spare" })] });
    const view = await renderPage();
    expect(await screen.findByTestId("agent-deployed-title")).toHaveTextContent("Spare");
    expect(screen.queryByTestId("prompt-live-unrecorded")).not.toBeInTheDocument();
    view.unmount();

    resetHarnessStatuses();
    getHarnessState.mockResolvedValue({ tool: "codex", deployed: false, error: null });
    listPrompts.mockResolvedValue({ prompts: [] });
    await renderPage();
    expect(await screen.findByTestId("prompt-list-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("prompt-live-unrecorded")).not.toBeInTheDocument();
  });

  it("shows the deploy button as working while its plan is prepared", async () => {
    listPrompts.mockResolvedValue({ prompts: [prompt({ id: "b", title: "Spare" })] });
    let release!: (value: unknown) => void;
    planActivate.mockReturnValue(new Promise((resolve) => { release = resolve; }));
    await renderPage();
    fireEvent.click(await screen.findByTestId("prompt-deploy-b"));
    await waitFor(() => expect(screen.getByTestId("prompt-deploy-b")).toHaveAttribute("aria-busy", "true"));
    fireEvent.click(screen.getByTestId("prompt-deploy-b"));
    expect(planActivate).toHaveBeenCalledTimes(1);
    await act(async () => release({ operationId: "op-1", envelope }));
    expect(await screen.findByTestId("quick-deploy-confirm")).toBeInTheDocument();
    expect(screen.getByTestId("prompt-deploy-b")).not.toHaveAttribute("aria-busy");
  });
});
