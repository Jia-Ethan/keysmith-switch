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
      "about.installAndRestart",
      "tool.zcodeNoInstall",
    ]) {
      expect(zhKeys).toContain(key);
    }
  });
});
