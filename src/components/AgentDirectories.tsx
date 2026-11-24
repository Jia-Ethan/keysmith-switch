import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { runtimeTargetKey } from "../lib/harnessState";
import type { RuntimeDirectories, RuntimeInfo, RuntimeTarget, Settings, SettingsPatch, ToolId } from "../types";
import { Dropdown } from "./Dropdown";
import { IconCheck } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { Button } from "./ui";

const TOOLS: ToolId[] = ["claude", "codex", "grok", "zcode", "pi", "dsh"];

function sameTarget(left: RuntimeTarget, right: RuntimeTarget): boolean {
  return runtimeTargetKey(left) === runtimeTargetKey(right);
}

function configuredDirectory(settings: Settings, tool: ToolId, target: RuntimeTarget): string {
  return settings.agentDirectories.find((entry) => sameTarget(entry.target, target))?.directories[tool] ?? "";
}

function defaultDirectory(runtime: RuntimeInfo | undefined, tool: ToolId): string {
  const home = runtime?.home;
  if (!home) return "";
  const child: Record<ToolId, string> = {
    claude: ".claude",
    codex: ".codex",
    grok: ".grok",
    zcode: ".zcode",
    pi: ".pi/agent",
    dsh: ".dsh",
  };
  const separator = home.includes("\\") ? "\\" : "/";
  return `${home.replace(/[\\/]$/, "")}${separator}${child[tool].replace("/", separator)}`;
}

function withDirectory(
  settings: Settings,
  target: RuntimeTarget,
  tool: ToolId,
  value: string,
): RuntimeDirectories[] {
  const entries = settings.agentDirectories.map((entry) => ({
    target: entry.target,
    directories: { ...entry.directories },
  }));
  let entry = entries.find((item) => sameTarget(item.target, target));
  if (!entry) {
    entry = { target, directories: {} };
    entries.push(entry);
  }
  const trimmed = value.trim();
  if (trimmed) entry.directories[tool] = trimmed;
  else delete entry.directories[tool];
  return entries.filter((item) => Object.keys(item.directories).length > 0);
}

export function AgentDirectories({
  settings,
  runtimes,
  busy,
  onSave,
}: {
  settings: Settings;
  runtimes: RuntimeInfo[];
  busy: boolean;
  onSave: (patch: SettingsPatch) => Promise<void>;
}) {
  const { t } = useTranslation();
  const native = runtimes.find((runtime) => runtime.target.kind === "native")?.target ?? { kind: "native" as const };
  const selected = useMemo(
    () => Object.fromEntries(TOOLS.map((tool) => [tool, settings.agentRuntimeTargets[tool] ?? native])) as Record<ToolId, RuntimeTarget>,
    [native, settings.agentRuntimeTargets],
  );
  const [drafts, setDrafts] = useState<Record<string, string>>({});

  useEffect(() => setDrafts({}), [settings.agentDirectories]);

  const saveRuntime = async (tool: ToolId, key: string) => {
    const target = runtimes.find((runtime) => runtimeTargetKey(runtime.target) === key)?.target;
    if (!target) return;
    await onSave({
      agentRuntimeTargets: { ...settings.agentRuntimeTargets, [tool]: target },
    });
  };

  const saveDirectory = async (tool: ToolId, target: RuntimeTarget, value: string) => {
    await onSave({ agentDirectories: withDirectory(settings, target, tool, value) });
  };

  return (
    <section className="surface-card overflow-hidden" data-testid="agent-directories">
      <div className="border-b border-border px-4 py-3 sm:px-5">
        <h2 className="text-[14px] font-semibold text-foreground">{t("settings.agentEnvironments")}</h2>
      </div>
      <div className="divide-y divide-border">
        {TOOLS.map((tool) => {
          const target = selected[tool];
          const targetKey = runtimeTargetKey(target);
          const runtime = runtimes.find((item) => runtimeTargetKey(item.target) === targetKey);
          const draftKey = `${tool}\0${targetKey}`;
          const stored = configuredDirectory(settings, tool, target);
          const value = drafts[draftKey] ?? stored;
          const choices = runtimes;
          return (
            <article key={tool} className="grid gap-3 px-4 py-3.5 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(12rem,1fr)] sm:px-5">
              <div className="flex min-w-0 items-center gap-2.5">
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border bg-background">
                  <ToolLogo tool={tool} size={18} />
                </span>
                <span className="truncate text-[13.5px] font-medium">{t(`nav.${tool}`)}</span>
              </div>
              <div className="grid min-w-0 gap-2">
                <Dropdown
                  label={t("runtime.label")}
                  value={targetKey}
                  options={choices.map((item) => ({ value: runtimeTargetKey(item.target), label: item.label }))}
                  disabled={busy}
                  onChange={(key) => void saveRuntime(tool, key)}
                  testId={`settings-runtime-${tool}`}
                  className="w-full"
                />
                <div className="flex min-w-0 gap-2">
                  <label className="min-w-0 flex-1">
                    <span className="sr-only">{t("settings.agentDirectory", { tool: t(`nav.${tool}`) })}</span>
                    <input
                      value={value}
                      onChange={(event) => setDrafts((current) => ({ ...current, [draftKey]: event.target.value }))}
                      placeholder={defaultDirectory(runtime, tool) || t("settings.agentDirectoryDefault")}
                      disabled={busy || runtime?.available === false}
                      data-testid={`settings-directory-${tool}`}
                      className="h-9 w-full rounded-md border border-border bg-background px-3 font-mono text-[12.5px] text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:border-primary focus:ring-1 focus:ring-primary"
                    />
                  </label>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busy || value === stored || runtime?.available === false}
                    onClick={() => void saveDirectory(tool, target, value)}
                    data-testid={`settings-directory-save-${tool}`}
                  >
                    <IconCheck />
                    {t("common.save")}
                  </Button>
                </div>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
