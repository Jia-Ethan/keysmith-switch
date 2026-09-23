import { useEffect, useRef, useState } from "react";
import { cx } from "./ui";

export type HarnessPhase = "idle" | "press" | "busy" | "success" | "failure";

const PRESS_MS = 100;
const SUCCESS_MS = 900;

function reducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function HarnessAction({
  label,
  busyLabel,
  successLabel,
  retryLabel,
  phase,
  reason,
  testId,
  reasonTestId,
  danger = false,
  onRun,
}: {
  label: string;
  busyLabel: string;
  successLabel: string;
  retryLabel: string;
  phase: HarnessPhase;
  reason?: string | null;
  testId: string;
  reasonTestId: string;
  danger?: boolean;
  onRun: () => void;
}) {
  const [pressed, setPressed] = useState(false);
  const pressTimer = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (pressTimer.current !== null) window.clearTimeout(pressTimer.current);
    };
  }, []);

  const locked = phase === "press" || phase === "busy";
  const text =
    phase === "busy" ? busyLabel : phase === "success" ? successLabel : phase === "failure" ? retryLabel : label;

  const start = () => {
    if (locked) return;
    if (reducedMotion()) {
      onRun();
      return;
    }
    setPressed(true);
    if (pressTimer.current !== null) window.clearTimeout(pressTimer.current);
    pressTimer.current = window.setTimeout(() => {
      setPressed(false);
      onRun();
    }, PRESS_MS);
  };

  return (
    <div className="flex min-w-0 items-center gap-3">
      <button
        type="button"
        data-testid={testId}
        data-phase={phase}
        disabled={locked}
        aria-busy={phase === "busy"}
        onClick={start}
        className={cx(
          "inline-flex h-8 min-w-[7.5rem] items-center justify-center rounded-md px-3 text-[13px] font-medium",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background",
          "disabled:cursor-not-allowed",
          pressed && "harness-press",
          danger
            ? "border border-destructive/50 bg-transparent text-destructive hover:bg-destructive/10 disabled:hover:bg-transparent"
            : "bg-primary text-primary-foreground hover:brightness-[1.06] disabled:hover:brightness-100",
        )}
      >
        {text}
      </button>
      {phase === "failure" && reason ? (
        <p data-testid={reasonTestId} className="min-w-0 text-[13px] leading-5 text-destructive">
          {reason}
        </p>
      ) : null}
    </div>
  );
}

export function useHarnessPhase(successLabelMs = SUCCESS_MS) {
  const [phase, setPhase] = useState<HarnessPhase>("idle");
  const [reason, setReason] = useState<string | null>(null);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    };
  }, []);

  const begin = () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    setReason(null);
    setPhase("busy");
  };

  const succeed = () => {
    setReason(null);
    setPhase("success");
    timer.current = window.setTimeout(() => setPhase("idle"), successLabelMs);
  };

  const fail = (message: string) => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    setReason(message);
    setPhase("failure");
  };

  return { phase, reason, begin, succeed, fail };
}
