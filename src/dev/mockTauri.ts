/**
 * Browser-only stand-in for the Tauri backend, so the UI can be designed and
 * reviewed with `npm run dev` and `?mock=1`. It is loaded only in dev builds
 * (see main.tsx) and never ships in a release bundle.
 */
import type { Envelope, PromptDetail, ToolId } from "../types";

const now = () => new Date().toISOString();
const ago = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();

const SEED: Array<[ToolId, string, string, string[], number]> = [
  ["claude", "严谨的代码审查员", "你是一位严谨的资深代码审查员。先说明风险，再给出最小改动的修复建议，避免无关重构。", ["代码", "审查"], 12],
  ["claude", "中文写作润色", "把用户给出的文字润色得更自然、简洁，保留原意，不添加新事实。", ["写作"], 240],
  ["claude", "每日站会助手", "根据昨天的提交与今天的计划，生成三句话的站会发言。", ["效率"], 2880],
  ["codex", "重构小能手", "只做行为不变的重构；每一步都跑测试确认。", ["重构"], 60],
  ["grok", "Grok 示例提示词", "Grok 的示例。", ["示例"], 20],
  ["zcode", "全栈脚手架", "为新项目生成最小可运行的全栈脚手架，并解释每个目录的用途。", ["脚手架"], 30],
];

interface Store {
  prompts: PromptDetail[];
  live: Partial<Record<ToolId, { id: string | null; title: string; body: string }>>;
  ops: Map<string, { kind: "activate" | "deactivate"; tool: ToolId; promptId: string | null }>;
}

const store: Store = {
  prompts: SEED.map(([tool, title, content, tags, mins], index) => ({
    id: `p${index + 1}`,
    tool,
    title,
    content,
    tags,
    active: false,
    lastUsedAt: ago(mins),
    updatedAt: ago(mins),
    createdAt: ago(mins + 500),
    excerpt: content.slice(0, 60),
  })),
  // ZCode runs a prompt that is not in the library, as in the reported screenshot.
  live: {
    claude: { id: "p1", title: "严谨的代码审查员", body: "" },
    zcode: { id: null, title: "", body: "你是 ZCode 的默认助手。这段提示词是用户手动放进机器里的，还没有收进提示词库。" },
  },
  ops: new Map(),
};

function envelope(tool: ToolId, command: string, over: Partial<Envelope> = {}): Envelope {
  return {
    schema: "keysmith-switch/adapter-v1",
    tool,
    command,
    ok: true,
    preview: true,
    available: true,
    unavailableReason: null,
    adapterVersion: "mock",
    cliPath: null,
    argv: [],
    exitCode: 0,
    status: "inactive",
    recoveryRequired: false,
    scopes: [{ id: "user", supported: true, reason: null }],
    targetPaths: [{ path: `~/.${tool}/PROMPT.md`, role: "prompt", exists: true }],
    plannedFiles: [{ path: `~/.${tool}/PROMPT.md`, action: "write", detail: "" }],
    backups: [{ target: `~/.${tool}/PROMPT.md`, backupPath: `~/.keysmith-switch/backups/op/${tool}.bak`, planned: true }],
    conflicts: [],
    warnings: [],
    blockers: [],
    currentFingerprint: "a1b2c3d4e5f6",
    targetFingerprint: "0f9e8d7c6b5a",
    doctor: { ok: true, checks: [] },
    reloadRequired: true,
    reloadHint: null,
    error: null,
    redactedStderr: "",
    ...over,
  };
}

const settings: Record<string, unknown> = { language: "zh-CN", updateChannel: "stable", advancedToolsEnabled: false, defaultClaudeScope: "user", recentProjectDirs: [], updaterEndpointOverride: null, autoCheckUpdates: true, theme: "system", firstRunCompleted: true, extensionsEnabled: new URLSearchParams(window.location.search).has("ext") };

