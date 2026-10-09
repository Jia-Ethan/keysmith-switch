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
const rewriteConnectClaude = vi.fn();
const rewriteDisconnectClaude = vi.fn();
const rewriteConnectZcode = vi.fn();
const rewriteConnectGrok = vi.fn();
const rewriteDisconnectGrok = vi.fn();
const rewriteDisconnectZcode = vi.fn();
const rewriteSetTableTools = vi.fn();

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
  rewriteConnectClaude: (...a: unknown[]) => rewriteConnectClaude(...a),
  rewriteDisconnectClaude: (...a: unknown[]) => rewriteDisconnectClaude(...a),
  rewriteConnectZcode: (...a: unknown[]) => rewriteConnectZcode(...a),
  rewriteConnectGrok: (...a: unknown[]) => rewriteConnectGrok(...a),
  rewriteDisconnectGrok: (...a: unknown[]) => rewriteDisconnectGrok(...a),
  rewriteDisconnectZcode: (...a: unknown[]) => rewriteDisconnectZcode(...a),
  rewriteSetTableTools: (...a: unknown[]) => rewriteSetTableTools(...a),
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
  grok: {
    link: { state: "unlinked" },
    service: { installed: false, running: false },
    hosts: ["cli-chat-proxy.grok.com"],
    leftOut: [],
    unsupported: null,
    configPath: "/home/.grok/config.toml",
  },
  ...over,
});

const toast = { ok: vi.fn(), err: vi.fn(), info: vi.fn(), toasts: [], dismiss: vi.fn() };

const linkedCodex = (over: Partial<RewriteView["codex"]> = {}) => ({
  ...view().codex,
  link: { state: "linked", provider: "custom" } as const,
  service: { installed: true, running: true },
  ...over,
});

