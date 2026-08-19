export type ToolId = "claude" | "codex" | "grok" | "zcode";

export type ScopeId = "user" | "project" | "local";

export type Language = "zh-CN" | "zh-TW" | "en";

export type UpdateChannel = "stable" | "beta";

export type PromptSort = "lastUsed" | "updated" | "title" | "created";

export type ToolStatusName =
  | "not-installed"
  | "inactive"
  | "active"
  | "drift"
  | "conflict"
  | "recovery-required"
  | "unavailable";

export type OfficialProductId = "claude" | "codex" | "grok" | "zcode";

export type OfficialAction = "install" | "update";

export type AdvancedKind = "scenario" | "grokRun" | "grokBreaktest";

export interface ScopeInfo {
  id: ScopeId;
  supported: boolean;
  reason: string | null;
}

export interface TargetPath {
  path: string;
  role: string;
  exists: boolean;
}

export interface PlannedFile {
  path: string;
  action: string;
  detail: string;
}

export interface BackupPlan {
  target: string;
  backupPath: string | null;
  planned: boolean;
}

export interface ConflictInfo {
  path: string;
  reason: string;
}

export interface DoctorCheck {
  name: string;
  ok: boolean;
  detail?: string;
}

export interface DoctorReport {
  ok: boolean;
  checks: DoctorCheck[];
}

export interface Envelope {
  schema: "keysmith-switch/adapter-v1";
  tool: ToolId;
  command: string;
  ok: boolean;
  preview: boolean;
  available: boolean;
  unavailableReason: string | null;
  adapterVersion: string;
  cliPath: string | null;
  argv: string[];
  exitCode: number;
  status: ToolStatusName;
  recoveryRequired: boolean;
  scopes: ScopeInfo[];
  targetPaths: TargetPath[];
  plannedFiles: PlannedFile[];
  backups: BackupPlan[];
  conflicts: Array<ConflictInfo | string>;
  warnings: string[];
  blockers: string[];
  currentFingerprint: string | null;
  targetFingerprint: string | null;
  doctor: DoctorReport;
  reloadRequired: boolean;
  reloadHint: string | null;
  error: string | null;
  redactedStderr: string;
}

export interface ToolInfo {
  id: ToolId;
  name: string;
  adapterVersion: string;
  available: boolean;
  unavailableReason: string | null;
  supportedScopes: ScopeId[];
  cliPath: string | null;
}

export interface PromptSummary {
  id: string;
  tool: ToolId;
  title: string;
  tags: string[];
  active: boolean;
  lastUsedAt: string | null;
  updatedAt: string;
  createdAt: string;
  excerpt: string | null;
}

export interface PromptDetail extends PromptSummary {
  content: string;
}

export interface PromptVersion {
  version: number;
  createdAt: string;
  title: string;
  summary: string | null;
}

export interface PromptDiff {
  unified: string;
  summary: string;
}

export interface Activation {
  id: string;
  tool: ToolId;
  promptId: string;
  promptTitle: string | null;
  scope: ScopeId;
  projectDir: string | null;
  active: boolean;
  createdAt: string;
  fingerprint: string | null;
}

export interface Operation {
  id: string;
  tool: ToolId;
  kind: string;
  status: string;
  error: string | null;
  createdAt: string;
  recoverAvailable: boolean;
}

export interface Settings {
  language: Language;
  updateChannel: UpdateChannel;
  advancedToolsEnabled: boolean;
  defaultClaudeScope: ScopeId;
  recentProjectDirs: string[];
  updaterEndpointOverride: string | null;
}

export type SettingsPatch = Partial<Settings>;

export interface AdapterVersionInfo {
  tool: ToolId;
  version: string;
  bundled: boolean;
  path: string | null;
}

export interface OfficialProduct {
  product: OfficialProductId;
  currentVersion: string | null;
  latestVersion: string | null;
  installed: boolean;
  executablePath: string | null;
  source: string;
  argv: string[];
  dest: string;
  available: boolean;
  unavailableReason: string | null;
}

export interface AboutInfo {
  app: {
    name: string;
    version: string;
    channel: UpdateChannel;
  };
  adapters: AdapterVersionInfo[];
  official: OfficialProduct[];
}

export interface UpdateCheck {
  available: boolean;
  currentVersion: string;
  latestVersion: string | null;
  notes: string | null;
  size: number | null;
  channel: UpdateChannel;
  restartRequired: boolean;
  progress: number | null;
  error: string | null;
  releasePage: string;
}

export interface UpdateInstall {
  ok: boolean;
  restartRequired: boolean;
  error: string | null;
  releasePage: string;
}

export interface OfficialPlan {
  planId: string;
  product: OfficialProductId;
  action: OfficialAction;
  currentVersion: string | null;
  latestVersion: string | null;
  installed: boolean;
  executablePath: string | null;
  source: string;
  argv: string[];
  dest: string;
  blockers: string[];
}

export interface OfficialResult {
  ok: boolean;
  product: OfficialProductId;
  action: OfficialAction;
  error: string | null;
}

export interface AdvancedToolInfo {
  kind: AdvancedKind;
  name: string;
  description: string;
}

export interface AdvancedResult {
  ok: boolean;
  kind: AdvancedKind;
  output: string;
  error: string | null;
}

export interface PlanResult {
  operationId: string;
  envelope: Envelope;
}

export interface OkResult {
  ok: boolean;
}

export const PUBLIC_RELEASE_PAGE =
  "https://github.com/Jia-Ethan/keysmith-switch-releases/releases";

export const ADAPTER_SCHEMA = "keysmith-switch/adapter-v1" as const;

export const DEFAULT_SETTINGS: Settings = {
  language: "zh-CN",
  updateChannel: "stable",
  advancedToolsEnabled: false,
  defaultClaudeScope: "user",
  recentProjectDirs: [],
  updaterEndpointOverride: null,
};

export const TOOL_IDS: ToolId[] = ["claude", "codex", "grok", "zcode"];
