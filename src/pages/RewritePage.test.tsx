import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RewriteView, RuleTable } from "../types";
import { RewritePage, ruleDiff } from "./RewritePage";

const rewriteState = vi.fn();
const rewriteSetSwitches = vi.fn();
const rewriteSaveUserRules = vi.fn();
const rewriteSetTableEnabled = vi.fn();
const rewriteReorderTables = vi.fn();
const rewriteCopyToUser = vi.fn();
const rewriteAcceptUpdate = vi.fn();
const rewriteConnectCodex = vi.fn();
const rewriteDisconnectCodex = vi.fn();

vi.mock("../api", () => ({
  rewriteState: (...a: unknown[]) => rewriteState(...a),
  rewriteSetSwitches: (...a: unknown[]) => rewriteSetSwitches(...a),
  rewriteSaveUserRules: (...a: unknown[]) => rewriteSaveUserRules(...a),
  rewriteSetTableEnabled: (...a: unknown[]) => rewriteSetTableEnabled(...a),
  rewriteReorderTables: (...a: unknown[]) => rewriteReorderTables(...a),
  rewriteCopyToUser: (...a: unknown[]) => rewriteCopyToUser(...a),
  rewriteAcceptUpdate: (...a: unknown[]) => rewriteAcceptUpdate(...a),
  rewriteConnectCodex: (...a: unknown[]) => rewriteConnectCodex(...a),
  rewriteDisconnectCodex: (...a: unknown[]) => rewriteDisconnectCodex(...a),
}));

const mine: RuleTable = {
  id: "user",
  kind: "user",
  title: "",
  enabled: true,
  priority: 0,
  packId: null,
  packVersion: null,
  tools: null,
  rules: [],
  pending: null,
};

const pack = (over: Partial<RuleTable> = {}): RuleTable => ({
  id: "pack:p.one",
  kind: "pack",
  title: "Pack one",
  enabled: true,
  priority: 1,
  packId: "p.one",
  packVersion: "1.0.0",
  tools: ["codex"],
  rules: [{ from: "a", to: "1" }],
  pending: null,
  ...over,
});

const view = (over: Partial<RewriteView> = {}): RewriteView => ({
  enabled: false,
  codexEnabled: true,
  claudeEnabled: true,
  grokEnabled: true,
  zcodeEnabled: true,
  tables: [mine],
  codex: {
    link: { state: "unlinked" },
    service: { installed: false, running: false },
    provider: { id: "custom", name: "custom", baseUrl: "https://x/v1" },
    unsupported: null,
    codexDir: "/home/.codex",
  },
  claude: {
    link: { state: "unlinked" },
    service: { installed: false, running: false },
    upstreamHost: "api.anthropic.com",
    unsupported: null,
    settingsPath: "/home/.claude/settings.json",
  },
  zcode: {
    link: { state: "unlinked" },
    service: { installed: false, running: false },
    providers: [{ name: "Gateway", host: "gateway.example" }],
    unsupported: null,
    configPath: "/home/.zcode/v2/provider_config.json",
  },
  ...over,
});

const toast = { ok: vi.fn(), err: vi.fn(), info: vi.fn(), toasts: [], dismiss: vi.fn() };

