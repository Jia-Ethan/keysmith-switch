import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { EmptyState } from "../components/EmptyState";
import { ErrorBanner } from "../components/ErrorBanner";
import { IconAlert, IconDownload, IconExternal, IconRefresh } from "../components/icons";
import {
  Button,
  Checkbox,
  Disclosure,
  Mono,
  Panel,
  PanelHeader,
  SectionLabel,
  cx,
} from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { formatArgv, formatBytes } from "../lib/format";
import { toastSafeMessage } from "../lib/redact";
import { openExternal } from "../lib/runtime";
import type {
  AboutInfo,
  OfficialAction,
  OfficialPlan,
  OfficialProduct,
  OfficialProductId,
  UpdateChannel,
  UpdateCheck,
} from "../types";
import { PUBLIC_RELEASE_PAGE } from "../types";

export const APP_UPDATE_AUTO_CHECK_DELAY_MS = 1800;

export function AboutPage({
  channel,
  toast,
  autoCheckDelayMs = null,
}: {
  channel: UpdateChannel;
  toast?: ToastApi;
  autoCheckDelayMs?: number | null;
}) {
  const { t } = useTranslation();
  const [about, setAbout] = useState<AboutInfo | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [update, setUpdate] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);
  const [officialPlan, setOfficialPlan] = useState<OfficialPlan | null>(null);
  const [officialConfirmed, setOfficialConfirmed] = useState(false);
  const [officialBusy, setOfficialBusy] = useState(false);
  const toastRef = useRef(toast);
  toastRef.current = toast;

  const loadAbout = useCallback(async () => {
    try {
      setAbout(await api.getAbout());
      setLoadError(null);
    } catch (err) {
      setAbout(null);
      setLoadError(toastSafeMessage(err) || t("errors.loadFailed"));
    }
  }, [t]);

  const checkUpdate = useCallback(async () => {
    setChecking(true);
    setConfirmed(false);
    setInstallError(null);
    try {
      const result = await api.checkAppUpdate(channel);
      setUpdate(result);
      if (result.error) setInstallError(result.error);
    } catch (err) {
      const message = toastSafeMessage(err) || t("about.updateFailed");
      setUpdate(null);
      setInstallError(message);
      toastRef.current?.err(message);
    } finally {
      setChecking(false);
    }
  }, [channel, t]);

  useEffect(() => {
    void loadAbout();
  }, [loadAbout]);

  useEffect(() => {
    if (autoCheckDelayMs == null) return;
    const timer = window.setTimeout(() => {
      void checkUpdate();
    }, autoCheckDelayMs);
    return () => window.clearTimeout(timer);
  }, [autoCheckDelayMs, checkUpdate]);

  const canInstall = Boolean(update?.available) && confirmed && !installing && !checking;
  const releasePage = update?.releasePage || PUBLIC_RELEASE_PAGE;
  const progress = installing ? 100 : update?.progress;

  const install = async () => {
    if (!canInstall) return;
    setInstalling(true);
    setInstallError(null);
    try {
      const result = await api.installAppUpdate();
      if (!result.ok) {
        const message = result.error || t("about.updateFailed");
        setInstallError(message);
        toast?.err(message);
      } else {
        toast?.ok(t("about.restartRequired"));
      }
    } catch (err) {
      const message = toastSafeMessage(err) || t("about.updateFailed");
      setInstallError(message);
      toast?.err(message);
    } finally {
      setInstalling(false);
    }
  };

  const previewOfficial = async (product: OfficialProductId, action: OfficialAction) => {
    setOfficialBusy(true);
    setOfficialConfirmed(false);
    try {
      setOfficialPlan(await api.planOfficialAction(product, action));
    } catch (err) {
      toast?.err(err);
      setOfficialPlan(null);
    } finally {
      setOfficialBusy(false);
    }
  };

  const runOfficial = async () => {
    if (!officialPlan || !officialConfirmed) return;
    setOfficialBusy(true);
    try {
      const result = await api.confirmOfficialAction(officialPlan.planId);
      if (!result.ok) toast?.err(result.error || t("about.officialBlocked"));
      else toast?.ok(t("common.success"));
      await loadAbout();
      setOfficialPlan(null);
      setOfficialConfirmed(false);
    } catch (err) {
      toast?.err(err);
    } finally {
      setOfficialBusy(false);
    }
  };

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-3xl flex-col gap-2 overflow-auto">
      {loadError ? (
        <ErrorBanner message={loadError} onRetry={() => void loadAbout()} retryLabel={t("common.retry")} />
      ) : null}

      {/* Layer 1 — the app itself */}
      <Panel>
        <PanelHeader
          title={`${t("about.layerApp")} · ${t("about.appUpdate")}`}
          actions={
            <>
              <Button
                size="sm"
                data-testid="check-update"
                disabled={checking || installing}
                onClick={() => void checkUpdate()}
              >
                <IconRefresh />
                {t("about.checkUpdate")}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void openExternal(releasePage)}>
                <IconExternal />
                <span className="hidden sm:inline">{t("about.releasePage")}</span>
              </Button>
            </>
          }
        />
        <div className="px-3 py-2.5">
          <p className="mb-2 rounded-md border border-amber-600/40 bg-amber-500/10 px-3 py-2 text-sm text-amber-800 dark:text-amber-300">
            {t("app.unsignedNotice")}
          </p>
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1 text-sm sm:grid-cols-[auto_minmax(0,1fr)_auto_minmax(0,1fr)]">
            <Row label={t("about.currentVersion")}>
              {update?.currentVersion ?? about?.app.version ?? "—"}
            </Row>
            <Row label={t("about.latestVersion")}>{update?.latestVersion ?? "—"}</Row>
            <Row label={t("about.size")}>{formatBytes(update?.size)}</Row>
            <Row label={t("settings.updateChannel")}>
              {update?.channel ?? channel}
            </Row>
          </dl>

          <div className="mt-2.5">
            <div
              className="h-1 overflow-hidden rounded bg-muted"
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.max(0, Math.min(100, progress ?? 0))}
            >
              <div
                className="h-full bg-primary transition-all"
                style={{ width: `${Math.max(0, Math.min(100, progress ?? (checking ? 30 : 0)))}%` }}
              />
            </div>
          </div>

          <div className="mt-2 min-h-[18px] text-[12px]">
            {checking ? <p className="text-muted-foreground">{t("about.checking")}</p> : null}
            {!checking && !update && !installError ? (
              <p className="text-muted-foreground">{t("about.noUpdateYet")}</p>
            ) : null}
            {!checking && update && !update.available && !update.error ? (
              <p className="text-primary">{t("about.upToDate")}</p>
            ) : null}
            {!checking && update?.available ? (
              <p className="font-medium text-primary">{t("about.updateAvailable")}</p>
            ) : null}
          </div>

          {installError ? (
            <div className="mt-2 flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-[12px] text-destructive">
              <IconAlert size={14} className="mt-px shrink-0" />
              <div className="min-w-0">
                <p className="font-medium">{t("about.updateFailed")}</p>
                <p>{t("about.keptCurrent")}</p>
                <p className="mt-0.5 break-words opacity-90">{installError}</p>
              </div>
            </div>
          ) : null}

          {update?.notes ? (
            <div className="mt-2">
              <SectionLabel>{t("about.notes")}</SectionLabel>
              <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap rounded-md border border-border bg-muted/40 px-2 py-1.5 text-[11px] leading-relaxed">
                {update.notes}
              </pre>
            </div>
          ) : null}

          <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-2 border-t border-border pt-2.5">
            <Checkbox
              data-testid="confirm-update"
              checked={confirmed}
              disabled={!update?.available || installing}
              label={t("about.confirmUpdate")}
              hint={!update?.available ? undefined : t("about.installBlockedHint")}
              onChange={(event) => setConfirmed(event.target.checked)}
            />
            <Button
              variant="primary"
              data-testid="install-update"
              disabled={!canInstall}
              className="ml-auto"
              onClick={() => void install()}
            >
              <IconDownload />
              {installing ? t("about.installing") : t("about.installAndRestart")}
            </Button>
          </div>
          <p className="mt-1.5 text-[11px] text-muted-foreground">{t("about.autoCheck")}</p>
        </div>
      </Panel>

      {/* Layer 2 — bundled Keysmith adapters */}
      <Panel>
        <PanelHeader title={`${t("about.layerAdapters")} · ${t("about.adapters")}`} />
        <div className="px-3 py-2">
          <p className="mb-2 text-[11px] text-muted-foreground">{t("about.adaptersHint")}</p>
          {about?.adapters?.length ? (
            <ul className="flex flex-col">
              {about.adapters.map((item) => (
                <li
                  key={item.tool}
                  className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-border py-1.5 text-[12px] last:border-b-0"
                >
                  <span className="min-w-[92px] font-medium capitalize text-foreground">{item.tool}</span>
                  <Mono className="text-foreground">{item.version}</Mono>
                  <span className="text-[11px] text-muted-foreground">
                    {item.bundled ? t("about.adapterBundled") : t("about.adapterExternal")}
                  </span>
                  <span className="text-[11px] text-muted-foreground">{t("about.noHotUpdate")}</span>
                  {item.path ? (
                    <Mono className="ml-auto max-w-full truncate" >
                      <span title={item.path}>{item.path}</span>
                    </Mono>
                  ) : null}
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[12px] text-muted-foreground">{t("common.none")}</p>
          )}
        </div>
      </Panel>

      {/* Layer 3 — official CLI products */}
      <Panel>
        <PanelHeader title={`${t("about.layerOfficial")} · ${t("about.official")}`} />
        <div className="px-3 py-2">
          <p className="mb-2 text-[11px] text-muted-foreground">{t("about.officialHint")}</p>
          {about?.official?.length ? (
            <div className="flex flex-col gap-1.5">
              {about.official.map((product) => (
                <OfficialRow
                  key={product.product}
                  product={product}
                  busy={officialBusy}
                  onPlan={previewOfficial}
                />
              ))}
            </div>
          ) : (
            <EmptyState title={t("common.none")} testId="about-official-empty" />
          )}

          {officialPlan ? (
            <div
              className="mt-2.5 rounded-lg border border-border bg-muted/40 p-2.5 text-[12px]"
              data-testid="official-plan"
            >
              <p className="font-medium capitalize text-foreground">
                {officialPlan.product} / {officialPlan.action}
              </p>
              <dl className="mt-1.5 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1">
                <Row label={t("about.command")}>
                  <Mono>{formatArgv(officialPlan.argv)}</Mono>
                </Row>
                <Row label={t("about.dest")}>
                  <Mono>{officialPlan.dest || "—"}</Mono>
                </Row>
                <Row label={t("about.source")}>{officialPlan.source || "—"}</Row>
              </dl>

              {officialPlan.blockers.length > 0 ? (
                <div className="mt-2 flex items-start gap-2 rounded border border-destructive/40 bg-destructive/10 px-2 py-1.5 text-destructive">
                  <IconAlert size={14} className="mt-px shrink-0" />
                  <ul className="min-w-0 list-inside list-disc">
                    {officialPlan.blockers.map((item, index) => (
                      <li key={index}>{item}</li>
                    ))}
                  </ul>
                </div>
              ) : (
                <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-2">
                  <Checkbox
                    checked={officialConfirmed}
                    data-testid="confirm-official"
                    label={t("about.confirmOfficial")}
                    onChange={(event) => setOfficialConfirmed(event.target.checked)}
                  />
                  <Button
                    size="sm"
                    variant="primary"
                    className="ml-auto"
                    data-testid="run-official"
                    disabled={!officialConfirmed || officialBusy}
                    onClick={() => void runOfficial()}
                  >
                    {t("about.runOfficial")}
                  </Button>
                </div>
              )}
            </div>
          ) : null}
        </div>
      </Panel>
    </div>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="whitespace-nowrap text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-words text-foreground">{children}</dd>
    </>
  );
}

function OfficialRow({
  product,
  busy,
  onPlan,
}: {
  product: OfficialProduct;
  busy: boolean;
  onPlan: (product: OfficialProductId, action: OfficialAction) => void;
}) {
  const { t } = useTranslation();
  const action: OfficialAction = product.installed ? "update" : "install";
  const blocked = !product.available;

  return (
    <article className="rounded-md border border-border px-2.5 py-2">
      <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1">
        <h3 className="text-[12px] font-semibold capitalize text-foreground">{product.product}</h3>
        <span
          className={cx(
            "inline-flex items-center rounded border px-1.5 py-px text-[10px] font-medium",
            product.installed
              ? "border-primary/30 bg-primary/10 text-primary"
              : "border-border bg-muted text-muted-foreground",
          )}
        >
          {product.installed ? t("about.installed") : t("about.notInstalled")}
        </span>
        <Mono className="text-foreground">
          {product.currentVersion ?? "—"} → {product.latestVersion ?? "—"}
        </Mono>
        {!blocked ? (
          <Button
            size="sm"
            className="ml-auto"
            disabled={busy}
            data-testid={`official-plan-${product.product}`}
            onClick={() => onPlan(product.product, action)}
          >
            {t("about.planAction")}
          </Button>
        ) : null}
      </div>

      {blocked ? (
        <p className="mt-1.5 flex items-start gap-1.5 text-[11px] text-amber-600 dark:text-amber-500">
          <IconAlert size={12} className="mt-px shrink-0" />
          <span className="min-w-0">
            {product.product === "zcode" ? t("about.zcodeMacOnly") : t("about.officialBlocked")}
            {product.unavailableReason ? ` · ${product.unavailableReason}` : ""}
          </span>
        </p>
      ) : null}

      <Disclosure title={t("common.details")} testId={`official-details-${product.product}`}>
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-[11px]">
          <Row label={t("about.executable")}>
            <Mono>{product.executablePath ?? "—"}</Mono>
          </Row>
          <Row label={t("about.source")}>{product.source || "—"}</Row>
          <Row label={t("about.command")}>
            <Mono>{formatArgv(product.argv)}</Mono>
          </Row>
          <Row label={t("about.dest")}>
            <Mono>{product.dest || "—"}</Mono>
          </Row>
        </dl>
      </Disclosure>
    </article>
  );
}
