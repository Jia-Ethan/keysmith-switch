import { useSyncExternalStore } from "react";
import { getHarnessState } from "../api";
import type { ToolId } from "../types";

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
  error: string | null;
  /** The library prompt live on the machine; null when unknown or not deployed. */
  promptId: string | null;
  promptTitle: string | null;
}

export type HarnessStatusMap = Partial<Record<ToolId, HarnessEntry>>;

type Listener = () => void;

let statuses: HarnessStatusMap = {};
const listeners = new Set<Listener>();
const inflight = new Map<ToolId, Promise<HarnessEntry>>();

function emit(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function setStatus(tool: ToolId, entry: HarnessEntry): void {
  statuses = { ...statuses, [tool]: entry };
  emit();
}

export function getHarnessStatus(tool: ToolId): HarnessEntry | undefined {
  return statuses[tool];
}

export function useHarnessStatus(tool: ToolId): HarnessEntry | undefined {
  return useSyncExternalStore(
    subscribe,
    () => statuses[tool],
    () => statuses[tool],
  );
}

async function read(tool: ToolId): Promise<HarnessEntry> {
  try {
    const state = await getHarnessState(tool);
    const deployed = state.deployed;
    return {
      machine: deployed ? "deployed" : "undeployed",
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
export function loadHarnessStatus(tool: ToolId, force = false): Promise<HarnessEntry> {
  const known = statuses[tool];
  if (known && !force) return Promise.resolve(known);

  const pending = inflight.get(tool);
  if (pending) return pending;

  const request = read(tool).then((entry) => {
    inflight.delete(tool);
    setStatus(tool, entry);
    return entry;
  });
  inflight.set(tool, request);
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
): HarnessEntry {
  const deployed = action === "deploy";
  const entry: HarnessEntry = {
    machine: deployed ? "deployed" : "undeployed",
    error: null,
    promptId: deployed ? prompt?.id ?? null : null,
    promptTitle: deployed ? prompt?.title ?? null : null,
  };
  setStatus(tool, entry);
  return entry;
}

/** Test seam: drop everything remembered in this run. */
export function resetHarnessStatuses(): void {
  statuses = {};
  inflight.clear();
  emit();
}
