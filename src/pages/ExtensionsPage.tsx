import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { extensionErrorCode, useExtensions } from "../components/ExtensionsProvider";
import { Callout } from "../components/PlanPreview";
import { IconAlert, IconCheck, IconDownload, IconPuzzle, IconRefresh, IconShield } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import { Button, cx } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { formatBytes } from "../lib/format";
import type { ExtensionPack, ExtensionReport } from "../types";

/**
 * Extension packs: prompt bundles that the official source publishes and that update on
 * their own, apart from the app. Off until the person turns it on, because turning it on
 * is what lets the app read the network.
 */
export function ExtensionsPage({ toast }: { toast: ToastApi }) {
  const { t, i18n } = useTranslation();
  const ext = useExtensions();
  const [enabling, setEnabling] = useState(false);
  const [removing, setRemoving] = useState<ExtensionPack | null>(null);
  const view = ext.view;

  const say = (report: ExtensionReport, kind: "install" | "update" | "remove") => {
    if (kind === "remove") toast.ok(t("extensions.removedToast", { ...report }));
    else if (kind === "update") toast.ok(t("extensions.updatedToast", { ...report }));
    else toast.ok(t("extensions.installedToast", { ...report }));
  };

  const fail = (error: unknown) => toast.err(t(`extensions.err.${extensionErrorCode(error)}`, { defaultValue: t("extensions.err.default") }));

  const enable = async (next: boolean) => {
    setEnabling(true);
    try {
      await ext.setEnabled(next);
      if (next) void ext.refresh();
    } catch (error) {
      fail(error);
    } finally {
      setEnabling(false);
    }
  };

  const install = async (pack: ExtensionPack) => {
    const update = pack.installedVersion !== null;
    try {
      say(await ext.install(pack.id), update ? "update" : "install");
    } catch (error) {
      fail(error);
    }
  };

  const remove = async (pack: ExtensionPack) => {
    setRemoving(null);
    try {
      say(await ext.uninstall(pack.id), "remove");
    } catch (error) {
      fail(error);
    }
  };

  const checkedAt = view?.checkedAt
    ? new Intl.DateTimeFormat(i18n.language, { hour: "2-digit", minute: "2-digit" }).format(new Date(view.checkedAt))
    : null;

  return (
    <section className="h-full min-h-0 w-full overflow-y-auto" data-testid="extensions-page" data-enabled={ext.enabled || undefined}>
      <div className="mx-auto flex w-full max-w-[1120px] flex-col gap-5 px-4 pb-10 pt-6 sm:px-6">
        <header className="flex flex-wrap items-center gap-4">
          <span className="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary/12 text-primary ring-1 ring-primary/20">
            <IconPuzzle size={24} />
          </span>
          <div className="min-w-0 flex-1">
            <h1 className="text-[22px] font-semibold tracking-[-0.02em] text-foreground">{t("extensions.title")}</h1>
            <p className="mt-0.5 text-[13px] text-muted-foreground">{t("extensions.lead")}</p>
          </div>
          {ext.enabled ? (
            <div className="flex shrink-0 items-center gap-2">
              {checkedAt ? <span className="hidden text-[12px] text-muted-foreground sm:inline">{t("extensions.lastChecked", { time: checkedAt })}</span> : null}
              <Button size="sm" variant="outline" loading={ext.refreshing} disabled={ext.refreshing || ext.busyId !== null} data-testid="extensions-refresh" onClick={() => void ext.refresh()}>
                <IconRefresh />
                {ext.refreshing ? t("extensions.checking") : t("extensions.refresh")}
              </Button>
              <Button size="sm" variant="ghost" disabled={enabling} data-testid="extensions-disable" onClick={() => void enable(false)}>
                {t("extensions.disable")}
              </Button>
            </div>
          ) : null}
        </header>

        {!ext.enabled ? (
          <div className="surface-card animate-page-in flex flex-col items-center gap-4 px-6 py-12 text-center" data-testid="extensions-off">
            <span className="flex h-16 w-16 items-center justify-center rounded-[22px] bg-primary/12 text-primary shadow-glow ring-1 ring-primary/20">
              <IconPuzzle size={30} />
            </span>
            <div className="max-w-[460px]">
              <h2 className="text-[17px] font-semibold text-foreground">{t("extensions.offTitle")}</h2>
              <p className="mt-1.5 text-[13.5px] leading-relaxed text-muted-foreground">{t("extensions.offBody")}</p>
              <p className="mt-1.5 text-[12.5px] text-muted-foreground">{t("extensions.notDeployed")}</p>
            </div>
            <Button variant="primary" loading={enabling} data-testid="extensions-enable" onClick={() => void enable(true)}>
              {t("extensions.enable")}
            </Button>
          </div>
        ) : null}

        {ext.enabled && view?.error ? (
          <Callout tone={view.error === "offline" ? "info" : "warn"} icon={<IconAlert size={14} />}>
            <span data-testid="extensions-error">{t(`extensions.err.${view.error}`, { defaultValue: t("extensions.err.default") })}</span>
          </Callout>
        ) : null}

        {ext.enabled && !view ? (
          <ul className="grid grid-cols-1 gap-3 md:grid-cols-2" aria-hidden="true">
            {[0, 1].map((index) => (
              <li key={index} className="surface-card flex h-[150px] flex-col gap-2.5 p-4">
                <div className="skeleton h-4 w-1/2" />
                <div className="skeleton h-3 w-full" />
                <div className="skeleton h-3 w-4/5" />
              </li>
            ))}
          </ul>
        ) : null}

        {ext.enabled && view && view.packs.length === 0 && !view.error ? (
          <div className="surface-card px-6 py-10 text-center" data-testid="extensions-empty">
            <p className="text-[15px] font-semibold text-foreground">{t("extensions.empty")}</p>
            <p className="mt-1 text-[13px] text-muted-foreground">{t("extensions.emptyHint")}</p>
          </div>
        ) : null}

        {ext.enabled && view && view.packs.length > 0 ? (
          <ul className="grid grid-cols-1 gap-3 md:grid-cols-2" data-testid="extensions-list">
            {view.packs.map((pack, index) => (
              <PackCard
                key={pack.id}
                pack={pack}
                index={index}
                busy={ext.busyId === pack.id}
                locked={ext.busyId !== null}
                onInstall={() => void install(pack)}
                onRemove={() => setRemoving(pack)}
              />
            ))}
          </ul>
        ) : null}
      </div>

      <ConfirmDialog
        open={removing !== null}
        danger
        title={t("extensions.uninstall")}
        description={removing?.name}
        confirmLabel={t("extensions.uninstall")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="extensions-confirm-uninstall"
        onClose={() => setRemoving(null)}
        onConfirm={() => removing && void remove(removing)}
      >
        <p className="text-[13.5px] text-muted-foreground">{t("extensions.uninstallBody")}</p>
      </ConfirmDialog>
    </section>
  );
}

function PackCard({
  pack,
  index,
  busy,
  locked,
  onInstall,
  onRemove,
}: {
  pack: ExtensionPack;
  index: number;
  busy: boolean;
  locked: boolean;
  onInstall: () => void;
  onRemove: () => void;
}) {
  const { t } = useTranslation();
  const installed = pack.installedVersion !== null;
  const status = !pack.compatible
    ? { text: t("extensions.needsApp", { version: pack.minAppVersion }), tone: "warn" as const }
    : pack.updateAvailable
      ? { text: t("extensions.updateAvailable", { version: pack.version }), tone: "primary" as const }
      : installed
        ? { text: t("extensions.installed", { version: pack.installedVersion }), tone: "ok" as const }
        : null;
  return (
    <li
      className="surface-card animate-rise flex flex-col p-4"
      style={{ animationDelay: `${Math.min(index, 8) * 40}ms` }}
      data-testid={`extension-${pack.id}`}
      data-state={!pack.compatible ? "incompatible" : pack.updateAvailable ? "update" : installed ? "installed" : "available"}
    >
      <div className="flex items-start gap-3">
        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-primary/20 to-primary/5 text-primary">
          <IconPuzzle size={20} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate text-[15px] font-semibold tracking-[-0.01em] text-foreground">{pack.name}</h3>
            {pack.official ? (
              <span className="inline-flex items-center gap-1 rounded-full bg-primary/12 px-2 py-0.5 text-[11px] font-semibold text-primary" data-testid={`extension-official-${pack.id}`}>
                <IconShield size={11} />
                {t("extensions.official")}
              </span>
            ) : null}
          </div>
          <p className="mt-1 line-clamp-2 text-[12.5px] leading-relaxed text-muted-foreground">{pack.description}</p>
        </div>
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[12px] text-muted-foreground">
        <span className="flex items-center gap-1">
          {pack.tools.map((tool) => (
            <ToolLogo key={tool} tool={tool} size={14} />
          ))}
        </span>
        <span>{t("extensions.items", { count: pack.itemCount })}</span>
        <span>{formatBytes(pack.size)}</span>
        <span className="font-mono">v{pack.version}</span>
      </div>
      <div className="mt-3 flex items-center gap-2">
        {status ? (
          <span
            data-testid={`extension-status-${pack.id}`}
            className={cx(
              "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-[12px] font-medium",
              status.tone === "ok" && "bg-success/12 text-success",
              status.tone === "primary" && "bg-primary/12 text-primary",
              status.tone === "warn" && "bg-warning/12 text-warning",
            )}
          >
            {status.tone === "ok" ? <IconCheck size={12} /> : null}
            {status.text}
          </span>
        ) : null}
        <div className="ml-auto flex items-center gap-1.5">
          {installed ? (
            <Button size="sm" variant="ghost" disabled={locked} data-testid={`extension-uninstall-${pack.id}`} onClick={onRemove}>
              {t("extensions.uninstall")}
            </Button>
          ) : null}
          {pack.compatible && (!installed || pack.updateAvailable) ? (
            <Button size="sm" variant="primary" loading={busy} disabled={locked && !busy} data-testid={`extension-install-${pack.id}`} onClick={onInstall}>
              <IconDownload size={13} />
              {installed ? t("extensions.update") : t("extensions.install")}
            </Button>
          ) : null}
        </div>
      </div>
    </li>
  );
}
