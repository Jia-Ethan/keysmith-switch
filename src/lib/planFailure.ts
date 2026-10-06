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

const OWNERSHIP = "existing deployment manifest ownership conflict: ";
const INSTRUCTIONS_MISSING =
  /model_instructions_file is missing; expected it to still reference (\S+?)\. Uninstall will leave/;
const INSTRUCTIONS_REPOINTED =
  /model_instructions_file ownership conflict: the current field is set to another path; expected it to still reference (\S+)/;
const DRY_RUN = /dry-run found (\d+) confirmed blocker\(s\); no files were changed\.?/;

/**
 * The adapter speaks English whatever the interface language is. Say the reasons people
 * actually hit in their own language; anything unknown passes through unchanged.
 */
export function localizeReason(reason: string, t: TFunction): string {
  const dry = DRY_RUN.exec(reason);
  if (dry) return t("plan.blockerDryRun", { count: Number(dry[1]) });
  if (!reason.startsWith(OWNERSHIP)) return reason;
  const missing = INSTRUCTIONS_MISSING.exec(reason);
  if (missing) return t("plan.blockerInstructionsMissing", { ref: missing[1] });
  const repointed = INSTRUCTIONS_REPOINTED.exec(reason);
  if (repointed) return t("plan.blockerInstructionsRepointed", { ref: repointed[1] });
  return t("plan.blockerOwnership", { detail: reason.slice(OWNERSHIP.length) });
}

/** Codex only: the managed config line is gone, so the old deployment record is stale and Cleanup puts it right. */
export function needsCleanup(tool: ToolId, envelope: Envelope): boolean {
  return (
    tool === "codex" &&
    [...envelope.blockers, envelope.error ?? ""].some((item) => INSTRUCTIONS_MISSING.test(item))
  );
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
    message: reason ? toastSafeMessage(localizeReason(reason, t)) : t("plan.adapterFailed"),
    detail: stderr ? stderr.slice(-700) : null,
  };
}

/** Grok only: the config drifted in a way the adapter can put right itself. */
export function needsReconcile(tool: ToolId, envelope: Envelope): boolean {
  return tool === "grok" && envelope.blockers.some((item) => item.includes(CONFIG_DRIFT));
}
