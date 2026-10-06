import type { RewriteView, ToolId } from "../../types";

/** The agents in the order the page lists them. */
export const REWRITE_AGENTS: ToolId[] = ["claude", "codex", "zcode", "grok"];

export const AGENT_NAMES: Record<ToolId, string> = {
  claude: "Claude Code",
  codex: "Codex",
  grok: "Grok Build",
  zcode: "ZCode",
};

export type AgentStatus = "active" | "paused" | "unlinked" | "bypassed" | "down" | "unsupported";

/** What one agent row shows: state, what connecting writes, where requests go, why not. */
export interface AgentRow {
  tool: ToolId;
  status: AgentStatus;
  /** The per-agent switch; shown only once the agent is connected. */
  enabled: boolean;
  /** A connection exists (linked, bypassed or down), so it can be disconnected. */
  connected: boolean;
  canConnect: boolean;
  /** i18n key under rewrite.unsupported for why it cannot be connected. */
  unsupported: string | null;
  /** i18n key under rewrite.bypassed when bypassed. */
  bypassed: string | null;
  file: string | null;
  field: string | null;
  hosts: string[];
  oauthNote: boolean;
  unrouted: number;
}

function status(view: RewriteView, enabled: boolean, linked: boolean, bypassed: boolean, running: boolean): AgentStatus {
  if (bypassed) return "bypassed";
  if (!linked) return "unlinked";
  if (!running) return "down";
  return view.enabled && enabled ? "active" : "paused";
}

export function agentRows(view: RewriteView): AgentRow[] {
  const codex = view.codex;
  const codexLinked = codex.link.state === "linked";
  const claude = view.claude;
  const claudeLinked = claude.link.state === "linked";
  const zcode = view.zcode;
  const zcodeLinked = zcode.link.state === "linked";
  const rows: Record<ToolId, AgentRow> = {
    codex: {
      tool: "codex",
      status: status(view, view.codexEnabled, codexLinked, codex.link.state === "bypassed", codex.service.running),
      enabled: view.codexEnabled,
      connected: codex.link.state !== "unlinked",
      canConnect: !codex.unsupported || codex.link.state !== "unlinked",
      unsupported: codex.unsupported && codex.link.state === "unlinked" ? `codex.${codex.unsupported}` : null,
      bypassed: codex.link.state === "bypassed" ? "codex" : null,
      file: codex.codexDir ? `${codex.codexDir}/config.toml` : null,
      field: "model_provider",
      hosts: codex.provider ? [hostOf(codex.provider.baseUrl)].filter(Boolean) as string[] : [],
      oauthNote: false,
      unrouted: 0,
    },
    claude: {
      tool: "claude",
      status: status(view, view.claudeEnabled, claudeLinked, claude.link.state === "bypassed", claude.service.running),
      enabled: view.claudeEnabled,
      connected: claude.link.state !== "unlinked",
      canConnect: !claude.unsupported || claude.link.state !== "unlinked",
      unsupported: claude.unsupported && claude.link.state === "unlinked" ? `claude.${claude.unsupported}` : null,
      bypassed: claude.link.state === "bypassed" ? "claude" : null,
      file: claude.settingsPath,
      field: "env.ANTHROPIC_BASE_URL",
      hosts: claude.upstreamHost ? [claude.upstreamHost] : [],
      oauthNote: true,
      unrouted: 0,
    },
    zcode: {
      tool: "zcode",
      status: status(view, view.zcodeEnabled, zcodeLinked, zcode.link.state === "bypassed", zcode.service.running),
      enabled: view.zcodeEnabled,
      connected: zcode.link.state !== "unlinked",
      canConnect: !zcode.unsupported || zcode.link.state !== "unlinked",
      unsupported: zcode.unsupported && zcode.link.state === "unlinked" ? `zcode.${zcode.unsupported}` : null,
      bypassed: zcode.link.state === "bypassed" ? "zcode" : null,
      file: zcode.configPath,
      field: null,
      hosts: [...new Set(zcode.providers.map((provider) => provider.host).filter((host): host is string => !!host))],
      oauthNote: false,
      unrouted: zcode.link.state === "linked" ? zcode.link.unrouted : 0,
    },
    grok: {
      tool: "grok",
      status: "unsupported",
      enabled: view.grokEnabled,
      connected: false,
      canConnect: false,
      unsupported: "grok.pending",
      bypassed: null,
      file: null,
      field: null,
      hosts: [],
      oauthNote: false,
      unrouted: 0,
    },
  };
  return REWRITE_AGENTS.map((tool) => rows[tool]);
}

function hostOf(url: string): string | null {
  try {
    return new URL(url).host;
  } catch {
    return null;
  }
}

/** The switch each agent row sends to rewrite_set_switches. */
export const SWITCH_KEY: Record<ToolId, "codexEnabled" | "claudeEnabled" | "grokEnabled" | "zcodeEnabled"> = {
  claude: "claudeEnabled",
  codex: "codexEnabled",
  grok: "grokEnabled",
  zcode: "zcodeEnabled",
};
