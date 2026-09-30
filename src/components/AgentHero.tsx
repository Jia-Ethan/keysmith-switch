import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ToolId } from "../types";
import { IconAlert, IconPower, IconRefresh } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { Button, IconButton, cx } from "./ui";

export type AgentMachine = "deployed" | "undeployed" | "unknown";

/**
 * The top of an agent's workspace: who the agent is, whether a prompt is live
 * and which one, and the two machine-level actions (re-read, remove). Everything
 * the person needs to know before touching the library sits here. Deployments
 * are always machine-wide, so there is no scope to show.
 */
export function AgentHero({
  tool,
  name,
  machine,
  reading,
  statusError,
  deployedTitle,
  unavailable,
  hint,
  busy,
  removing = false,
  onRefresh,
  onRemove,
  removeDisabled,
}: {
  tool: ToolId;
  name: string;
  machine: AgentMachine;
  reading: boolean;
  statusError: string | null | undefined;
  /** Title of the library prompt that is live, or null when it is not known. */
  deployedTitle: string | null;
  unavailable: boolean;
  /** Replaces the machine state when it cannot be read yet. */
  hint?: string | null;
  busy: boolean;
  /** The remove plan is being prepared: the button shows progress instead of a dead click. */
  removing?: boolean;
  onRefresh: () => void;
  onRemove: () => void;
  removeDisabled: boolean;
}) {
  const { t } = useTranslation();
  const deployed = machine === "deployed";
  const tone = statusError
    ? "error"
    : unavailable
      ? "off"
      : hint
        ? "idle"
        : machine === "unknown" || reading
        ? "reading"
        : deployed
          ? "live"
          : "idle";

  return (
    <section
      className="surface-card animate-page-in relative overflow-hidden"
      data-testid="agent-hero"
      data-machine={machine}
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 bg-[radial-gradient(90%_140%_at_0%_0%,rgb(var(--primary)/0.14),transparent_55%)]"
      />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -right-8 -top-10 opacity-[0.07] blur-[1px] transition-opacity dark:opacity-[0.09]"
      >
        {tool === "zcode" ? (
          // The ZCode mark is a filled tile; as a watermark only its letter reads.
          <svg width={190} height={190} viewBox="0 0 24 24" fill="none" className="text-primary">
            <path d="M8.8 9.2h6.4l-6.4 5.6h6.4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        ) : (
          <ToolLogo tool={tool} size={190} />
        )}
      </div>

      <div className="relative flex flex-wrap items-center gap-4 px-5 pb-4 pt-5">
        <div
          className={cx(
            "flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-card ring-1 ring-border",
            deployed ? "shadow-glow" : "shadow-pop",
          )}
        >
          <ToolLogo tool={tool} size={30} />
        </div>

        <div className="min-w-0 flex-1">
          <h1 className="truncate text-[21px] font-semibold tracking-[-0.02em] text-foreground">{name}</h1>
          {/* Re-keyed on every change of state so a deploy or removal visibly lands. */}
          <div key={`${tone}:${deployedTitle ?? ""}`} className="animate-page-in">
            <div
              className="mt-1 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-[12.5px]"
              data-testid="agent-status"
              data-reading={reading || undefined}
            >
              <StatusPill tone={tone}>
                {statusError
                  ? t("hero.statusError")
                  : unavailable
                    ? t("status.unavailable")
                    : hint
                      ? hint
                      : tone === "reading"
                        ? t("quickDeploy.reading")
                        : deployed
                          ? t("quickDeploy.deployedState")
                          : t("quickDeploy.undeployedState")}
              </StatusPill>
            </div>
            {deployed && !statusError && !unavailable && !hint && tone !== "reading" ? (
              <p className="mt-2 flex min-w-0 items-baseline gap-1.5 text-[13px]" data-testid="agent-live-prompt">
                <span className="shrink-0 text-muted-foreground">{t("hero.livePrompt")}</span>
                {deployedTitle ? (
                  <span className="min-w-0 truncate font-medium text-foreground" data-testid="agent-deployed-title">
                    {deployedTitle}
                  </span>
                ) : (
                  <span className="min-w-0 truncate text-muted-foreground" data-testid="agent-deployed-unknown">
                    · {t("hero.livePromptUnknown")}
                  </span>
                )}
              </p>
            ) : null}
          </div>
          {statusError ? (
            <p className="mt-1.5 flex items-start gap-1.5 text-[12px] text-destructive">
              <IconAlert size={13} className="mt-px shrink-0" />
              <span className="min-w-0 break-words">{statusError}</span>
            </p>
          ) : null}
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          {deployed ? (
            <Button
              size="sm"
              variant="danger"
              loading={removing}
              disabled={removeDisabled && !removing}
              data-testid="quick-deploy-remove"
              onClick={onRemove}
            >
              <IconPower size={13} />
              {t("quickDeploy.remove")}
            </Button>
          ) : null}
          <IconButton
            label={t("quickDeploy.refresh")}
            size="iconSm"
            data-testid="harness-refresh"
            disabled={reading || busy}
            onClick={onRefresh}
            className="border border-border bg-card/70"
          >
            <IconRefresh className={reading ? "harness-spin" : undefined} />
          </IconButton>
        </div>
      </div>
    </section>
  );
}

const PILL_TONE = {
  live: "bg-primary/15 text-primary ring-primary/25",
  idle: "bg-muted text-muted-foreground ring-border",
  reading: "bg-muted text-muted-foreground ring-border",
  off: "bg-muted text-muted-foreground ring-border",
  error: "bg-destructive/10 text-destructive ring-destructive/25",
} as const;

function StatusPill({ tone, children }: { tone: keyof typeof PILL_TONE; children: ReactNode }) {
  return (
    <span
      className={cx(
        "inline-flex h-6 items-center gap-1.5 rounded-full px-2.5 font-medium ring-1 ring-inset transition-colors duration-300",
        PILL_TONE[tone],
      )}
    >
      {tone === "reading" ? (
        <span className="harness-spinner" aria-hidden="true" />
      ) : (
        <span
          className={cx("status-dot", tone === "live" ? "text-primary" : tone === "error" ? "text-destructive" : "text-muted-foreground/60")}
          data-live={tone === "live" ? "" : undefined}
          aria-hidden="true"
        />
      )}
      {children}
    </span>
  );
}
