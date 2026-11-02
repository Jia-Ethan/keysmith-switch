import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { extensionErrorCode, useExtensions } from "../components/ExtensionsProvider";
import { HarnessPlanView } from "../components/HarnessPlanView";
import { Callout } from "../components/PlanPreview";
import * as api from "../api";
import { IconAlert, IconCheck, IconDownload, IconPuzzle, IconRefresh, IconShield, IconSwap, IconTerminal } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import { Button, cx, Switch } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { formatBytes, relativeTime } from "../lib/format";
import type { ExtensionPack, ExtensionReport, HarnessPlan, HarnessReport, RulePreview } from "../types";

/**
 * Extension packs: prompt bundles that the official source publishes and that update on
 * their own, apart from the app. Off until the person turns it on, because turning it on
 * is what lets the app read the network.
 */
export function ExtensionsPage({ toast }: { toast: ToastApi }) {
  const { t, i18n } = useTranslation();
  const ext = useExtensions();
  const [enabling, setEnabling] = useState(false);
  const [disabling, setDisabling] = useState(false);
  const [removing, setRemoving] = useState<ExtensionPack | null>(null);
  // A rule pack is installed only after its rules have been shown (or a sealed summary).
  const [previewing, setPreviewing] = useState<{ pack: ExtensionPack; preview: RulePreview | null } | null>(null);
  // A harness pack is written only from a plan the person has read, file by file.
  const [planning, setPlanning] = useState<{ pack: ExtensionPack; plan: HarnessPlan | null } | null>(null);
  const [updatingAll, setUpdatingAll] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const view = ext.view;

  // "3 minutes ago" has to move on by itself while the page stays open.
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  // Packs with an update lead, so the rail's count points at the first cards.
  // "Update all" never includes a harness pack: each of those is its own reviewed plan.
  const packs = useMemo(() => {
    const list = view?.packs ?? [];
    return [...list.filter((pack) => pack.updateAvailable), ...list.filter((pack) => !pack.updateAvailable)];
  }, [view]);
  const pending = packs.filter((pack) => pack.updateAvailable && pack.kind !== "harness");

  const say = (report: ExtensionReport, kind: "install" | "update" | "remove", pack?: ExtensionPack) => {
    if (pack?.kind === "harness") {
      toast.ok(t("extensions.harness.removedToast", { removed: report.removed, kept: report.kept }));
      return;
    }
    if (pack?.kind === "rules") {
      toast.ok(t(kind === "remove" ? "extensions.rules.removedToast" : kind === "update" ? "extensions.rules.updatedToast" : "extensions.rules.installedToast"));
      return;
    }
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
      setDisabling(false);
    }
  };

  const check = async () => {
    const fresh = await ext.refresh();
    if (!fresh || fresh.error) return;
    if (fresh.updates > 0) toast.info(t("extensions.foundUpdates", { count: fresh.updates }));
    else if (fresh.packs.length > 0) toast.ok(t("extensions.upToDate"));
  };

  const updateAll = async () => {
    setUpdatingAll(true);
    const total = { added: 0, updated: 0, copied: 0, linked: 0, kept: 0, removed: 0 };
    let done = 0;
    try {
      // One at a time: the backend writes to the same library for each pack.
      for (const pack of pending) {
        const report = await ext.install(pack.id);
        for (const key of Object.keys(total) as (keyof ExtensionReport)[]) total[key] += report[key];
        done += 1;
      }
      toast.ok(t("extensions.updatedAllToast", { count: done, ...total }));
    } catch (error) {
      if (done > 0) toast.info(t("extensions.updatedSomeToast", { done, total: pending.length }));
      fail(error);
    } finally {
      setUpdatingAll(false);
    }
  };

  const install = async (pack: ExtensionPack, enable = false) => {
    const update = pack.installedVersion !== null;
    try {
      say(await ext.install(pack.id, enable), update ? "update" : "install", pack);
    } catch (error) {
      fail(error);
    }
  };

  const preview = async (pack: ExtensionPack) => {
    setPreviewing({ pack, preview: null });
    try {
      const next = await api.previewExtensionRules(pack.id);
      setPreviewing((current) => (current?.pack.id === pack.id ? { pack, preview: next } : current));
    } catch (error) {
      setPreviewing(null);
      fail(error);
    }
  };

  const plan = async (pack: ExtensionPack) => {
    setPlanning({ pack, plan: null });
    try {
      const next = await api.planHarness(pack.id);
      setPlanning((current) => (current?.pack.id === pack.id ? { pack, plan: next } : current));
    } catch (error) {
      setPlanning(null);
      fail(error);
    }
  };

  const confirmPlan = async () => {
    const current = planning;
    if (!current?.plan) return;
    const update = current.plan.fromVersion !== null;
    try {
      const report: HarnessReport = await ext.confirmHarness(current.pack.id, current.plan.planId);
      toast.ok(t(update ? "extensions.harness.updatedToast" : "extensions.harness.installedToast", { ...report }));
      setPlanning(null);
    } catch (error) {
      fail(error);
      // Whatever changed, the plan on screen is spent: read the pack and the disk again.
      if (extensionErrorCode(error) === "changed") void plan(current.pack);
    }
  };

  const remove = async (pack: ExtensionPack) => {
    setRemoving(null);
    try {
      say(await ext.uninstall(pack.id), "remove", pack);
    } catch (error) {
      fail(error);
    }
  };

  const checkedMs = view?.checkedAt ? new Date(view.checkedAt).getTime() : NaN;
  const checkedAt = Number.isNaN(checkedMs)
    ? null
    : now - checkedMs < 45_000
      ? t("extensions.checkedJustNow")
      : t("extensions.lastChecked", { time: relativeTime(view?.checkedAt, i18n.language, now) });
  const checkedAtExact = view?.checkedAt
    ? new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(view.checkedAt))
    : undefined;
  const locked = ext.busyId !== null || updatingAll;

  return (
    <section className="h-full min-h-0 w-full overflow-y-auto" data-testid="extensions-page" data-enabled={ext.enabled || undefined}>
      <div className="mx-auto flex w-full max-w-[1120px] flex-col gap-5 px-4 pb-10 pt-6 sm:px-6">
        <header className="flex flex-wrap items-start gap-x-4 gap-y-2">
          <div className="min-w-0 flex-1">
            <h1 className="flex items-center gap-2 text-[22px] font-semibold tracking-[-0.02em] text-foreground">
              <IconPuzzle size={20} className="text-primary" />
              {t("extensions.title")}
            </h1>
            <p className="mt-1 text-[13px] text-muted-foreground">{t("extensions.lead")}</p>
          </div>
          {ext.enabled ? (
            <div className="flex shrink-0 items-center gap-2">
              {checkedAt ? (
                <span className="hidden text-[12px] text-muted-foreground sm:inline" title={checkedAtExact} data-testid="extensions-checked-at">
                  {checkedAt}
                </span>
              ) : null}
              {pending.length >= 2 ? (
                <Button size="sm" variant="primary" loading={updatingAll} disabled={locked || ext.refreshing} data-testid="extensions-update-all" onClick={() => void updateAll()}>
                  <IconDownload size={13} />
                  {t("extensions.updateAll", { count: pending.length })}
                </Button>
              ) : null}
              <Button size="sm" variant="outline" loading={ext.refreshing} disabled={ext.refreshing || locked} data-testid="extensions-refresh" onClick={() => void check()}>
                <IconRefresh />
                {ext.refreshing ? t("extensions.checking") : t("extensions.refresh")}
              </Button>
              <Switch
                checked
                busy={enabling}
                disabled={locked}
                aria-label={t("extensions.switchLabel")}
                data-testid="extensions-disable"
                onCheckedChange={() => setDisabling(true)}
              />
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

        {ext.enabled && view && packs.length > 0 ? (
          <ul className="grid grid-cols-1 gap-3 md:grid-cols-2" data-testid="extensions-list">
            {packs.map((pack, index) => (
              <PackCard
                key={pack.id}
                pack={pack}
                index={index}
                busy={ext.busyId === pack.id}
                locked={locked}
                onInstall={() =>
                  void (pack.kind === "harness"
                    ? plan(pack)
                    : pack.kind === "rules" && pack.installedVersion === null
                      ? preview(pack)
                      : install(pack))
                }
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
        <p className="text-[13.5px] text-muted-foreground">
          {removing?.kind === "harness"
            ? t("extensions.harness.uninstallBody")
            : removing?.kind === "rules"
              ? t("extensions.rules.uninstallBody")
              : t("extensions.uninstallBody")}
        </p>
      </ConfirmDialog>

      <ConfirmDialog
        open={previewing !== null}
        wide
        title={t("extensions.rules.previewTitle", { name: previewing?.pack.name ?? "" })}
        confirmLabel={t("extensions.rules.installEnable")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="extensions-rules-install-enable"
        confirmDisabled={!previewing?.preview}
        busy={previewing ? ext.busyId === previewing.pack.id : false}
        onClose={() => setPreviewing(null)}
        onConfirm={() => {
          const current = previewing;
          if (!current) return;
          void install(current.pack, true).then(() => setPreviewing(null));
        }}
        footerStart={
          <Button
            size="sm"
            variant="ghost"
            data-testid="extensions-rules-install-only"
            disabled={!previewing?.preview || (previewing ? ext.busyId === previewing.pack.id : false)}
            onClick={() => {
              const current = previewing;
              if (!current) return;
              void install(current.pack, false).then(() => setPreviewing(null));
            }}
          >
            {t("extensions.rules.installOnly")}
          </Button>
        }
      >
        <p className="mb-3 text-[13px] text-muted-foreground">
          {(previewing?.preview?.opaque ?? previewing?.pack.opaque)
            ? t("extensions.rules.previewBodySealed")
            : t("extensions.rules.previewBody")}
        </p>
        {previewing?.preview?.opaque ? (
          <p className="text-[13.5px] text-foreground" data-testid="extensions-rules-preview-sealed">
            {t("extensions.rules.sealedCount", { count: previewing.preview.count })}
          </p>
        ) : previewing?.preview ? (
          <ul
            className="grid max-h-[50vh] grid-cols-[minmax(0,max-content)_auto_minmax(0,1fr)] gap-x-2 gap-y-0.5 overflow-y-auto font-mono text-[12.5px]"
            data-testid="extensions-rules-preview"
          >
            {previewing.preview.rules.map((rule) => (
              <li key={rule.from} className="contents">
                <span className="truncate text-foreground">{rule.from}</span>
                <span className="text-muted-foreground">→</span>
                <span className="truncate text-foreground">{rule.to || "∅"}</span>
              </li>
            ))}
          </ul>
        ) : (
          <div className="skeleton h-16 w-full" aria-hidden="true" />
        )}
      </ConfirmDialog>

      <ConfirmDialog
        open={planning !== null}
        wide
        title={
          planning?.plan?.fromVersion
            ? t("extensions.harness.planTitleUpdate", { name: planning.pack.name, from: planning.plan.fromVersion, to: planning.plan.version })
            : t("extensions.harness.planTitle", { name: planning?.pack.name ?? "" })
        }
        confirmLabel={planning?.plan?.fromVersion ? t("extensions.harness.confirmUpdate") : t("extensions.harness.confirmInstall")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="extensions-harness-confirm"
        confirmDisabled={!planning?.plan || planning.plan.blocked || planning.plan.nothingToDo}
        busy={planning ? ext.busyId === planning.pack.id : false}
        onClose={() => setPlanning(null)}
        onConfirm={() => void confirmPlan()}
      >
        {planning?.plan ? (
          <HarnessPlanView plan={planning.plan} />
        ) : (
          <div className="flex flex-col gap-2" aria-label={t("extensions.harness.planLoading")}>
            <div className="skeleton h-4 w-2/3" />
            <div className="skeleton h-16 w-full" />
          </div>
        )}
      </ConfirmDialog>

      <ConfirmDialog
        open={disabling}
        title={t("extensions.disableTitle")}
        confirmLabel={t("extensions.disable")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="extensions-confirm-disable"
        busy={enabling}
        onClose={() => setDisabling(false)}
        onConfirm={() => void enable(false)}
      >
        <p className="text-[13.5px] text-muted-foreground">{t("extensions.disableBody")}</p>
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
      ? { text: t("extensions.updateFromTo", { from: pack.installedVersion, to: pack.version }), tone: "primary" as const }
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
          {pack.kind === "rules" ? <IconSwap size={20} /> : pack.kind === "harness" ? <IconTerminal size={20} /> : <IconPuzzle size={20} />}
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
        </div>
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[12px] text-muted-foreground">
        <span className="flex items-center gap-1">
          {pack.tools.map((tool) => (
            <ToolLogo key={tool} tool={tool} size={14} />
          ))}
        </span>
        <span>
          {pack.kind === "rules"
            ? t("extensions.rules.kind")
            : pack.kind === "harness"
              ? t("extensions.harness.kind", { count: pack.components?.length ?? 0 })
              : t("extensions.items", { count: pack.itemCount })}
        </span>
        <span>{formatBytes(pack.size)}</span>
        <span className="font-mono">v{pack.version}</span>
      </div>
      {pack.kind === "harness" && pack.components && pack.components.length > 0 ? (
        <ul className="mt-2.5 flex flex-wrap gap-1.5" data-testid={`extension-components-${pack.id}`}>
          {pack.components.map((component) => (
            <li
              key={component.id}
              className="rounded-md bg-muted/70 px-1.5 py-0.5 font-mono text-[11.5px] text-muted-foreground"
              title={`${t(`extensions.harness.types.${component.type}`)} · ${component.title}`}
            >
              {component.type === "skill" || component.type === "command" ? `/${component.name}` : component.name}
            </li>
          ))}
        </ul>
      ) : null}
      {installed && ((pack.modifiedFiles ?? 0) > 0 || (pack.missingFiles ?? 0) > 0) ? (
        <p className="mt-2 text-[12px] text-warning" data-testid={`extension-drift-${pack.id}`}>
          {[
            (pack.modifiedFiles ?? 0) > 0 ? t("extensions.harness.modified", { count: pack.modifiedFiles }) : null,
            (pack.missingFiles ?? 0) > 0 ? t("extensions.harness.missing", { count: pack.missingFiles }) : null,
          ]
            .filter(Boolean)
            .join(" · ")}
        </p>
      ) : null}
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
              {installed
                ? t("extensions.update")
                : pack.kind === "harness"
                  ? t("extensions.harness.view")
                  : pack.kind === "rules"
                  ? pack.opaque
                    ? t("extensions.rules.install")
                    : t("extensions.rules.view")
                  : t("extensions.install")}
            </Button>
          ) : null}
        </div>
      </div>
    </li>
  );
}
