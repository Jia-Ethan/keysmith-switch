import { describe, expect, it } from "vitest";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";

function flattenKeys(value: unknown, prefix = ""): string[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }
  return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) =>
    flattenKeys(child, prefix ? `${prefix}.${key}` : key),
  );
}

describe("i18n locales", () => {
  const zhKeys = flattenKeys(zhCN).sort();
  const twKeys = flattenKeys(zhTW).sort();
  const enKeys = flattenKeys(en).sort();

  it("keeps the same keys for zh-CN, zh-TW, and en", () => {
    expect(twKeys).toEqual(zhKeys);
    expect(enKeys).toEqual(zhKeys);
  });

  it("covers required product copy", () => {
    for (const key of [
      "nav.claude",
      "nav.codex",
      "nav.grok",
      "nav.zcode",
      "nav.settings",
      "nav.about",
      "nav.advanced",
      "prompts.empty",
      "about.adapters",
      "about.official",
      "tool.zcodeNoInstall",
    ]) {
      expect(zhKeys).toContain(key);
    }
  });

  it("does not expose build-signing or Preview labels in UI copy", () => {
    for (const locale of [zhCN, zhTW, en]) {
      const copy = JSON.stringify(locale).toLowerCase();
      expect(copy).not.toContain("unsigned preview");
      expect(copy).not.toContain("preview ·");
      expect(copy).not.toContain("developer id");
      expect(copy).not.toContain("authenticode");
      expect(copy).not.toContain("未签名");
      expect(copy).not.toContain("未簽名");
    }
  });
});
