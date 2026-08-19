import { invoke } from "@tauri-apps/api/core";
import { toastSafeMessage } from "./lib/redact";
import { isTauriRuntime } from "./lib/runtime";
import type {
  AboutInfo,
  Activation,
  AdvancedKind,
  AdvancedResult,
  AdvancedToolInfo,
  Envelope,
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
  ScopeId,
  Settings,
  SettingsPatch,
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

export function recoverTool(input: {
  tool: ToolId;
  scope?: ScopeId;
  projectDir?: string;
}): Promise<PlanResult> {
  return call("recover_tool", input);
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

export function checkAppUpdate(channel?: UpdateChannel): Promise<UpdateCheck> {
  return call("check_app_update", channel ? { channel } : {});
}

export function installAppUpdate(): Promise<UpdateInstall> {
  return call("install_app_update", { confirmed: true });
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

export function listAdvancedTools(): Promise<{
  tools: AdvancedToolInfo[];
  enabled: boolean;
}> {
  return call("list_advanced_tools", {});
}

export function runAdvanced(kind: AdvancedKind, args: Record<string, string>): Promise<AdvancedResult> {
  return call("run_advanced", { kind, args });
}
