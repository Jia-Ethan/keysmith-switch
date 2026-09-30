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

const delay = <T,>(value: T, ms = 260) => new Promise<T>((resolve) => setTimeout(() => resolve(value), ms));

function summary(p: PromptDetail) {
  const { content: _content, ...rest } = p;
  return rest;
}

async function handle(cmd: string, args: Record<string, any> = {}): Promise<unknown> {
  switch (cmd) {
    case "get_settings":
      return { language: "zh-CN", updateChannel: "stable", advancedToolsEnabled: false, defaultClaudeScope: "user", recentProjectDirs: [], updaterEndpointOverride: null, autoCheckUpdates: false, theme: "system", firstRunCompleted: true };
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
      return delay({ operationId, envelope: envelope(prompt.tool, "plan-activate") }, 320);
    }
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
    case "check_app_update":
      return { available: false };
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