// ?ext=1 starts with extensions on; ?ext=update also pretends one pack has a newer version.
const extUpdate = new URLSearchParams(window.location.search).get("ext") === "update";
const extPacks = [
  { id: "keysmith.example", version: extUpdate ? "0.2.0" : "0.1.0", minAppVersion: "0.2.5", name: "示例包", description: "用来演示拓展包格式的示例，不建议长期使用；正式内容会另外发布。", tools: ["claude", "codex"], itemCount: 2, size: 1312, official: true, compatible: true, installedVersion: extUpdate ? "0.1.0" : (null as string | null), updateAvailable: false },
  { id: "keysmith.future", version: "1.0.0", minAppVersion: "9.0.0", name: "需要新版 App 的包", description: "这个包要求比现在更新的 App。", tools: ["claude"], itemCount: 5, size: 8800, official: true, compatible: false, installedVersion: null as string | null, updateAvailable: false },
];
function extView() {
  const packs = extPacks.map((p) => ({ ...p, updateAvailable: p.compatible && p.installedVersion !== null && p.installedVersion !== p.version }));
  return { packs, updates: packs.filter((p) => p.updateAvailable).length, error: null, checkedAt: now() };
}

let claudeMemoryBytes = 1342;
let codexMemoriesGone = false;
const snapshots: Array<{ id: string; createdAt: string; tool: ToolId; kind: string; deployment: { present: boolean; title: string | null; restorable: boolean }; memory: { path: string; bytes: number; lines: number; sha256: string; mode: number } | null; memories?: { path: string; files: number; bytes: number } | null }> = [];
let grokDrift = new URLSearchParams(window.location.search).get("grok") === "drift";

const delay = <T,>(value: T, ms = 260) => new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

function summary(p: PromptDetail) {
  const { content: _content, ...rest } = p;
  return rest;
}

