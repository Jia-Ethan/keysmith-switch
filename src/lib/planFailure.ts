import type { TFunction } from "i18next";
import type { Envelope, ToolId } from "../types";
import { toastSafeMessage } from "./redact";

/** Why a confirmed plan did not go through, in a sentence, with the technical part kept apart. */
export interface PlanFailure {
  message: string;
  detail: string | null;
}

/** What the adapter says when the config it manages was changed behind its back. */
export const CONFIG_DRIFT = "does not match managed after-state";

/** `ok=false` and `exit 1` are bookkeeping, not reasons. */
const NOISE = /^(ok=false|exit \d+|missing plan)$/;

export function isNoise(reason: string): boolean {
  return NOISE.test(reason.trim());
}

/** A command failed with a JSON body (`{"kind":…,"message":…}`); say what it means. */
export function readableError(reason: unknown, t: TFunction): string {
  const text = reason instanceof Error ? reason.message : String(reason ?? "");
  try {
    const body = JSON.parse(text) as { message?: unknown };
    const message = typeof body?.message === "string" ? body.message : "";
    if (message.startsWith("plan already used") || message.startsWith("plan not found")) {
      return t("errors.planUsed");
    }
    if (message) return toastSafeMessage(message);
  } catch {
    // not JSON: fall through
  }
  return toastSafeMessage(text) || t("plan.failed");
}

/** The failure to show after an adapter ran and did not finish. */
export function failureFromEnvelope(envelope: Envelope, t: TFunction): PlanFailure {
  const reason = envelope.error || envelope.blockers.find((item) => !isNoise(item));
  const stderr = envelope.redactedStderr?.trim();
  return {
    message: reason ? toastSafeMessage(reason) : t("plan.adapterFailed"),
    detail: stderr ? stderr.slice(-700) : null,
  };
}

/** Grok only: the config drifted in a way the adapter can put right itself. */
export function needsReconcile(tool: ToolId, envelope: Envelope): boolean {
  return tool === "grok" && envelope.blockers.some((item) => item.includes(CONFIG_DRIFT));
}
