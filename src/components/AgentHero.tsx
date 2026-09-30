import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ToolId } from "../types";
import { IconAlert, IconPencil, IconPower, IconRefresh } from "./icons";
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
  onEdit,
  editing = false,
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
  /** Open the live prompt in the editor. */
  onEdit?: () => void;
  editing?: boolean;
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

  const showLive = deployed && !statusError && !unavailable && !hint && tone !== "reading";

  return (
    <section
      className="hero-stage animate-page-in relative overflow-hidden rounded-[28px] border border-border/70 bg-card"
      data-testid="agent-hero"
      data-machine={machine}
    >
      <div aria-hidden="true" className="hero-aurora pointer-events-none absolute inset-0" />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -right-10 -top-12 opacity-[0.06] blur-[1px] dark:opacity-[0.09]"
      >
        {tool === "zcode" ? (
          // The ZCode mark is a filled tile; as a watermark only its letter reads.
          <svg width={240} height={240} viewBox="0 0 832 832" className="text-primary">
            <path
              fill="currentColor"
              d="M404,632 L658,632 L658,570 L459,570 L448,574 L440,581Z M670,199 L467,199 L161,632 L364,632Z M173,261 L373,261 L385,256 L392,249 L427,199 L173,199Z"
            />
          </svg>
        ) : (
          <ToolLogo tool={tool} size={240} />
        )}
      </div>

      <div className="relative flex flex-wrap items-center gap-5 px-7 py-7">
        <div className="relative shrink-0">
          {deployed ? <span aria-hidden="true" className="hero-orb-ring absolute inset-0 rounded-[24px]" /> : null}
          <div
            className={cx(
              "relative flex h-[76px] w-[76px] items-center justify-center rounded-[24px] bg-card ring-1 ring-border transition-shadow duration-500",
              deployed ? "shadow-glow" : "shadow-pop",
            )}
          >
            <ToolLogo tool={tool} size={38} />
          </div>
        </div>

        <div className="min-w-0 flex-1" data-testid="agent-status" data-reading={reading || undefined}>
          <div className="flex items-center gap-2.5">
            <h1 className="truncate text-[13px] font-semibold tracking-[0.02em] text-muted-foreground">{name}</h1>
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
          {/* Re-keyed on every change of state so a deploy or removal visibly lands. */}
          <div key={`${tone}:${deployedTitle ?? ""}`} className="animate-page-in">
            {showLive ? (
              <p className="mt-2 min-w-0" data-testid="agent-live-prompt">
                <span className="block text-[12.5px] text-muted-foreground">{t("hero.livePrompt")}</span>
                {deployedTitle ? (
                  <span
                    className="mt-0.5 block min-w-0 truncate text-[26px] font-semibold leading-tight tracking-[-0.025em] text-foreground"
                    data-testid="agent-deployed-title"
                  >
                    {deployedTitle}
                  </span>
                ) : (
                  <span
                    className="mt-0.5 block min-w-0 truncate text-[22px] font-semibold leading-tight tracking-[-0.02em] text-foreground"
                    data-testid="agent-deployed-unknown"
                  >
                    {t("hero.livePromptUnknown")}
                  </span>
                )}
              </p>
            ) : tone === "idle" && !hint ? (
              <p className="mt-2 text-[22px] font-semibold leading-tight tracking-[-0.02em] text-foreground">
                {t("hero.emptyTitle")}
                <span className="mt-1 block text-[13px] font-normal tracking-normal text-muted-foreground">
                  {t("hero.emptyHint")}
                </span>
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
          {showLive && onEdit ? (
            <Button
              size="md"
              variant="outline"
              loading={editing}
              disabled={removeDisabled && !editing}
              data-testid="hero-edit"
              onClick={onEdit}
            >
              <IconPencil size={14} />
              {t("hero.edit")}
            </Button>
          ) : null}
          {deployed ? (
            <Button
              size="md"
              variant="danger"
              loading={removing}
              disabled={removeDisabled && !removing}
              data-testid="quick-deploy-remove"
              onClick={onRemove}
            >
              <IconPower size={14} />
              {t("quickDeploy.remove")}
            </Button>
          ) : null}
          <IconButton
            label={t("quickDeploy.refresh")}
            size="icon"
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
