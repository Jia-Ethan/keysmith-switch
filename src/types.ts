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
  /** The text came with an extension pack: its title and tags show, its text never does. */
  locked?: boolean;
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

export type ThemeMode = "light" | "dark" | "system";

export interface Settings {
  language: Language;
  updateChannel: UpdateChannel;
  advancedToolsEnabled: boolean;
  defaultClaudeScope: ScopeId;
  recentProjectDirs: string[];
  updaterEndpointOverride: string | null;
  autoCheckUpdates: boolean;
  theme: ThemeMode;
  firstRunCompleted: boolean;
  /** Extension packs read the network only when this is on. */
  extensionsEnabled: boolean;
  /** Input rewrite master switch. Off keeps the rules but rewrites nothing. */
  rewriteEnabled: boolean;
  /** Input rewrite per agent, under the master switch. */
  rewriteCodexEnabled: boolean;
  rewriteClaudeEnabled: boolean;
  rewriteGrokEnabled: boolean;
  rewriteZcodeEnabled: boolean;
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

export interface FeedbackInput {
  kind: "bug" | "feature";
  description: string;
  solution?: string;
  contact?: string;
  screenshots: string[];
}

export interface FeedbackResult {
  issueUrl: string | null;
  fallbackUrl: string | null;
  /** Why the browser form was used; null when the issue was created directly. */
  reason: "browser" | "screenshotsManual" | "createFailed" | null;
  /** A field was shortened to keep the prefilled form URL openable. */
  truncated?: boolean;
}

export interface AboutInfo {
  app: {
    name: string;
    version: string;
    channel: UpdateChannel;
    preview: boolean;
    signed: boolean;
    identifier: string;
    website: string;
    github: string;
  };
  adapters: AdapterVersionInfo[];
  official: OfficialProduct[];
}

export interface ImportCandidate {
  id: string;
  tool: ToolId;
  path: string;
  title: string;
  excerpt: string;
  alreadyImported: boolean;
}

export interface RecoveryMarker {
  kind: string;
  quarantined: string | null;
  rebuilt: boolean;
  at: string;
  detail: string;
}

export interface SidecarToolStatus {
  tool: ToolId;
  frozen: boolean;
  path: string | null;
  available: boolean;
}

export interface FirstRunReport {
  firstRun: boolean;
  candidates: ImportCandidate[];
  recovery: RecoveryMarker | null;
  sidecar: { pythonRequired: boolean; tools: SidecarToolStatus[] };
}

export interface BackupEntry {
  id: string;
  path: string;
  createdAt: string;
  kind: string;
  bytes: number;
}

export interface ClearPlan {
  home: string;
  categories: Array<{ name: string; path: string; exists: boolean }>;
  irreversible: boolean;
  confirmPhrase: string;
}

export interface ImportResult {
  imported: number;
  skipped: number;
  errors: string[];
}

export interface DataDirs {
  home: string;
  logs: string;
  backups: string;
  prompts: string;
}

export type UpdateInstallMode = "none" | "inApp" | "manual";
export type UpdateManualReason = "bootstrapRequired" | "signatureKeyMismatch" | null;

export interface UpdateDetail {
  code: string;
  message: string;
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
  installMode: UpdateInstallMode;
  reason: UpdateManualReason;
  detail?: UpdateDetail | null;
}

export interface UpdateInstall {
  ok: boolean;
  restartRequired: boolean;
  error: string | null;
  releasePage: string;
  installMode: UpdateInstallMode;
  reason: UpdateManualReason;
  detail?: UpdateDetail | null;
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
  autoCheckUpdates: true,
  theme: "system",
  firstRunCompleted: false,
  extensionsEnabled: false,
  rewriteEnabled: false,
  rewriteCodexEnabled: true,
  rewriteClaudeEnabled: true,
  rewriteGrokEnabled: true,
  rewriteZcodeEnabled: true,
};

export const TOOL_IDS: ToolId[] = ["claude", "codex", "grok", "zcode"];

/** An extension pack as the interface shows it; the text is already in the app's language. */
export interface ExtensionPack {
  id: string;
  /** "rules" packs add an input rewrite table instead of prompts. */
  kind: "prompts" | "rules";
  version: string;
  minAppVersion: string;
  name: string;
  /** Carried from the manifest; the Extensions page never renders it (see extensions SPEC.md). */
  description: string;
  tools: ToolId[];
  itemCount: number;
  size: number;
  official: boolean;
  compatible: boolean;
  installedVersion: string | null;
  updateAvailable: boolean;
}

export interface ExtensionsView {
  packs: ExtensionPack[];
  updates: number;
  /** A code such as "offline" when the last look failed; the packs are then the cached ones. */
  error: string | null;
  checkedAt: string | null;
}

export interface ExtensionReport {
  added: number;
  updated: number;
  copied: number;
  linked: number;
  kept: number;
  removed: number;
}

export interface ExtensionChange {
  view: ExtensionsView;
  report: ExtensionReport;
}

/** What an agent is running, as recorded in a snapshot. */
export interface DeploymentMeta {
  present: boolean;
  title: string | null;
  /** The live text was saved, so a rollback can deploy it again. */
  restorable: boolean;
}

export interface MemoryMeta {
  path: string;
  bytes: number;
  lines: number;
  sha256: string;
  mode: number | null;
}

/** A saved state of one agent: made before a cleanup, or before a rollback overwrote something. */
/** An agent's own memory folder (Codex: ~/.codex/memories), counted, never read. */
export interface MemoriesMeta {
  path: string;
  files: number;
  bytes: number;
}

/** One leftover piece of user-level setup, such as `rules` or a wrapper, counted, never read. */
export interface ExtraMeta {
  name: string;
  path: string;
  files: number;
  bytes: number;
  /** `saved`, `saved-without-login` (a copy with every key removed) or `erased` (not saved). */
  kind?: "saved" | "saved-without-login" | "erased";
}

export interface SnapshotMeta {
  id: string;
  createdAt: string;
  tool: ToolId;
  kind: "cleanup" | "before-rollback";
  deployment: DeploymentMeta;
  memory: MemoryMeta | null;
  memories?: MemoriesMeta | null;
  /** The rest of the user-level setup (rules, custom agents, wrapper) kept in the snapshot. */
  extras?: ExtraMeta[];
}

export interface CleanupPlan {
  operationId: string;
  tool: ToolId;
  deployment: DeploymentMeta;
  memory: MemoryMeta | null;
  /** The agent's memory folder, if it has files in it. Only cleared when asked for. */
  memories: MemoriesMeta | null;
  /** The rest of the user-level setup a fresh install does not have. */
  extras?: ExtraMeta[];
  /** The agent's login is in the system keychain: it is removed and never saved. */
  loginInKeychain?: boolean;
  /** The agent is running, so its memory folder cannot be cleared or restored now. */
  agentRunning: boolean;
  /** The deployed config was changed by hand: it is left in place, the rest is still cleaned. */
  configDrifted?: boolean;
  nothingToDo: boolean;
  blockers: string[];
}

export interface CleanupResult {
  snapshotId: string | null;
  deactivated: boolean;
  memoryCleared: boolean;
  memoriesCleared: boolean;
  extrasCleared?: number;
  erased?: number;
  keychainCleared?: boolean;
}

export interface RollbackPlan {
  operationId: string;
  snapshot: SnapshotMeta;
  currentTitle: string | null;
  replacesDeployment: boolean;
  memory: { restoreBytes: number; currentBytes: number; currentDiffers: boolean } | null;
  memories: { restoreFiles: number; restoreBytes: number; currentFiles: number; currentBytes: number } | null;
  /** How many saved pieces of setup are put back. */
  extras?: number;
  agentRunning: boolean;
  /** What is there now is saved as a snapshot first. */
  savesCurrent: boolean;
  blockers: string[];
}

export interface RollbackResult {
  savedSnapshotId: string | null;
  redeployed: boolean;
  memoryRestored: boolean;
  memoriesRestored: boolean;
  extrasRestored?: number;
}

export type AnnouncementKind = "news" | "release" | "preview";

/** One announcement, already in the person's language. Text is plain: never rendered as HTML. */
export interface Announcement {
  id: string;
  kind: AnnouncementKind;
  publishedAt: string;
  title: string;
  body: string;
  /** Shown as a card at the top of the agent pages until closed. */
  pinned: boolean;
  /** Only on these agents' pages; empty means everywhere. */
  tools: ToolId[];
  link: string | null;
  read: boolean;
  dismissed: boolean;
}

export interface AnnouncementsView {
  items: Announcement[];
  unread: number;
  fetchedAt: string | null;
  /** Why the last look failed (`offline`, `invalid`, `too-new`), when it did. */
  error: string | null;
}

// ----- input rewrite ------------------------------------------------------------------

export interface RewriteRule {
  from: string;
  to: string;
}

export interface RulePendingUpdate {
  version: string;
  title: string;
  rules: RewriteRule[];
}

export interface RuleTable {
  id: string;
  kind: "user" | "pack";
  title: string;
  enabled: boolean;
  priority: number;
  packId: string | null;
  packVersion: string | null;
  /** The agents this table applies to; null means every agent. */
  tools: ToolId[] | null;
  rules: RewriteRule[];
  /** An update to an enabled pack table that waits for the person to accept it. */
  pending: RulePendingUpdate | null;
}

export type CodexLinkState =
  | { state: "linked"; provider: string }
  | { state: "bypassed"; provider: string | null }
  | { state: "unlinked" };

export interface CodexRewriteView {
  link: CodexLinkState;
  service: { installed: boolean; running: boolean };
  provider: { id: string; name: string; baseUrl: string } | null;
  /** Why Codex cannot be linked, e.g. "built-in-provider". */
  unsupported: string | null;
  codexDir: string | null;
}

export interface RewriteView {
  enabled: boolean;
  codexEnabled: boolean;
  tables: RuleTable[];
  codex: CodexRewriteView;
}