describe("RewritePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    rewriteState.mockResolvedValue(view());
    try {
      localStorage.clear();
    } catch {
      // ignore
    }
  });

  it("starts empty and paused, with one row per agent", async () => {
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-empty")).toHaveTextContent("还没有规则");
    expect(screen.getByTestId("rewrite-master")).not.toBeChecked();
    expect(screen.getByTestId("rewrite-paused")).toBeInTheDocument();
    for (const tool of ["claude", "codex", "zcode", "grok"]) {
      expect(screen.getByTestId(`rewrite-agent-${tool}`)).toBeInTheDocument();
    }
    expect(screen.getByTestId("rewrite-agent-codex")).toHaveAttribute("data-state", "unlinked");
    expect(screen.getByTestId("rewrite-agent-grok")).toHaveAttribute("data-state", "unlinked");
    // No switch for an empty table or an unconnected agent.
    expect(screen.queryByTestId("rewrite-toggle-user")).not.toBeInTheDocument();
    expect(screen.queryByTestId("rewrite-toggle-agent-codex")).not.toBeInTheDocument();
    // Every on/off control is a switch, never a checkbox.
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });

  it("names the master switch apart from the page title", async () => {
    render(<RewritePage toast={toast as never} />);
    const master = await screen.findByRole("switch", { name: "输入替换总开关" });
    expect(master).toBe(screen.getByTestId("rewrite-master"));
  });

  it("turns the master switch on", async () => {
    rewriteSetSwitches.mockResolvedValue(view({ enabled: true }));
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-master"));
    await waitFor(() => expect(rewriteSetSwitches).toHaveBeenCalledWith({ enabled: true }));
    await waitFor(() => expect(screen.getByTestId("rewrite-master")).toBeChecked());
    expect(screen.queryByTestId("rewrite-paused")).not.toBeInTheDocument();
  });

  it("adds, validates and saves rules", async () => {
    rewriteSaveUserRules.mockImplementation(async (rules) => view({ tables: [{ ...mine, rules }] }));
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-add"));
    expect(screen.getByTestId("rewrite-problem")).toHaveTextContent("原文不能为空");
    expect(screen.getByTestId("rewrite-problem")).toHaveAttribute("role", "alert");
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

  it("explains matching only on request", async () => {
    render(<RewritePage toast={toast as never} />);
    await screen.findByTestId("rewrite-empty");
    expect(screen.queryByTestId("rewrite-how-body")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId("rewrite-how"));
    expect(screen.getByTestId("rewrite-how-body")).toHaveTextContent("区分大小写");
  });

  it("connecting says what it writes, then connects", async () => {
    rewriteConnectClaude.mockResolvedValue(
      view({ claude: { ...view().claude, link: { state: "linked" }, service: { installed: true, running: true } } }),
    );
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-connect-claude"));
    expect(screen.getByText(/env\.ANTHROPIC_BASE_URL/)).toBeInTheDocument();
    expect(screen.getByText(/登录令牌会经过本机中转/)).toBeInTheDocument();
    expect(rewriteConnectClaude).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("rewrite-connect-confirm"));
    await waitFor(() => expect(rewriteConnectClaude).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByTestId("rewrite-agent-claude")).toHaveAttribute("data-state", "paused"));
    expect(screen.getByTestId("rewrite-toggle-agent-claude")).toBeInTheDocument();
  });

  it("shows a connected agent as active without exposing its provider id", async () => {
    rewriteState.mockResolvedValue(view({ enabled: true, codex: linkedCodex() }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-codex")).toHaveTextContent("生效中");
    expect(screen.getByTestId("rewrite-agent-codex")).not.toHaveTextContent("custom");
    fireEvent.click(screen.getByTestId("rewrite-details-codex"));
    expect(screen.getByTestId("rewrite-details-body-codex")).toHaveTextContent("config.toml");
  });

  it("an agent switch is off-able on its own and follows the master switch", async () => {
    rewriteState.mockResolvedValue(view({ enabled: true, codex: linkedCodex() }));
    rewriteSetSwitches.mockResolvedValue(view({ enabled: true, codexEnabled: false, codex: linkedCodex() }));
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-toggle-agent-codex"));
    await waitFor(() => expect(rewriteSetSwitches).toHaveBeenCalledWith({ codexEnabled: false }));
    await waitFor(() => expect(screen.getByTestId("rewrite-status-codex")).toHaveTextContent("已暂停"));
  });

  it("blocks connecting an unsupported setup and says why", async () => {
    rewriteState.mockResolvedValue(view({ codex: { ...view().codex, provider: null, unsupported: "built-in-provider" } }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-codex")).toHaveTextContent("内置的 OpenAI");
    expect(screen.getByTestId("rewrite-connect-codex")).toBeDisabled();
  });

  it("warns when an agent bypasses the relay and offers to reconnect", async () => {
    rewriteState.mockResolvedValue(
      view({ codex: { ...view().codex, link: { state: "bypassed", provider: "other" }, service: { installed: true, running: true } } }),
    );
    rewriteConnectCodex.mockResolvedValue(view({ codex: linkedCodex() }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-codex")).toHaveTextContent("被绕过");
    fireEvent.click(screen.getByTestId("rewrite-reconnect-codex"));
    // Repairing an existing connection does not ask again.
    await waitFor(() => expect(rewriteConnectCodex).toHaveBeenCalled());
  });

  it("warns when linked but the relay is down", async () => {
    rewriteState.mockResolvedValue(view({ codex: linkedCodex({ service: { installed: true, running: false } }) }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-codex")).toHaveTextContent("连不上模型");
    expect(screen.getByTestId("rewrite-reconnect-codex")).toHaveTextContent("重新启动");
  });

  it("asks ZCode users to reconnect when new providers are not routed", async () => {
    rewriteState.mockResolvedValue(
      view({ enabled: true, zcode: { ...view().zcode, link: { state: "linked", unrouted: 2 }, service: { installed: true, running: true } } }),
    );
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-zcode")).toHaveTextContent("2 个新加的 provider");
    expect(screen.getByTestId("rewrite-reconnect-zcode")).toBeInTheDocument();
  });

  it("connects Grok, saying what it writes and that the sign-in token passes through", async () => {
    rewriteConnectGrok.mockResolvedValue(
      view({ grok: { ...view().grok, link: { state: "linked", unrouted: 0, clashing: 0 }, service: { installed: true, running: true } } }),
    );
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-connect-grok"));
    expect(screen.getByText(/model\.\*\.base_url/)).toBeInTheDocument();
    expect(screen.getByText(/登录令牌会经过本机中转/)).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("rewrite-connect-confirm"));
    await waitFor(() => expect(rewriteConnectGrok).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByTestId("rewrite-agent-grok")).toHaveAttribute("data-state", "paused"));
  });

  it("asks Grok users to reconnect when new models are not routed", async () => {
    rewriteState.mockResolvedValue(
      view({ enabled: true, grok: { ...view().grok, link: { state: "linked", unrouted: 2, clashing: 0 }, service: { installed: true, running: true } } }),
    );
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-grok")).toHaveTextContent("2 个新加的 模型");
    expect(screen.getByTestId("rewrite-reconnect-grok")).toBeInTheDocument();
  });

  it("warns Grok users when a custom model would be sent the sign-in token, and offers to reconnect", async () => {
    rewriteState.mockResolvedValue(
      view({
        enabled: true,
        grok: { ...view().grok, link: { state: "linked", unrouted: 0, clashing: 1 }, service: { installed: true, running: true } },
      }),
    );
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-grok")).toHaveTextContent("登录令牌会发到它的地址");
    expect(screen.getByTestId("rewrite-reconnect-grok")).toBeInTheDocument();
  });

  it("lists the models kept off the relay because a custom model uses them as its upstream", async () => {
    rewriteState.mockResolvedValue(
      view({
        enabled: true,
        grok: {
          ...view().grok,
          link: { state: "linked", unrouted: 0, clashing: 0 },
          service: { installed: true, running: true },
          leftOut: ["grok-4.7"],
        },
      }),
    );
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-grok")).toHaveTextContent("生效中");
    fireEvent.click(screen.getByTestId("rewrite-details-grok"));
    expect(screen.getByTestId("rewrite-details-body-grok")).toHaveTextContent("未经过中转：grok-4.7");
    expect(screen.queryByTestId("rewrite-reconnect-grok")).not.toBeInTheDocument();
  });

  it("explains why Grok cannot be connected when every model is spoken for", async () => {
    rewriteState.mockResolvedValue(view({ grok: { ...view().grok, hosts: [], unsupported: "no-models" } }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-grok")).toHaveTextContent("没有可连接的模型");
    expect(screen.getByTestId("rewrite-connect-grok")).toBeDisabled();
  });

  it("tells Grok users to run Grok once when it has no model list yet", async () => {
    rewriteState.mockResolvedValue(view({ grok: { ...view().grok, hosts: [], unsupported: "no-catalog" } }));
    render(<RewritePage toast={toast as never} />);
    expect(await screen.findByTestId("rewrite-status-grok")).toHaveTextContent("请先运行一次 Grok");
    expect(screen.getByTestId("rewrite-connect-grok")).toBeDisabled();
  });

  it("disconnect is a danger action that asks first and says rules stay", async () => {
    rewriteState.mockResolvedValue(view({ codex: linkedCodex() }));
    rewriteDisconnectCodex.mockResolvedValue(view());
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-disconnect-codex"));
    expect(screen.getByText(/只还原 .*config\.toml 里由 Keysmith 写入的那一项.*规则保留/)).toBeInTheDocument();
    expect(screen.getByTestId("rewrite-disconnect-codex").className).toContain("text-destructive");
    expect(rewriteDisconnectCodex).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("rewrite-disconnect-confirm"));
    await waitFor(() => expect(rewriteDisconnectCodex).toHaveBeenCalled());
  });

  it("scopes a table to some agents", async () => {
    const table = { ...mine, rules: [{ from: "a", to: "b" }] };
    rewriteState.mockResolvedValue(view({ tables: [table] }));
    rewriteSetTableTools.mockResolvedValue(view({ tables: [{ ...table, tools: ["claude", "zcode", "grok"] }] }));
    render(<RewritePage toast={toast as never} />);
    const codexChip = await screen.findByTestId("rewrite-scope-user-codex");
    expect(codexChip).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(codexChip);
    await waitFor(() => expect(rewriteSetTableTools).toHaveBeenCalledWith("user", ["claude", "zcode", "grok"]));
  });

  it("refuses to scope a table to no agent", async () => {
    const table = { ...mine, rules: [{ from: "a", to: "b" }], tools: ["codex" as const] };
    rewriteState.mockResolvedValue(view({ tables: [table] }));
    render(<RewritePage toast={toast as never} />);
    fireEvent.click(await screen.findByTestId("rewrite-scope-user-codex"));
    expect(screen.getByText(/至少选择一个 Agent/)).toBeInTheDocument();
    expect(rewriteSetTableTools).not.toHaveBeenCalled();
  });

  it("filters tables by agent and searches rules", async () => {
    rewriteState.mockResolvedValue(
      view({ tables: [{ ...mine, rules: [{ from: "apple", to: "1" }, { from: "pear", to: "2" }] }, pack({ tools: ["codex"] })] }),
    );
    render(<RewritePage toast={toast as never} />);
    await screen.findByTestId("rewrite-table-p.one");
    fireEvent.click(screen.getByTestId("rewrite-filter"));
    fireEvent.click(await screen.findByRole("option", { name: "Claude Code" }));
    expect(screen.queryByTestId("rewrite-table-p.one")).not.toBeInTheDocument();
    expect(screen.getByTestId("rewrite-table-user")).toBeInTheDocument();
    fireEvent.change(screen.getByTestId("rewrite-search"), { target: { value: "pea" } });
    expect(screen.queryByDisplayValue("apple")).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("pear")).toBeInTheDocument();
  });

  it("folds long tables", async () => {
    const many = Array.from({ length: 25 }, (_, i) => ({ from: `w${i}`, to: `${i}` }));
    rewriteState.mockResolvedValue(view({ tables: [{ ...mine, rules: many }] }));
    render(<RewritePage toast={toast as never} />);
    await screen.findByDisplayValue("w0");
    expect(screen.queryByDisplayValue("w24")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId("rewrite-show-all"));
    expect(screen.getByDisplayValue("w24")).toBeInTheDocument();
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
    fireEvent.click(screen.getByRole("switch", { name: "启用Pack one" }));
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
