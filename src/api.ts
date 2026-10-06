import { invoke } from "@tauri-apps/api/core";
import { toastSafeMessage } from "./lib/redact";
import { isTauriRuntime } from "./lib/runtime";
import type {
  AboutInfo,
  Activation,
  AdvancedKind,
  AdvancedResult,
  AdvancedToolInfo,
  AnnouncementsView,
  BackupEntry,
  CleanupPlan,
  CleanupResult,
  ClearPlan,
  DataDirs,
  Envelope,
  ExtensionChange,
  ExtensionsView,
  RewriteRule,
  RewriteView,
  FeedbackInput,
  FeedbackResult,
  FirstRunReport,
  ImportResult,
  OfficialAction,
  OfficialPlan,
  OfficialProductId,
  OfficialResult,
  OkResult,
  Operation,
  PlanResult,
  PromptDetail,
  PromptDiff,
  PromptSort,
  PromptSummary,
  PromptVersion,
  RollbackPlan,
  RollbackResult,
  ScopeId,
  Settings,
  SettingsPatch,
  SnapshotMeta,
  ToolId,
  ToolInfo,
  UpdateChannel,
  UpdateCheck,
  UpdateInstall,
} from "./types";
import { PUBLIC_RELEASE_PAGE } from "./types";

export { PUBLIC_RELEASE_PAGE };

export class ApiError extends Error {
  readonly command: string;
  readonly cause: unknown;

  constructor(message: string, command: string, cause?: unknown) {
    super(message);
    this.name = "ApiError";
    this.command = command;
    this.cause = cause;
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    throw new ApiError("Tauri runtime unavailable", command);
  }
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw new ApiError(toastSafeMessage(err) || `${command} failed`, command, err);
  }
}

export function listTools(): Promise<{ tools: ToolInfo[] }> {
  return call("list_tools", {});
}

export function listPrompts(input: {
  tool: ToolId;
  query?: string;
  tag?: string;
  sort?: PromptSort;
}): Promise<{ prompts: PromptSummary[] }> {
  return call("list_prompts", input);
}

export function getPrompt(id: string): Promise<PromptDetail> {
  return call("get_prompt", { id });
}

export function createPrompt(input: {
  tool: ToolId;
  title: string;
  content: string;
  tags: string[];
}): Promise<PromptDetail> {
  return call("create_prompt", input);
}

export function createPastedPrompt(input: {
  tool: ToolId;
  title: string;
  content: string;
}): Promise<PromptDetail> {
  return call("create_pasted_prompt", input);
}

export function updatePrompt(input: {
  id: string;
  title?: string;
  content?: string;
  tags?: string[];
}): Promise<PromptDetail> {
  return call("update_prompt", input);
}

export function deletePrompt(id: string): Promise<OkResult> {
  return call("delete_prompt", { id });
}

export function copyPrompt(id: string, targetTool: ToolId): Promise<PromptDetail> {
  return call("copy_prompt", { id, targetTool });
}

export function promptHistory(id: string): Promise<{ versions: PromptVersion[] }> {
  return call("prompt_history", { id });
}

export function promptDiff(
  id: string,
  fromVersion: number,
  toVersion: number,
): Promise<PromptDiff> {
  return call("prompt_diff", { id, fromVersion, toVersion });
}

export function restorePromptVersion(id: string, version: number): Promise<PromptDetail> {
  return call("restore_prompt_version", { id, version });
}

export function toolStatus(input: {
  tool: ToolId;
  scope?: ScopeId;
  projectDir?: string;
}): Promise<Envelope> {
  return call("tool_status", input);
}

export function planActivate(input: {
  promptId: string;
  scope: ScopeId;
  projectDir?: string;
}): Promise<PlanResult> {
  return call("plan_activate", input);
}

export function activate(operationId: string): Promise<PlanResult> {
  return call("activate", { operationId });
}

export function planDeactivate(input: {
  promptId?: string;
  tool: ToolId;
  scope: ScopeId;
  projectDir?: string;
}): Promise<PlanResult> {
  return call("plan_deactivate", input);
}

export function deactivate(operationId: string): Promise<PlanResult> {
  return call("deactivate", { operationId });
}

export interface HarnessOutcome {
  ok: boolean;
  tool: ToolId;
  action: "deploy" | "remove";
  promptId: string | null;
  error: string | null;
}

