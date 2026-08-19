export interface PlanLike {
  ok?: boolean;
  exitCode?: number | null;
  blockers?: unknown;
  error?: string | null;
}

export interface PlanGate {
  ok: boolean;
  reasons: string[];
}

function asBlockers(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map((item) => String(item)).filter((item) => item.length > 0);
}

/** Unique proceed check: blockers, ok===false, or exitCode!==0 block confirm. */
export function gatePlan(plan: PlanLike | null | undefined): PlanGate {
  if (!plan) {
    return { ok: false, reasons: ["missing plan"] };
  }

  const reasons: string[] = [];
  const blockers = asBlockers(plan.blockers);

  if (plan.ok === false) {
    reasons.push(plan.error || "ok=false");
  }
  if (plan.exitCode !== 0) {
    reasons.push(`exit ${plan.exitCode ?? "unknown"}`);
  }
  if (blockers.length > 0) {
    for (const blocker of blockers) {
      if (!reasons.includes(blocker)) reasons.push(blocker);
    }
  }

  return { ok: reasons.length === 0, reasons };
}

export function canConfirmPlan(plan: PlanLike | null | undefined): boolean {
  return gatePlan(plan).ok;
}
