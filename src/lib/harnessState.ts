import { useSyncExternalStore } from "react";
import { getHarnessState } from "../api";
import type { RuntimeTarget, ToolId } from "../types";

/**
 * Per-tool deployment state, remembered for one app run.
 *
 * The local read is the slow part of the first screen: every tool switch used to
 * re-read the machine. Results live here instead, keyed by tool, and survive
 * navigation. The store is module scope, so a fresh launch starts empty by
 * construction — remembering is per run, never permanent.
 */
export type HarnessMachine = "deployed" | "undeployed";

export interface HarnessEntry {
  machine: HarnessMachine;
  /** Deployed, but the agent's config was changed by hand since. */
  drifted?: boolean;
  /** DSH only: deployed into a profile the host does not boot; a redeploy moves it. */
  wrongProfile?: boolean;
  error: string | null;
  /** The library prompt live on the machine; null when unknown or not deployed. */
  promptId: string | null;
  promptTitle: string | null;
}

export type HarnessStatusMap = Record<string, HarnessEntry>;

type Listener = () => void;

let statuses: HarnessStatusMap = {};
const listeners = new Set<Listener>();
const inflight = new Map<string, Promise<HarnessEntry>>();

export const NATIVE_RUNTIME: RuntimeTarget = { kind: "native" };

export function runtimeTargetKey(target: RuntimeTarget = NATIVE_RUNTIME): string {
  return target.kind === "native"
    ? "native"
    : `wsl:${target.distribution}:${target.user ?? ""}`;
}

function statusKey(tool: ToolId, target: RuntimeTarget = NATIVE_RUNTIME): string {
  return `${tool}\0${runtimeTargetKey(target)}`;
}

function emit(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function setStatus(tool: ToolId, target: RuntimeTarget, entry: HarnessEntry): void {
  statuses = { ...statuses, [statusKey(tool, target)]: entry };
  emit();
}

export function getHarnessStatus(tool: ToolId, target: RuntimeTarget = NATIVE_RUNTIME): HarnessEntry | undefined {
  return statuses[statusKey(tool, target)];
}

export function useHarnessStatus(tool: ToolId, target: RuntimeTarget = NATIVE_RUNTIME): HarnessEntry | undefined {
  const key = statusKey(tool, target);
  return useSyncExternalStore(
    subscribe,
    () => statuses[key],
    () => statuses[key],
  );
}

async function read(tool: ToolId, target: RuntimeTarget): Promise<HarnessEntry> {
  try {
    const state =
      target.kind === "wsl"
        ? await getHarnessState(tool, target)
        : await getHarnessState(tool);
    const deployed = state.deployed;
    return {
      machine: deployed ? "deployed" : "undeployed",
      drifted: deployed && Boolean(state.drifted),
      wrongProfile: deployed && Boolean(state.wrongProfile),
      error: state.error,
      promptId: deployed ? state.promptId ?? null : null,
      promptTitle: deployed ? state.promptTitle ?? null : null,
    };
  } catch (error) {
    return {
      machine: "undeployed",
      error: error instanceof Error ? error.message : null,
      promptId: null,
      promptTitle: null,
    };
  }
}

/**
 * Read the machine unless this run already knows the answer.
 * Concurrent callers for the same tool share one read.
 */
export function loadHarnessStatus(
  tool: ToolId,
  targetOrForce: RuntimeTarget | boolean = NATIVE_RUNTIME,
  force = false,
): Promise<HarnessEntry> {
  const target = typeof targetOrForce === "boolean" ? NATIVE_RUNTIME : targetOrForce;
  const shouldForce = typeof targetOrForce === "boolean" ? targetOrForce : force;
  const key = statusKey(tool, target);
  const known = statuses[key];
  if (known && !shouldForce) return Promise.resolve(known);

  const pending = inflight.get(key);
  if (pending) return pending;

  const request = read(tool, target).then((entry) => {
    inflight.delete(key);
    setStatus(tool, target, entry);
    return entry;
  });
  inflight.set(key, request);
  return request;
}

/**
 * Deploy and uninstall write the result straight into the store. A deploy
 * passes the prompt it just wrote so the hero can name it without another read.
 */
export function applyHarnessOutcome(
  tool: ToolId,
  action: "deploy" | "remove",
  prompt?: { id?: string | null; title?: string | null },
  target: RuntimeTarget = NATIVE_RUNTIME,
): HarnessEntry {
  const deployed = action === "deploy";
  const entry: HarnessEntry = {
    machine: deployed ? "deployed" : "undeployed",
    error: null,
    promptId: deployed ? prompt?.id ?? null : null,
    promptTitle: deployed ? prompt?.title ?? null : null,
  };
  setStatus(tool, target, entry);
  return entry;
}

/** Test seam: drop everything remembered in this run. */
export function resetHarnessStatuses(): void {
  statuses = {};
  inflight.clear();
  emit();
}