export interface HarnessState {
  tool: ToolId;
  /** A prompt is on the machine; true for a drifted deployment too. */
  deployed: boolean;
  /** The agent's config was changed by hand after the last deploy. */
  drifted?: boolean;
  error: string | null;
  /** The library prompt the machine is running, when the backend could identify it. */
  promptId?: string | null;
  promptTitle?: string | null;
}

export function getHarnessState(tool: ToolId): Promise<HarnessState> {
  return call("harness_state", { tool });
}

/** Take the prompt live on the machine into the library, so it can be edited. */
export function adoptLivePrompt(input: { tool: ToolId; title: string }): Promise<PromptDetail> {
  return call("adopt_live_prompt", input);
}

/** Grok only: put the managed lines of a drifted config back. Preview first, then confirm. */
export function planReconcile(tool: ToolId): Promise<PlanResult> {
  return call("plan_reconcile", { tool });
}

export function confirmReconcile(operationId: string): Promise<PlanResult> {
  return call("confirm_reconcile", { operationId });
}

export function removeHarness(tool: ToolId): Promise<HarnessOutcome> {
  return call("remove_harness", { tool });
}

export function recoverTool(input: {
  tool: ToolId;
  scope?: ScopeId;
  projectDir?: string;
}): Promise<PlanResult> {
  return call("recover_tool", input);
}

export function confirmRecover(operationId: string): Promise<PlanResult> {
  return call("confirm_recover", { operationId });
}

export function doctor(tool: ToolId): Promise<Envelope> {
  return call("doctor", { tool });
}

export function listActivations(tool: ToolId): Promise<{ activations: Activation[] }> {
  return call("list_activations", { tool });
}

export function listOperations(tool?: ToolId): Promise<{ operations: Operation[] }> {
  return call("list_operations", tool ? { tool } : {});
}

export function getSettings(): Promise<Settings> {
  return call("get_settings", {});
}

export function updateSettings(patch: SettingsPatch): Promise<Settings> {
  return call("update_settings", patch);
}

export function getAbout(): Promise<AboutInfo> {
  return call("get_about", {});
}

export function submitFeedback(input: FeedbackInput): Promise<FeedbackResult> {
  return call("submit_feedback", { input });
}

export function checkAppUpdate(channel?: UpdateChannel): Promise<UpdateCheck> {
  return call("check_app_update", channel ? { channel } : {});
}

export function installAppUpdate(channel?: UpdateChannel): Promise<UpdateInstall> {
  return call("install_app_update", { confirmed: true, ...(channel ? { channel } : {}) });
}

export function planOfficialAction(
  product: OfficialProductId,
  action: OfficialAction,
): Promise<OfficialPlan> {
  return call("plan_official_action", { product, action });
}

export function confirmOfficialAction(planId: string): Promise<OfficialResult> {
  return call("confirm_official_action", { planId, confirmed: true });
}

export function cancelOfficialAction(): Promise<{ ok: boolean; cancelled: boolean }> {
  return call("cancel_official_action", {});
}

export function listAdvancedTools(): Promise<{
  tools: AdvancedToolInfo[];
  enabled: boolean;
}> {
  return call("list_advanced_tools", {});
}

export function runAdvanced(kind: AdvancedKind, args: Record<string, string>): Promise<AdvancedResult> {
  return call("run_advanced", { kind, args });
}

export function getStartupReport(): Promise<FirstRunReport> {
  return call("get_startup_report", {});
}

export function importExistingPrompts(paths: string[]): Promise<ImportResult> {
  return call("import_existing_prompts", { paths });
}

export function importMarkdownFiles(tool: ToolId, paths: string[]): Promise<ImportResult> {
  return call("import_markdown_files", { tool, paths });
}

export function inspectZipArchive(path: string): Promise<{ mode: "restore" | "import" }> {
  return call("inspect_zip_archive", { path });
}

export function importZipArchive(path: string): Promise<ImportResult> {
  return call("import_zip_archive", { path });
}

export function exportZipArchive(path: string): Promise<{ ok: boolean; path: string }> {
  return call("export_zip_archive", { path });
}

export function createBackup(): Promise<BackupEntry> {
  return call("create_backup", {});
}

export function listBackups(): Promise<{ backups: BackupEntry[] }> {
  return call("list_backups", {});
}

export function restoreBackup(path: string): Promise<ImportResult> {
  return call("restore_backup", { path });
}

export function planClearAllData(): Promise<ClearPlan> {
  return call("plan_clear_all_data", {});
}

