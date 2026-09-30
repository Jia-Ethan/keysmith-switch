import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { formatBytes } from "../lib/format";
import { openExternal } from "../lib/runtime";
import { PUBLIC_RELEASE_PAGE } from "../types";
import { ErrorBanner } from "./ErrorBanner";
import { IconAlert, IconCheck, IconDownload, IconExternal, IconRefresh } from "./icons";
import { useUpdateOptional } from "./UpdateProvider";
import { Button, SectionLabel } from "./ui";

type Kind = "idle" | "checking" | "upToDate" | "available" | "error";

type Updater = NonNullable<ReturnType<typeof useUpdateOptional>>;

/**
 * The About page's update row. One orb changes with the state (its icons cross
 * fade, the check draws itself, a ring turns while looking), the text under it
 * rises in, and anything that needs room opens by growing rather than appearing,
 * so the card never jumps.
 */
export function UpdateSection({
  updater,
  fallbackVersion,
  onInstall,
}: {
  updater: Updater | null;
  fallbackVersion?: string;
  onInstall: () => void;
}) {
  const { t, i18n } = useTranslation();
  const checking = Boolean(updater?.checking);
  const update = updater?.update ?? null;
  const manual = update?.installMode === "manual";
  const error = updater?.error && !manual ? updater.error : null;
  const kind: Kind = checking
    ? "checking"
    : error
      ? "error"
      : update?.available
        ? "available"
        : update
          ? "upToDate"
          : "idle";

  // While a new look is under way the last answer stays on screen, dimmed,
  // instead of collapsing and reopening a moment later.
  const held = useRef<{ kind: Kind; update: typeof update; error: string | null }>({ kind, update, error });
  if (!checking) held.current = { kind, update, error };
  const shown = checking ? held.current : { kind, update, error };

  const current = update?.currentVersion ?? fallbackVersion ?? "—";
  const latest = update?.latestVersion ?? "—";
  const size = typeof update?.size === "number" && update.size > 0 ? ` · ${formatBytes(update.size)}` : "";
  const checkedAt = updater?.lastCheckedAt
    ? new Intl.DateTimeFormat(i18n.language, { hour: "2-digit", minute: "2-digit" }).format(updater.lastCheckedAt)
    : null;

  const text: { title: string; sub: string | null; testId?: string; role?: "status" } = {
    idle: { title: `v${current}`, sub: t("about.idleHint") },
    checking: { title: t("about.checking"), sub: t("about.checkingHint"), testId: "update-checking", role: "status" as const },
    upToDate: {
      title: `${current} · ${t("about.upToDate")}`,
      sub: checkedAt ? t("about.checkedAt", { time: checkedAt }) : null,
      role: "status" as const,
    },
    available: { title: `${t("about.updateAvailable")} · ${latest}${size}`, sub: `${current} → ${latest}` },
    error: { title: t("about.checkFailed"), sub: t("about.checkFailedHint") },
  }[kind];

  const extras = shown.kind === "available" || shown.kind === "error";

  return (
    <section className="border-b border-border p-4 sm:p-5" data-testid="update-section" data-state={kind}>
      <SectionLabel>{t("about.appUpdate")}</SectionLabel>
      <div className="mt-3 flex items-center gap-4">
        <UpdateOrb kind={kind} />
        <div className="min-h-[44px] min-w-0 flex-1">
          <div key={kind} className="update-text-in">
            <p
              className={kind === "available" ? "truncate text-[14.5px] font-semibold text-primary" : "truncate text-[14.5px] font-semibold text-foreground"}
              data-testid={text.testId}
              role={text.role}
            >
              {text.title}
            </p>
            {text.sub ? <p className="mt-0.5 truncate text-[12.5px] text-muted-foreground">{text.sub}</p> : null}
          </div>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="min-w-[8.75rem] justify-center"
          data-testid="check-update"
          loading={checking}
          disabled={!updater || checking || updater.installing}
          onClick={() => void updater?.check()}
        >
          <IconRefresh />
          {checking ? t("about.checking") : t("about.checkUpdate")}
        </Button>
      </div>

      <div className="update-bar mt-3" data-active={checking || undefined} aria-hidden="true">
        <span />
      </div>

      <Reveal open={extras}>
        <div className={checking ? "pointer-events-none opacity-50 transition-opacity" : "transition-opacity"}>
          {shown.kind === "error" && shown.error ? (
            <div className="space-y-2 pt-3">
              <ErrorBanner
                message={shown.error}
                onRetry={() => void updater?.check()}
                retryLabel={t("common.retry")}
              />
              <Button
                size="sm"
                variant="outline"
                data-testid="open-update-release-on-error"
                onClick={() => void openExternal(shown.update?.releasePage || PUBLIC_RELEASE_PAGE)}
              >
                <IconExternal />
                {t("about.openReleasePage")}
              </Button>
            </div>
          ) : null}

          {shown.kind === "available" && shown.update ? (
            <div className="flex flex-wrap items-center gap-3 pt-3">
              {shown.update.installMode === "manual" ? (
                <div className="min-w-0 max-w-2xl flex-1">
                  <p className="text-sm text-muted-foreground" data-testid="manual-update-message">
                    {t(
                      shown.update.reason === "signatureKeyMismatch"
                        ? "about.manualSignatureKeyMismatch"
                        : shown.update.reason === "bootstrapRequired"
                          ? "about.manualBootstrapRequired"
                          : "about.manualUpdateRequired",
                    )}
                  </p>
                  {shown.update.detail?.message ? (
                    <details className="mt-2" data-testid="update-error-details">
                      <summary className="cursor-pointer text-xs text-muted-foreground">{t("about.updateDetails")}</summary>
                      <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-border bg-muted/40 px-2.5 py-1.5 font-mono text-[12px] leading-snug text-muted-foreground">
                        {shown.update.detail.message}
                      </pre>
                    </details>
                  ) : null}
                </div>
              ) : null}
              <div className="ml-auto">
                {shown.update.installMode === "manual" ? (
                  <Button
                    size="sm"
                    variant="primary"
                    data-testid="open-update-release"
                    onClick={() => void openExternal(shown.update!.releasePage)}
                  >
                    <IconExternal />
                    {t("about.openReleasePage")}
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="primary"
                    data-testid="install-update"
                    disabled={updater?.installing}
                    onClick={onInstall}
                  >
                    <IconDownload />
                    {t("about.installAndRestart")}
                  </Button>
                )}
              </div>
            </div>
          ) : null}
        </div>
      </Reveal>
    </section>
  );
}

