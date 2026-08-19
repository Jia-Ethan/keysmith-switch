import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ErrorBanner } from "../components/ErrorBanner";
import { Button, Card } from "../components/ui";
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
  autoCheckDelayMs = APP_UPDATE_AUTO_CHECK_DELAY_MS,
}: {
  channel: UpdateChannel;
  toast?: ToastApi;
  autoCheckDelayMs?: number;
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
    <div className="mx-auto flex max-w-3xl flex-col gap-3">
      {loadError ? <ErrorBanner message={loadError} onRetry={() => void loadAbout()} retryLabel={t("common.retry")} /> : null}

      <Card title={`${t("about.layerApp")} · ${t("about.appUpdate")}`}>
        <p className="mb-2 text-[12px] text-ink-200">{t("about.autoCheck")}</p>
        <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-[12px]">
          <Row label={t("about.currentVersion")} value={update?.currentVersion ?? about?.app.version ?? "0.1.0"} />
          <Row label={t("about.latestVersion")} value={update?.latestVersion ?? "—"} />
          <Row label={t("about.size")} value={formatBytes(update?.size)} />
          <Row
            label={t("about.restartRequired")}
            value={update?.restartRequired ? t("common.yes") : t("common.no")}
          />
        </dl>
        {update?.notes ? (
          <div className="mt-2 rounded bg-ink-900 p-2 text-[12px] whitespace-pre-wrap">{update.notes}</div>
        ) : null}
        <div className="mt-2">
          <div className="mb-1 text-[11px] text-ink-200">{t("about.progress")}</div>
          <div className="h-1.5 overflow-hidden rounded bg-ink-700">
            <div
              className="h-full bg-accent-600 transition-all"
              style={{ width: `${Math.max(0, Math.min(100, progress ?? (checking ? 30 : 0)))}%` }}
            />
          </div>
        </div>
        {checking ? <p className="mt-2 text-[12px] text-ink-200">{t("about.checking")}</p> : null}
        {update && !update.available && !update.error ? (
          <p className="mt-2 text-[12px] text-teal-200">{t("about.upToDate")}</p>
        ) : null}
        {update?.available ? <p className="mt-2 text-[12px] text-teal-200">{t("about.updateAvailable")}</p> : null}
        {installError ? (
          <div className="mt-2 text-[12px] text-rose-200">
            <p>{t("about.updateFailed")}</p>
            <p>{t("about.keptCurrent")}</p>
            <p>{installError}</p>
          </div>
        ) : null}

        <label className="mt-3 flex items-center gap-2 text-[12px]">
          <input
            type="checkbox"
            data-testid="confirm-update"
            checked={confirmed}
            disabled={!update?.available || installing}
            onChange={(event) => setConfirmed(event.target.checked)}
          />
          {t("about.confirmUpdate")}
        </label>

        <div className="mt-3 flex flex-wrap gap-2">
          <Button onClick={() => void checkUpdate()} disabled={checking || installing}>
            {t("about.checkUpdate")}
          </Button>
          <Button
            variant="primary"
            data-testid="install-update"
            disabled={!canInstall}
            onClick={() => void install()}
          >
            {installing ? t("about.installing") : t("about.installAndRestart")}
          </Button>
          <Button onClick={() => void openExternal(releasePage)}>{t("about.releasePage")}</Button>
        </div>
      </Card>

      <Card title={`${t("about.layerAdapters")} · ${t("about.adapters")}`}>
        <p className="mb-2 text-[12px] text-ink-200">{t("about.adaptersHint")}</p>
        <ul className="space-y-1 text-[12px]">
          {(about?.adapters ?? []).map((item) => (
            <li key={item.tool} className="flex flex-wrap gap-2">
              <span className="font-medium">{item.tool}</span>
              <span className="font-mono">{item.version}</span>
              <span className="text-ink-200">{t("about.noHotUpdate")}</span>
              {item.path ? <span className="font-mono text-ink-200">{item.path}</span> : null}
            </li>
          ))}
          {!about?.adapters?.length ? <li className="text-ink-200">{t("common.none")}</li> : null}
        </ul>
      </Card>

      <Card title={`${t("about.layerOfficial")} · ${t("about.official")}`}>
        <p className="mb-2 text-[12px] text-ink-200">{t("about.officialHint")}</p>
        <div className="grid gap-2">
          {(about?.official ?? []).map((product) => (
            <OfficialCard
              key={product.product}
              product={product}
              busy={officialBusy}
              onPlan={previewOfficial}
            />
          ))}
          {!about?.official?.length ? <p className="text-[12px] text-ink-200">{t("common.none")}</p> : null}
        </div>

        {officialPlan ? (
          <div className="mt-3 rounded border border-ink-200/10 bg-ink-900 p-3 text-[12px]">
            <p className="font-medium">
              {officialPlan.product} / {officialPlan.action}
            </p>
            <p className="mt-1 font-mono">{formatArgv(officialPlan.argv)}</p>
            <p className="mt-1">{officialPlan.dest}</p>
            {officialPlan.blockers.length > 0 ? (
              <ul className="mt-1 text-rose-200">
                {officialPlan.blockers.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            ) : (
              <label className="mt-2 flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={officialConfirmed}
                  onChange={(event) => setOfficialConfirmed(event.target.checked)}
                />
                {t("about.confirmOfficial")}
              </label>
            )}
            <Button
              className="mt-2"
              variant="primary"
              disabled={!officialConfirmed || officialPlan.blockers.length > 0 || officialBusy}
              onClick={() => void runOfficial()}
            >
              {t("about.runOfficial")}
            </Button>
          </div>
        ) : null}
      </Card>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-ink-200">{label}</dt>
      <dd>{value}</dd>
    </>
  );
}

function OfficialCard({
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
    <article className="rounded border border-ink-200/10 bg-ink-900/60 p-2">
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-[13px] font-semibold capitalize">{product.product}</h3>
        <span className="text-[11px] text-ink-200">
          {product.installed ? t("about.installed") : t("about.notInstalled")}
        </span>
        <span className="font-mono text-[11px]">
          {product.currentVersion ?? "—"} → {product.latestVersion ?? "—"}
        </span>
      </div>
      <dl className="mt-1 grid grid-cols-[88px_1fr] gap-y-0.5 text-[11px]">
        <dt className="text-ink-200">{t("about.executable")}</dt>
        <dd className="truncate font-mono">{product.executablePath ?? "—"}</dd>
        <dt className="text-ink-200">{t("about.source")}</dt>
        <dd className="truncate">{product.source || "—"}</dd>
        <dt className="text-ink-200">{t("about.command")}</dt>
        <dd className="truncate font-mono">{formatArgv(product.argv)}</dd>
        <dt className="text-ink-200">{t("about.dest")}</dt>
        <dd className="truncate font-mono">{product.dest || "—"}</dd>
      </dl>
      {blocked ? (
        <p className="mt-1 text-[11px] text-amber-200">
          {product.product === "zcode" ? t("about.zcodeMacOnly") : t("about.officialBlocked")}
          {product.unavailableReason ? ` · ${product.unavailableReason}` : ""}
        </p>
      ) : (
        <Button className="mt-2" disabled={busy} onClick={() => onPlan(product.product, action)}>
          {t("about.planAction")}
        </Button>
      )}
    </article>
  );
}
