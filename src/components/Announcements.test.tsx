import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Announcement, AnnouncementsView } from "../types";
import { AnnouncementsPage } from "../pages/AnnouncementsPage";
import { AnnouncementsProvider } from "./AnnouncementsProvider";
import { PinnedAnnouncements } from "./PinnedAnnouncements";

const announcementsState = vi.fn();
const announcementsRefresh = vi.fn();
const markAnnouncements = vi.fn();
const openExternal = vi.fn();

vi.mock("../api", () => ({
  announcementsState: (...args: unknown[]) => announcementsState(...args),
  announcementsRefresh: (...args: unknown[]) => announcementsRefresh(...args),
  markAnnouncements: (...args: unknown[]) => markAnnouncements(...args),
}));
vi.mock("../lib/runtime", () => ({ openExternal: (url: string) => openExternal(url) }));

function item(overrides: Partial<Announcement> = {}): Announcement {
  return {
    id: "a",
    kind: "news",
    publishedAt: "2026-10-02T08:00:00Z",
    title: "ZCode 请用 API 登录",
    body: "第一行\n第二行",
    pinned: false,
    tools: [],
    link: null,
    read: false,
    dismissed: false,
    ...overrides,
  };
}

function view(items: Announcement[]): AnnouncementsView {
  return { items, unread: items.filter((i) => !i.read).length, fetchedAt: null, error: null };
}

describe("announcements", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    announcementsRefresh.mockRejectedValue(new Error("offline"));
  });

  it("lists announcements as plain text, marks them read, and opens links in the browser", async () => {
    const link = "https://github.com/Jia-Ethan/keysmith-switch-releases";
    announcementsState.mockResolvedValue(view([item({ link, body: "<b>not bold</b>" })]));
    markAnnouncements.mockResolvedValue(view([item({ link, body: "<b>not bold</b>", read: true })]));
    render(
      <AnnouncementsProvider>
        <AnnouncementsPage />
      </AnnouncementsProvider>,
    );
    const card = await screen.findByTestId("announcement");
    expect(card).toHaveTextContent("<b>not bold</b>");
    expect(card.querySelector("b")).toBeNull();
    await waitFor(() => expect(markAnnouncements).toHaveBeenCalledWith(["a"], false));
    fireEvent.click(screen.getByTestId("announcement-link"));
    expect(openExternal).toHaveBeenCalledWith(link);
  });

  it("shows pinned announcements only on their agent's page, until closed", async () => {
    announcementsState.mockResolvedValue(
      view([item({ id: "z", pinned: true, tools: ["zcode"] }), item({ id: "all", pinned: true, title: "For everyone" })]),
    );
    markAnnouncements.mockResolvedValue(
      view([item({ id: "z", pinned: true, tools: ["zcode"] }), item({ id: "all", pinned: true, read: true, dismissed: true })]),
    );
    const { rerender } = render(
      <AnnouncementsProvider>
        <PinnedAnnouncements tool="claude" />
      </AnnouncementsProvider>,
    );
    const cards = await screen.findAllByTestId("pinned-announcement");
    expect(cards).toHaveLength(1);
    expect(cards[0]).toHaveTextContent("For everyone");

    rerender(
      <AnnouncementsProvider>
        <PinnedAnnouncements tool="zcode" />
      </AnnouncementsProvider>,
    );
    expect(await screen.findAllByTestId("pinned-announcement")).toHaveLength(2);
    await act(async () => {
      fireEvent.click(screen.getAllByTestId("pinned-announcement-dismiss")[1]);
    });
    expect(markAnnouncements).toHaveBeenCalledWith(["all"], true);
    expect(screen.getAllByTestId("pinned-announcement")).toHaveLength(1);
  });

  it("says nothing when there are no announcements to pin", async () => {
    announcementsState.mockResolvedValue(view([]));
    render(
      <AnnouncementsProvider>
        <PinnedAnnouncements tool="claude" />
      </AnnouncementsProvider>,
    );
    await waitFor(() => expect(announcementsState).toHaveBeenCalled());
    expect(screen.queryByTestId("pinned-announcements")).toBeNull();
  });
});
