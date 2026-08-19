import { describe, expect, it } from "vitest";
import { activeIdsFor, scopesForTool, scopesFromEnvelope, defaultScopeFor } from "./tools";
import type { Activation, Envelope, ScopeId } from "../types";

function activation(overrides: Partial<Activation>): Activation {
  return {
    id: "a1",
    tool: "claude",
    promptId: "p1",
    promptTitle: null,
    scope: "user",
    projectDir: null,
    active: true,
    createdAt: "2026-01-01T00:00:00Z",
    fingerprint: null,
    ...overrides,
  };
}

describe("scope support per tool", () => {
  it("keeps user / project / local for claude only", () => {
    expect(scopesForTool("claude")).toEqual(["user", "project", "local"]);
    expect(scopesForTool("codex")).toEqual(["user"]);
    expect(scopesForTool("grok")).toEqual(["user"]);
    expect(scopesForTool("zcode")).toEqual(["user"]);
  });

  it("never fabricates project / local for non-claude tools", () => {
    const reported: ScopeId[] = ["user", "project", "local"];
    expect(scopesForTool("codex", reported)).toEqual(["user"]);
    expect(scopesForTool("grok", reported)).toEqual(["user"]);
    expect(scopesForTool("zcode", reported)).toEqual(["user"]);
  });

  it("filters claude scopes down to what the adapter reports", () => {
    expect(scopesForTool("claude", ["user", "project"])).toEqual(["user", "project"]);
  });

  it("reads supported scopes out of an envelope and still filters per tool", () => {
    const envelope = {
      scopes: [
        { id: "user", supported: true, reason: null },
        { id: "project", supported: true, reason: null },
        { id: "local", supported: false, reason: "not supported" },
      ],
    } as unknown as Envelope;
    expect(scopesFromEnvelope(envelope, "claude")).toEqual(["user", "project"]);
    expect(scopesFromEnvelope(envelope, "codex")).toEqual(["user"]);
  });

  it("falls back to user when the claude default is unsupported", () => {
    expect(defaultScopeFor("claude", ["user", "project", "local"], "local")).toBe("local");
    expect(defaultScopeFor("claude", ["user"], "local")).toBe("user");
    expect(defaultScopeFor("codex", ["user"], "local")).toBe("user");
  });
});

describe("activeIdsFor scope isolation", () => {
  it("only reports activations from the current tool + scope", () => {
    const activations = [
      activation({ id: "a1", promptId: "user-hit", scope: "user" }),
      activation({ id: "a2", promptId: "project-miss", scope: "project", projectDir: "/repo" }),
      activation({ id: "a3", promptId: "other-tool", tool: "codex", scope: "user" }),
    ];
    expect(activeIdsFor(activations, "claude", "user", "")).toEqual(["user-hit"]);
  });

  it("keys project / local activations on the selected project directory", () => {
    const activations = [
      activation({ id: "a1", promptId: "repo-a", scope: "project", projectDir: "/repo-a" }),
      activation({ id: "a2", promptId: "repo-b", scope: "project", projectDir: "/repo-b" }),
    ];
    expect(activeIdsFor(activations, "claude", "project", "/repo-a")).toEqual(["repo-a"]);
    expect(activeIdsFor(activations, "claude", "project", "/repo-b")).toEqual(["repo-b"]);
    expect(activeIdsFor(activations, "claude", "project", "/repo-c")).toEqual([]);
  });

  it("does not leak a project activation into user scope", () => {
    const activations = [
      activation({ id: "a1", promptId: "scoped", scope: "project", projectDir: "/repo" }),
    ];
    expect(activeIdsFor(activations, "claude", "user", "")).toEqual([]);
  });

  it("ignores inactive rows", () => {
    const activations = [activation({ promptId: "stale", active: false })];
    expect(activeIdsFor(activations, "claude", "user", "")).toEqual([]);
  });

  it("separates local from project for the same directory", () => {
    const activations = [
      activation({ id: "a1", promptId: "as-project", scope: "project", projectDir: "/repo" }),
      activation({ id: "a2", promptId: "as-local", scope: "local", projectDir: "/repo" }),
    ];
    expect(activeIdsFor(activations, "claude", "project", "/repo")).toEqual(["as-project"]);
    expect(activeIdsFor(activations, "claude", "local", "/repo")).toEqual(["as-local"]);
  });
});