/** Five icons on one orb; only the current one is visible, the others wait scaled down. */
function UpdateOrb({ kind }: { kind: Kind }) {
  return (
    <span className="update-orb relative flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl" data-state={kind} aria-hidden="true">
      <span className="update-orb-ring" />
      <span className="update-orb-icon" data-for="idle">
        <IconRefresh size={18} />
      </span>
      <span className="update-orb-icon" data-for="checking">
        <IconRefresh size={18} className="update-orb-spin" />
      </span>
      <span className="update-orb-icon" data-for="upToDate">
        <IconCheck size={20} className="update-orb-check" />
      </span>
      <span className="update-orb-icon" data-for="available">
        <IconDownload size={18} />
      </span>
      <span className="update-orb-icon" data-for="error">
        <IconAlert size={18} />
      </span>
    </span>
  );
}

/** Grows open and closed instead of appearing; the last content stays while it closes. */
function Reveal({ open, children }: { open: boolean; children: ReactNode }) {
  const [mounted, setMounted] = useState(open);
  const [expanded, setExpanded] = useState(open);
  const last = useRef<ReactNode>(children);
  if (open) last.current = children;

  useEffect(() => {
    if (open) {
      setMounted(true);
      const frame = window.requestAnimationFrame(() => setExpanded(true));
      return () => window.cancelAnimationFrame(frame);
    }
    setExpanded(false);
    const timer = window.setTimeout(() => setMounted(false), 420);
    return () => window.clearTimeout(timer);
  }, [open]);

  if (!mounted && !open) return null;
  return (
    <div className="reveal" data-expanded={expanded || undefined}>
      <div className="min-h-0 overflow-hidden">{open ? children : last.current}</div>
    </div>
  );
}
