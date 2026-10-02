import { useTranslation } from "react-i18next";
import type { ToolId } from "../types";
import { useAnnouncementsOptional } from "./AnnouncementsProvider";
import { IconBell, IconClose } from "./icons";

/**
 * Pinned announcements for one agent's page: the ones for every agent and the ones for this
 * agent, until the person closes them. Plain text, kept short; the full text is in the list.
 */
export function PinnedAnnouncements({ tool, onOpenAll }: { tool: ToolId; onOpenAll?: () => void }) {
  const { t } = useTranslation();
  const ann = useAnnouncementsOptional();
  const pinned = (ann?.view?.items ?? []).filter(
    (item) => item.pinned && !item.dismissed && (item.tools.length === 0 || item.tools.includes(tool)),
  );
  if (!ann || pinned.length === 0) return null;
  return (
    <div className="flex flex-col gap-2" data-testid="pinned-announcements">
      {pinned.map((item) => (
        <div
          key={item.id}
          role="status"
          data-testid="pinned-announcement"
          className="flex items-start gap-2 rounded-xl border border-primary/25 bg-primary/8 px-3.5 py-2.5 text-[12.5px] text-foreground"
        >
          <IconBell size={14} className="mt-px shrink-0 text-primary" />
          <div className="min-w-0 flex-1">
            <p className="font-medium">{item.title}</p>
            <p className="mt-0.5 line-clamp-3 whitespace-pre-wrap break-words text-muted-foreground">{item.body}</p>
            {onOpenAll ? (
              <button
                type="button"
                className="mt-1 text-[12px] font-medium text-primary hover:underline"
                data-testid="pinned-announcement-more"
                onClick={onOpenAll}
              >
                {t("announcements.viewAll")}
              </button>
            ) : null}
          </div>
          <button
            type="button"
            title={t("announcements.dismiss")}
            aria-label={t("announcements.dismiss")}
            data-testid="pinned-announcement-dismiss"
            className="shrink-0 rounded-md p-0.5 text-muted-foreground hover:bg-card hover:text-foreground"
            onClick={() => void ann.dismiss(item.id)}
          >
            <IconClose size={13} />
          </button>
        </div>
      ))}
    </div>
  );
}