describe("RewritePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    rewriteState.mockResolvedValue(view());
  });

  it("starts with no rules and explains matching", async () => {
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-empty")).toHaveTextContent("区分大小写");
    expect(screen.getByTestId("rewrite-master")).not.toBeChecked();
    expect(screen.getByTestId("rewrite-codex-connection")).toHaveAttribute("data-state", "unlinked");
  });

  it("turns the master switch on", async () => {
    rewriteSetSwitches.mockResolvedValue(view({ enabled: true }));
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-master"));
    await waitFor(() => expect(rewriteSetSwitches).toHaveBeenCalledWith({ enabled: true }));
    await waitFor(() => expect(screen.getByTestId("rewrite-master")).toBeChecked());
  });

  it("adds, validates and saves rules", async () => {
    rewriteSaveUserRules.mockImplementation(async (rules) => view({ tables: [{ ...mine, rules }] }));
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-add"));
    expect(screen.getByTestId("rewrite-problem")).toHaveTextContent("原文不能为空");
    expect(screen.getByTestId("rewrite-save")).toBeDisabled();
    fireEvent.change(screen.getByTestId("rewrite-from-0"), { target: { value: "提示词" } });
    fireEvent.change(screen.getByTestId("rewrite-to-0"), { target: { value: "指令" } });
    fireEvent.click(screen.getByTestId("rewrite-add"));
    fireEvent.change(screen.getByTestId("rewrite-from-1"), { target: { value: "提示词" } });
    expect(screen.getByTestId("rewrite-problem")).toHaveTextContent("重复");
    fireEvent.click(screen.getByTestId("rewrite-remove-1"));
    fireEvent.click(screen.getByTestId("rewrite-save"));
    await waitFor(() => expect(rewriteSaveUserRules).toHaveBeenCalledWith([{ from: "提示词", to: "指令" }]));
    await waitFor(() => expect(screen.queryByTestId("rewrite-save")).not.toBeInTheDocument());
  });

  it("strips line breaks from the matched text", async () => {
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-add"));
    fireEvent.change(screen.getByTestId("rewrite-from-0"), { target: { value: "a\nb" } });
    expect(screen.getByTestId("rewrite-from-0")).toHaveValue("ab");
  });

  it("connects Codex", async () => {
    rewriteConnectCodex.mockResolvedValue(
      view({ codex: { ...view().codex, link: { state: "linked", provider: "custom" }, service: { installed: true, running: true } } }),
    );
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-connect"));
    expect(await screen.findByTestId("rewrite-linked")).toHaveTextContent("custom");
    expect(screen.queryByTestId("rewrite-connect")).not.toBeInTheDocument();
  });

  it("blocks connecting an unsupported provider and says why", async () => {
    rewriteState.mockResolvedValue(view({ codex: { ...view().codex, provider: null, unsupported: "built-in-provider" } }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-unsupported")).toHaveTextContent("内置的 OpenAI");
    expect(screen.getByTestId("rewrite-connect")).toBeDisabled();
  });

  it("warns when Codex bypasses the relay and offers to reconnect", async () => {
    rewriteState.mockResolvedValue(
      view({ codex: { ...view().codex, link: { state: "bypassed", provider: "other" }, service: { installed: true, running: true } } }),
    );
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-bypassed")).toBeInTheDocument();
    expect(screen.getByTestId("rewrite-connect")).toHaveTextContent("重新连接");
  });

  it("warns when linked but the relay is down", async () => {
    rewriteState.mockResolvedValue(
      view({ codex: { ...view().codex, link: { state: "linked", provider: "custom" }, service: { installed: true, running: false } } }),
    );
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-down")).toHaveTextContent("连不上模型");
  });

  it("disconnect asks first and says rules stay", async () => {
    rewriteState.mockResolvedValue(
      view({ codex: { ...view().codex, link: { state: "linked", provider: "custom" }, service: { installed: true, running: true } } }),
    );
    rewriteDisconnectCodex.mockResolvedValue(view());
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-disconnect"));
    expect(screen.getByText(/规则会保留/)).toBeInTheDocument();
    expect(rewriteDisconnectCodex).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("rewrite-disconnect-confirm"));
    await waitFor(() => expect(rewriteDisconnectCodex).toHaveBeenCalled());
  });

  it("shows a pending pack update and accepts it after review", async () => {
    const table = pack({ pending: { version: "1.1.0", title: "Pack one", rules: [{ from: "a", to: "2" }, { from: "b", to: "" }] } });
    rewriteState.mockResolvedValue(view({ tables: [mine, table] }));
    rewriteAcceptUpdate.mockResolvedValue(view({ tables: [mine, pack({ packVersion: "1.1.0" })] }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-pending-p.one")).toHaveTextContent("1.1.0");
    fireEvent.click(screen.getByTestId("rewrite-review-p.one"));
    expect(screen.getByTestId("rewrite-diff")).toHaveTextContent("新增");
    expect(screen.getByTestId("rewrite-diff")).toHaveTextContent("改动");
    fireEvent.click(screen.getByTestId("rewrite-accept"));
    await waitFor(() => expect(rewriteAcceptUpdate).toHaveBeenCalledWith("pack:p.one"));
  });

  it("copies a pack to my rules and toggles it", async () => {
    rewriteState.mockResolvedValue(view({ tables: [mine, pack()] }));
    rewriteCopyToUser.mockResolvedValue(view({ tables: [{ ...mine, rules: [{ from: "a", to: "1" }] }, pack()] }));
    rewriteSetTableEnabled.mockResolvedValue(view({ tables: [mine, pack({ enabled: false })] }));
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-copy-p.one"));
    await waitFor(() => expect(rewriteCopyToUser).toHaveBeenCalledWith("pack:p.one"));
    fireEvent.click(screen.getByTestId("rewrite-toggle-p.one"));
    await waitFor(() => expect(rewriteSetTableEnabled).toHaveBeenCalledWith("pack:p.one", false));
  });
});

describe("ruleDiff", () => {
  it("separates added, removed and changed", () => {
    expect(
      ruleDiff(
        [
          { from: "a", to: "1" },
          { from: "b", to: "2" },
        ],
        [
          { from: "a", to: "9" },
          { from: "c", to: "3" },
        ],
      ),
    ).toEqual({
      added: [{ from: "c", to: "3" }],
      removed: [{ from: "b", to: "2" }],
      changed: [{ from: "a", before: "1", after: "9" }],
    });
  });
});
