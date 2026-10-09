import { describe, expect, it } from "vitest";
import i18n from "../i18n";
import { localizeReason, needsCleanup } from "./planFailure";
import type { Envelope } from "../types";

const MISSING =
  "existing deployment manifest ownership conflict: top-level config.toml model_instructions_file is missing; expected it to still reference ./gpt-overlay.md. Uninstall will leave the current config.toml unchanged and will not restore the pre-deployment backup";
const DRY = "dry-run found 1 confirmed blocker(s); no files were changed.";
const PERM_EN =
  "Deployment failed; starting rollback: [Errno 5] cannot open filesystem handle: F:\\Apps\\Codex\\home\\.keysmith-write-abc\\previous: Access is denied.";
const PERM_ZH =
  "Deployment failed; starting rollback: [Errno 5] cannot open filesystem handle: F:\\Apps\\Codex\\home\\.keysmith-write-abc\\previous: 拒绝访问。";

describe("localizeReason", () => {
  it("says the missing model_instructions_file in Chinese and points to Clean up", async () => {
    await i18n.changeLanguage("zh-CN");
    const text = localizeReason(MISSING, i18n.t);
    expect(text).toContain("./gpt-overlay.md");
    expect(text).not.toMatch(/ownership conflict|Uninstall/);
    expect(localizeReason(DRY, i18n.t)).toBe("预检发现 1 个阻塞问题，没有改动任何文件。");
  });

  it("passes unknown reasons through", () => {
    expect(localizeReason("something else", i18n.t)).toBe("something else");
  });

  it("localizes Windows ACL permission denied (English OS error text)", async () => {
    await i18n.changeLanguage("zh-CN");
    const result = localizeReason(PERM_EN, i18n.t);
    expect(result).toContain("权限不足");
    expect(result).not.toMatch(/\[Errno|Access is denied|cannot open filesystem handle/);
  });

  it("localizes Windows ACL permission denied (Chinese OS error text)", async () => {
    await i18n.changeLanguage("zh-CN");
    const result = localizeReason(PERM_ZH, i18n.t);
    expect(result).toContain("权限不足");
    expect(result).not.toMatch(/\[Errno|拒绝访问|cannot open filesystem handle/);
  });

  it("says in words that Grok finds no valid deployment record", async () => {
    await i18n.changeLanguage("zh-CN");
    const text = localizeReason("no valid deployment manifest", i18n.t);
    expect(text).toContain("找不到有效的部署记录");
    expect(text).not.toMatch(/manifest/);
  });

  it("offers Cleanup only for the Codex missing-line case", () => {
    const env = { blockers: [MISSING], error: null } as unknown as Envelope;
    expect(needsCleanup("codex", env)).toBe(true);
    expect(needsCleanup("claude", env)).toBe(false);
  });
});