async function handle(cmd: string, args: Record<string, any> = {}): Promise<unknown> {
  switch (cmd) {
    case "get_settings":
      return settings;
    case "update_settings": {
      Object.assign(settings, args);
      return settings;
    }
    case "extensions_state":
      return extView();
    case "extensions_refresh":
      return delay(extView(), 1100);
    case "extension_install": {
      const pack = extPacks.find((p) => p.id === args.packId)!;
      const report = { added: pack.installedVersion ? 0 : pack.itemCount, updated: pack.installedVersion ? 1 : 0, copied: 0, linked: 0, kept: 0, removed: 0 };
      pack.installedVersion = pack.version;
      return delay({ view: extView(), report }, 1200);
    }
    case "extension_uninstall": {
      const pack = extPacks.find((p) => p.id === args.packId)!;
      pack.installedVersion = null;
      return delay({ view: extView(), report: { added: 0, updated: 0, copied: 0, linked: 0, kept: 1, removed: 1 } }, 700);
    }
    case "get_about":
      return { app: { name: "Keysmith Switch", version: "0.2.3", channel: "stable", preview: false, signed: false, identifier: "com.jia-ethan.keysmith-switch", website: "", github: "" }, adapters: [], official: [] };
    case "get_startup_report":
      return { firstRun: false, candidates: [], recovery: null, sidecar: { pythonRequired: false, tools: [] } };
    case "list_tools":
      return { tools: (["claude", "codex", "grok", "zcode"] as ToolId[]).map((id) => ({ id, name: id, adapterVersion: "mock", available: true, unavailableReason: null, supportedScopes: ["user"], cliPath: null })) };
    case "harness_state": {
      const live = store.live[args.tool as ToolId];
      return delay({ tool: args.tool, deployed: Boolean(live), error: null, promptId: live?.id ?? null, promptTitle: live?.id ? live.title : null }, 380);
    }
    case "adopt_live_prompt": {
      const live = store.live[args.tool as ToolId];
      if (!live || live.id) throw new Error("the live prompt cannot be read back");
      const created: PromptDetail = { id: `p${store.prompts.length + 1}`, tool: args.tool, title: args.title, content: live.body, tags: ["imported"], active: false, lastUsedAt: null, updatedAt: now(), createdAt: now(), excerpt: live.body.slice(0, 60) };
      store.prompts.push(created);
      live.id = created.id;
      live.title = created.title;
      return delay(created, 420);
    }
    case "update_prompt": {
      const prompt = store.prompts.find((p) => p.id === args.id)!;
      Object.assign(prompt, { title: args.title ?? prompt.title, content: args.content ?? prompt.content, updatedAt: now() });
      return prompt;
    }
    case "list_prompts": {
      let list = store.prompts.filter((p) => p.tool === args.tool);
      if (args.query) list = list.filter((p) => (p.title + p.content).includes(args.query));
      if (args.tag) list = list.filter((p) => p.tags.includes(args.tag));
      return delay({ prompts: list.map(summary) }, 200);
    }
    case "list_activations": {
      const live = store.live[args.tool as ToolId];
      return { activations: live?.id ? [{ id: "a1", tool: args.tool, promptId: live.id, promptTitle: live.title, scope: "user", projectDir: null, active: true, createdAt: now(), fingerprint: null }] : [] };
    }
    case "list_operations":
      return { operations: [] };
    case "get_prompt":
      return store.prompts.find((p) => p.id === args.id);
    case "create_pasted_prompt": {
      const created: PromptDetail = { id: `p${store.prompts.length + 1}`, tool: args.tool, title: args.title, content: args.content, tags: [], active: false, lastUsedAt: null, updatedAt: now(), createdAt: now(), excerpt: String(args.content).slice(0, 60) };
      store.prompts.push(created);
      return created;
    }
    case "plan_activate": {
      const prompt = store.prompts.find((p) => p.id === args.promptId)!;
      const operationId = `op${store.ops.size + 1}`;
      store.ops.set(operationId, { kind: "activate", tool: prompt.tool, promptId: prompt.id });
      // ?grok=drift: Grok's config was changed behind the adapter's back until it is tidied.
      if (prompt.tool === "grok" && grokDrift) {
        const why = "config content does not match managed after-state";
        return delay({ operationId, envelope: envelope(prompt.tool, "plan-activate", { ok: false, exitCode: 1, blockers: [why], warnings: [why] }) }, 320);
      }
      return delay({ operationId, envelope: envelope(prompt.tool, "plan-activate") }, 320);
    }
    case "plan_cleanup": {
      const tool = args.tool as ToolId;
      const live = store.live[tool];
      const memory = tool === "claude" && claudeMemoryBytes > 0 ? { path: "/Users/you/.claude/CLAUDE.md", bytes: claudeMemoryBytes, lines: 31, sha256: "m1", mode: 420 } : null;
      // ?codexrunning=1 pretends Codex is open, which keeps its memories folder out of reach.
      const memories = tool === "codex" && !codexMemoriesGone ? { path: "/Users/you/.codex/memories", files: 2778, bytes: 41_943_040 } : null;
      const agentRunning = tool === "codex" && new URLSearchParams(window.location.search).has("codexrunning");
      const root = tool === "zcode" ? ".zcode" : `.${tool}`;
      const extras = [
        { name: `${root}/rules`, path: `/Users/you/${root}/rules`, files: 4, bytes: 6_144, kind: "saved" },
        { name: `${root}/${tool === "claude" ? "settings.json" : "config.toml"}`, path: `/Users/you/${root}/config`, files: 1, bytes: 1_180, kind: "saved-without-login" },
        { name: `${root}/sessions`, path: `/Users/you/${root}/sessions`, files: 312, bytes: 12_582_912, kind: "erased" },
      ];
      const loginInKeychain = tool === "claude";
      return delay({ operationId: `cleanup-${tool}`, tool, deployment: { present: Boolean(live), title: live?.title || null, restorable: Boolean(live) }, memory, memories, extras, loginInKeychain, agentRunning, nothingToDo: false, blockers: [] }, 450);
    }
    case "confirm_cleanup": {
      const tool = String(args.operationId).replace("cleanup-", "") as ToolId;
      const live = store.live[tool];
      const id = `2026100${snapshots.length + 1}T120000Z-${tool}-a1b2c3`;
      snapshots.unshift({ id, createdAt: now(), tool, kind: "cleanup", deployment: { present: Boolean(live), title: live?.title || null, restorable: Boolean(live) }, memory: tool === "claude" && claudeMemoryBytes > 0 ? { path: "/Users/you/.claude/CLAUDE.md", bytes: claudeMemoryBytes, lines: 31, sha256: "m1", mode: 420 } : null });
      delete store.live[tool];
      if (tool === "claude") claudeMemoryBytes = 0;
      const memoriesCleared = tool === "codex" && args.clearMemories === true && !codexMemoriesGone;
      if (memoriesCleared) {
        codexMemoriesGone = true;
        snapshots[0].memories = { path: "/Users/you/.codex/memories", files: 2778, bytes: 41_943_040 };
      }
      return delay({ snapshotId: id, deactivated: Boolean(live), memoryCleared: tool === "claude", memoriesCleared }, 1100);
    }
    case "list_snapshots":
      return snapshots;
    case "plan_rollback": {
      const snap = snapshots.find((s) => s.id === args.snapshotId)!;
      const live = store.live[snap.tool];
      return delay({ operationId: `rollback-${snap.id}`, snapshot: snap, currentTitle: live?.title || null, replacesDeployment: Boolean(live), memory: snap.memory ? { restoreBytes: snap.memory.bytes, currentBytes: claudeMemoryBytes, currentDiffers: claudeMemoryBytes !== snap.memory.bytes } : null, memories: snap.memories ? { restoreFiles: snap.memories.files, restoreBytes: snap.memories.bytes, currentFiles: codexMemoriesGone ? 0 : snap.memories.files, currentBytes: 0 } : null, agentRunning: false, savesCurrent: Boolean(live) || claudeMemoryBytes > 0, blockers: [] }, 400);
    }
    case "confirm_rollback": {
      const snap = snapshots.find((s) => `rollback-${s.id}` === args.operationId)!;
      if (snap.deployment.present) store.live[snap.tool] = { id: null, title: snap.deployment.title || "", body: "restored" };
      if (snap.memory) claudeMemoryBytes = snap.memory.bytes;
      if (snap.memories) codexMemoriesGone = false;
      return delay({ savedSnapshotId: null, redeployed: snap.deployment.present, memoryRestored: Boolean(snap.memory), memoriesRestored: Boolean(snap.memories) }, 1000);
    }
    case "delete_snapshot": {
      const index = snapshots.findIndex((s) => s.id === args.snapshotId);
      if (index >= 0) snapshots.splice(index, 1);
      return { ok: true };
    }
    case "plan_reconcile":
      return delay({ operationId: "reconcile-1", envelope: envelope("grok", "reconcile") }, 400);
    case "confirm_reconcile":
      grokDrift = false;
      return delay({ operationId: "reconcile-1", envelope: envelope("grok", "reconcile", { preview: false }) }, 900);
    case "plan_deactivate": {
      const operationId = `op${store.ops.size + 1}`;
      store.ops.set(operationId, { kind: "deactivate", tool: args.tool, promptId: null });
      return delay({ operationId, envelope: envelope(args.tool, "plan-deactivate") }, 280);
    }
    case "activate":
    case "deactivate": {
      const op = store.ops.get(args.operationId)!;
      if (op.kind === "activate") {
        const prompt = store.prompts.find((p) => p.id === op.promptId)!;
        store.live[op.tool] = { id: prompt.id, title: prompt.title, body: prompt.content };
      } else delete store.live[op.tool];
      return delay({ operationId: args.operationId, envelope: envelope(op.tool, cmd, { preview: false }) }, 700);
    }
    case "check_app_update": {
      // ?update=1 pretends a newer release is out; without it the app is current.
      const newer = new URLSearchParams(window.location.search).has("update");
      return delay(
        {
          available: newer,
          currentVersion: "0.2.3",
          latestVersion: newer ? "0.2.4" : null,
          notes: null,
          size: newer ? 40_956_249 : null,
          channel: "stable",
          restartRequired: newer,
          progress: null,
          error: null,
          releasePage: "https://github.com/Jia-Ethan/keysmith-switch-releases/releases",
          installMode: "inApp",
          reason: null,
        },
        1400,
      );
    }
    default:
      return {};
  }
}

export function installMockTauri(): void {
  (window as any).__TAURI_INTERNALS__ = {
    invoke: (cmd: string, args?: Record<string, unknown>) => handle(cmd, args),
    transformCallback: () => 0,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
  };
}