export function clearAllData(phrase: string): Promise<OkResult> {
  return call("clear_all_data", { phrase, confirmed: true });
}

export function getDataDirs(): Promise<DataDirs> {
  return call("get_data_dirs", {});
}

export function acknowledgeRecovery(): Promise<OkResult> {
  return call("acknowledge_recovery", {});
}

export function logFrontendError(message: string, stack?: string): Promise<OkResult> {
  return call("log_frontend_error", { message, stack });
}

export function showMainWindow(): Promise<OkResult> {
  return call("show_main_window", {});
}

export function quitApp(): Promise<OkResult> {
  return call("quit_app", {});
}

export function markFirstRunDone(): Promise<Settings> {
  return call("mark_first_run_done", {});
}

/** The last verified list of extension packs; never touches the network. */
export function extensionsState(): Promise<ExtensionsView> {
  return call("extensions_state", {});
}

/** Ask the network. Needs extensions to be switched on. */
export function extensionsRefresh(): Promise<ExtensionsView> {
  return call("extensions_refresh", {});
}

/** The announcements known now; never touches the network. */
export function announcementsState(): Promise<AnnouncementsView> {
  return call("announcements_state", {});
}

/** Read the announcements feed. A failure keeps the last good copy. */
export function announcementsRefresh(): Promise<AnnouncementsView> {
  return call("announcements_refresh", {});
}

/** Mark announcements as read, or closed (`dismiss`) for a pinned card. */
export function markAnnouncements(ids: string[], dismiss = false): Promise<AnnouncementsView> {
  return call("announcements_mark", { ids, dismiss });
}

/** `enable` switches a newly installed rule table on; prompt packs ignore it. */
export function installExtension(packId: string, enable = false): Promise<ExtensionChange> {
  return call("extension_install", { packId, enable });
}

/** The rules a rule pack would add, read from the verified archive; installs nothing. */
export function previewExtensionRules(packId: string): Promise<RewriteRule[]> {
  return call("extension_preview_rules", { packId });
}

export function uninstallExtension(packId: string): Promise<ExtensionChange> {
  return call("extension_uninstall", { packId });
}

/** Preview a cleanup: what would be removed, nothing is changed. */
export function planCleanup(tool: ToolId): Promise<CleanupPlan> {
  return call("plan_cleanup", { tool });
}

/** Save a snapshot, then remove the deployment (and Claude Code's user-level memory file). */
export function confirmCleanup(operationId: string, clearMemories = false): Promise<CleanupResult> {
  return call("confirm_cleanup", { operationId, clearMemories });
}

export function listSnapshots(): Promise<SnapshotMeta[]> {
  return call("list_snapshots", {});
}

export function planRollback(snapshotId: string): Promise<RollbackPlan> {
  return call("plan_rollback", { snapshotId });
}

export function confirmRollback(operationId: string): Promise<RollbackResult> {
  return call("confirm_rollback", { operationId });
}

export function deleteSnapshot(snapshotId: string): Promise<OkResult> {
  return call("delete_snapshot", { snapshotId });
}

/** Input rewrite rules, switches and Codex link state. Reads files only. */
export function rewriteState(): Promise<RewriteView> {
  return call("rewrite_state", {});
}

export function rewriteSetSwitches(input: { enabled?: boolean; codexEnabled?: boolean }): Promise<RewriteView> {
  return call("rewrite_set_switches", input);
}

export function rewriteSaveUserRules(rules: RewriteRule[]): Promise<RewriteView> {
  return call("rewrite_save_user_rules", { rules });
}

export function rewriteSetTableEnabled(id: string, enabled: boolean): Promise<RewriteView> {
  return call("rewrite_set_table_enabled", { id, enabled });
}

export function rewriteReorderTables(ids: string[]): Promise<RewriteView> {
  return call("rewrite_reorder_tables", { ids });
}

export function rewriteCopyToUser(id: string): Promise<RewriteView> {
  return call("rewrite_copy_to_user", { id });
}

export function rewriteAcceptUpdate(id: string): Promise<RewriteView> {
  return call("rewrite_accept_update", { id });
}

/** Start the relay and point Codex's config at it. */
export function rewriteConnectCodex(): Promise<RewriteView> {
  return call("rewrite_connect_codex", {});
}

/** Point Codex back at its own provider and stop the relay. Rules stay. */
export function rewriteDisconnectCodex(): Promise<RewriteView> {
  return call("rewrite_disconnect_codex", {});
}
