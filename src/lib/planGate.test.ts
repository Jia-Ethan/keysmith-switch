import { describe, expect, it } from "vitest";
import { canConfirmPlan, gatePlan } from "./planGate";

const okPlan = { ok: true, exitCode: 0, blockers: [] as string[] };

describe("gatePlan", () => {
  it("allows confirm only when ok, exitCode 0, and no blockers", () => {
    expect(gatePlan(okPlan)).toEqual({ ok: true, reasons: [] });
    expect(canConfirmPlan(okPlan)).toBe(true);
  });

  it("blocks confirm when blockers are present", () => {
    const gated = gatePlan({ ...okPlan, blockers: ["conflict on CLAUDE.md"] });
    expect(gated.ok).toBe(false);
    expect(gated.reasons).toContain("conflict on CLAUDE.md");
    expect(canConfirmPlan({ ...okPlan, blockers: ["x"] })).toBe(false);
  });

  it("blocks confirm when ok === false", () => {
    const gated = gatePlan({ ...okPlan, ok: false, error: "preview refused" });
    expect(gated.ok).toBe(false);
    expect(gated.reasons).toContain("preview refused");
    expect(canConfirmPlan({ ok: false, exitCode: 0, blockers: [] })).toBe(false);
  });

  it("blocks confirm when exitCode !== 0", () => {
    const gated = gatePlan({ ...okPlan, exitCode: 2 });
    expect(gated.ok).toBe(false);
    expect(gated.reasons[0]).toMatch(/exit 2/);
    expect(canConfirmPlan({ ok: true, exitCode: 1, blockers: [] })).toBe(false);
  });
});
