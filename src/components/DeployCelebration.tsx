import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import type { ToolId } from "../types";
import { ToolLogo } from "./ToolLogos";

const SPARKS = 10;
export const CELEBRATION_MS = 1600;

/**
 * A short, non-blocking confirmation after a deployment lands: the agent's mark,
 * a drawn check and a ring of sparks in the agent color. Pointer events pass
 * through, and it removes itself; reduced motion turns it into a static badge.
 */
export function DeployCelebration({
  tool,
  title,
  subtitle,
  onDone,
}: {
  tool: ToolId;
  title: string;
  subtitle?: string;
  onDone: () => void;
}) {
  // The parent re-renders while it reloads after a deploy; the timer must not restart.
  const doneRef = useRef(onDone);
  doneRef.current = onDone;
  useEffect(() => {
    const timer = window.setTimeout(() => doneRef.current(), CELEBRATION_MS);
    return () => window.clearTimeout(timer);
  }, []);

  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      className="pointer-events-none fixed inset-0 z-[75] flex items-center justify-center"
      data-testid="deploy-celebration"
      role="status"
      aria-live="polite"
    >
      <div
        aria-hidden="true"
        className="celebrate-root absolute inset-0 bg-[radial-gradient(circle_at_center,hsl(var(--background)/0.8),hsl(var(--background)/0.35)_35%,transparent_65%)]"
      />
      <div className="celebrate-root relative flex flex-col items-center gap-4">
        <div className="relative flex h-28 w-28 items-center justify-center">
          <span className="celebrate-ring absolute inset-0 rounded-full border-2 border-primary" aria-hidden="true" />
          {Array.from({ length: SPARKS }, (_, index) => (
            <span
              key={index}
              aria-hidden="true"
              className="celebrate-spark absolute left-1/2 top-1/2 -ml-[3px] -mt-[3px] h-1.5 w-1.5 rounded-full bg-primary"
              style={{ ["--angle" as string]: `${(360 / SPARKS) * index}deg` }}
            />
          ))}
          <div className="celebrate-badge relative flex h-24 w-24 items-center justify-center rounded-[28px] bg-card shadow-glow ring-1 ring-primary/30">
            <ToolLogo tool={tool} size={44} />
            <span className="absolute -bottom-2 -right-2 flex h-9 w-9 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg ring-4 ring-background">
              <svg width="18" height="18" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path
                  className="celebrate-check"
                  d="M3.4 8.4 6.5 11.4l6.1-6.8"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </span>
          </div>
        </div>
        <div className="celebrate-badge glass rounded-2xl border border-border px-4 py-2 text-center shadow-pop">
          <p className="text-[14px] font-semibold text-foreground">{title}</p>
          {subtitle ? <p className="text-[12px] text-muted-foreground">{subtitle}</p> : null}
        </div>
      </div>
    </div>,
    document.body,
  );
}
