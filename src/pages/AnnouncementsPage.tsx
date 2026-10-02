import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useAnnouncements } from "../components/AnnouncementsProvider";
import { Callout } from "../components/PlanPreview";
import { IconAlert, IconBell, IconExternal, IconRefresh } from "../components/icons";
import { Button, cx } from "../components/ui";
import { openExternal } from "../lib/runtime";
import type { Announcement } from "../types";

/**
 * Announcements from the publisher: what a version brings, what comes next, how to sign in.
 * Opening the page marks what is shown as read. Text is plain; a link opens in the browser.
 */
export function AnnouncementsPage() {
  const { t, i18n } = useTranslation();
  const ann = useAnnouncements();
  const view = ann.view;
  const markedRef = useRef<string>("");

  // What the person has seen here is no longer new. Marked once per set of unread ids.
  useEffect(() => {
    const unread = (view?.items ?? []).filter((item) => !item.read).map((item) => item.id);
    const key = unread.join(",");
    if (unread.length === 0 || markedRef.current === key) return;
    markedRef.current = key;
    void ann.markRead(unread);
  }, [view, ann]);

  const date = (value: string) => {
    const parsed = new Date(value);
    return Number.isNaN(parsed.getTime())
      ? value.slice(0, 10)
      : new Intl.DateTimeFormat(i18n.language, { year: "numeric", month: "short", day: "numeric" }).format(parsed);
  };

  return (
    <section className="h-full min-h-0 w-full overflow-y-auto" data-testid="announcements-page">
      <div className="mx-auto flex w-full max-w-[860px] flex-col gap-5 px-4 pb-10 pt-6 sm:px-6">
        <header className="flex flex-wrap items-center gap-4">
          <span className="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary/12 text-primary ring-1 ring-primary/20">
            <IconBell size={24} />
          </span>
          <div className="min-w-0 flex-1">
            <h1 className="text-[22px] font-semibold tracking-[-0.02em] text-foreground">{t("announcements.title")}</h1>
            <p className="mt-0.5 text-[13px] text-muted-foreground">{t("announcements.lead")}</p>
          </div>
          <Button
            size="sm"
            variant="outline"
            loading={ann.refreshing}
            disabled={ann.refreshing}
            data-testid="announcements-refresh"
            onClick={() => void ann.refresh()}
          >
            <IconRefresh />
            {ann.refreshing ? t("announcements.checking") : t("announcements.refresh")}
          </Button>
        </header>

        {view?.error && view.error !== "offline" ? (
          <Callout tone="warn" icon={<IconAlert size={14} />}>
            <span data-testid="announcements-error">
              {t(`announcements.err.${view.error}`, { defaultValue: t("announcements.err.default") })}
            </span>
          </Callout>
        ) : null}

        {view && view.items.length === 0 ? (
          <div className="surface-card px-6 py-10 text-center" data-testid="announcements-empty">
            <p className="text-[15px] font-semibold text-foreground">{t("announcements.empty")}</p>
            <p className="mt-1 text-[13px] text-muted-foreground">{t("announcements.emptyHint")}</p>
          </div>
        ) : null}

        {view && view.items.length > 0 ? (
          <ul className="flex flex-col gap-3" data-testid="announcements-list">
            {view.items.map((item) => (
              <AnnouncementCard key={item.id} item={item} date={date(item.publishedAt)} />
            ))}
          </ul>
        ) : null}
      </div>
    </section>
  );
}

function AnnouncementCard({ item, date }: { item: Announcement; date: string }) {
  const { t } = useTranslation();
  return (
    <li className="surface-card flex flex-col gap-2 p-4" data-testid="announcement" data-unread={!item.read || undefined}>
      <div className="flex flex-wrap items-center gap-2 text-[12px] text-muted-foreground">
        <span
          className={cx(
            "rounded-full px-2 py-0.5 font-medium",
            item.kind === "release" && "bg-success/12 text-success",
            item.kind === "preview" && "bg-primary/12 text-primary",
            item.kind === "news" && "bg-muted text-foreground",
          )}
        >
          {t(`announcements.kind.${item.kind}`)}
        </span>
        <span>{date}</span>
        {item.tools.map((tool) => (
          <span key={tool} className="rounded-full bg-muted px-2 py-0.5">
            {t(`nav.${tool}`)}
          </span>
        ))}
        {!item.read ? <span className="h-1.5 w-1.5 rounded-full bg-destructive" aria-label={t("announcements.new")} /> : null}
      </div>
      <h2 className="text-[15px] font-semibold text-foreground">{item.title}</h2>
      <p className="whitespace-pre-wrap break-words text-[13.5px] leading-relaxed text-foreground/90">{item.body}</p>
      {item.link ? (
        <div>
          <Button size="sm" variant="ghost" data-testid="announcement-link" onClick={() => void openExternal(item.link ?? "")}>
            <IconExternal />
            {t("announcements.openLink")}
          </Button>
        </div>
      ) : null}
    </li>
  );
}
