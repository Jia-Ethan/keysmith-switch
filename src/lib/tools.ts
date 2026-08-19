import type { Envelope, ScopeId, ToolId, ToolInfo } from "../types";
import { TOOL_IDS } from "../types";

export const TOOL_CATALOG: ToolInfo[] = [
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
  {
    id: "grok",
    name: "Grok Build",
    adapterVersion: "0.4.1",
    available: true,
    unavailableReason: null,
    supportedScopes: ["user"],
    cliPath: null,
  },
  {
    id: "zcode",
    name: "ZCode",
    adapterVersion: "0.1.0",
    available: true,
    unavailableReason: null,
    supportedScopes: ["user"],
    cliPath: null,
  },
];

export function isToolId(value: string): value is ToolId {
  return TOOL_IDS.includes(value as ToolId);
}

export function mergeTools(remote: ToolInfo[] | null | undefined): ToolInfo[] {
  return TOOL_CATALOG.map((base) => {
    const hit = remote?.find((item) => item.id === base.id);
    if (!hit) return base;
    const scopes =
      hit.supportedScopes?.length > 0 ? hit.supportedScopes : base.supportedScopes;
    return {
      ...base,
      ...hit,
      name: hit.name || base.name,
      supportedScopes: scopesForTool(hit.id, scopes),
    };
  });
}

export function scopesForTool(tool: ToolId, reported?: ScopeId[]): ScopeId[] {
  if (tool === "claude") {
    const allowed: ScopeId[] = ["user", "project", "local"];
    if (!reported || reported.length === 0) return allowed;
    return allowed.filter((scope) => reported.includes(scope));
  }
  if (!reported || reported.length === 0) return ["user"];
  return reported.filter((scope) => scope === "user");
}

export function scopesFromEnvelope(envelope: Envelope | null, tool: ToolId): ScopeId[] {
  if (!envelope) return scopesForTool(tool);
  const supported = envelope.scopes.filter((item) => item.supported).map((item) => item.id);
  return scopesForTool(tool, supported);
}

export function scopeNeedsProjectDir(scope: ScopeId): boolean {
  return scope === "project" || scope === "local";
}

export function defaultScopeFor(
  tool: ToolId,
  supported: ScopeId[],
  defaultClaudeScope: ScopeId,
): ScopeId {
  if (tool === "claude" && supported.includes(defaultClaudeScope)) {
    return defaultClaudeScope;
  }
  return supported.includes("user") ? "user" : supported[0] ?? "user";
}

export function isRecoveryState(envelope: Envelope | null): boolean {
  if (!envelope) return false;
  return (
    envelope.recoveryRequired ||
    envelope.status === "recovery-required" ||
    envelope.status === "drift" ||
    envelope.status === "conflict"
  );
}
