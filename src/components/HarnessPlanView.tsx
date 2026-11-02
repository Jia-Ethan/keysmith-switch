import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Callout } from "./PlanPreview";
import { IconAlert, IconInfo } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { cx } from "./ui";
import type { HarnessFileAction, HarnessPlan, HarnessPlannedFile, ToolId } from "../types";

const TOOL_NAMES: Record<ToolId, string> = {
  claude: "Claude Code",
  codex: "Codex",
  grok: "Grok Build",
  zcode: "ZCode",
  pi: "pi",
  dsh: "DSH",
};

const ACTION_TONE: Record<HarnessFileAction, string> = {
  add: "bg-success/12 text-success",
  update: "bg-primary/12 text-primary",
  same: "bg-muted text-muted-foreground",
  "keep-modified": "bg-warning/12 text-warning",
  remove: "bg-muted text-muted-foreground",
  release: "bg-warning/12 text-warning",
  conflict: "bg-destructive/10 text-destructive",
};

/**
 * What a harness pack would write: every file, the agent it is for, where it goes, and its
 * full text (or, for an update, the diff). Nothing here is written until the person confirms.
 */
export function HarnessPlanView({ plan }: { plan: HarnessPlan }) {
  const { t } = useTranslation();
  // Unchanged files only add noise; they stay ours and are not listed.
  const files = plan.files.filter((file) => file.action !== "same");
  return (
    <div className="flex flex-col gap-3" data-testid="harness-plan">
      <p className="text-[13px] text-muted-foreground">{t("extensions.harness.planBody")}</p>
      {plan.blocked ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          <span data-testid="harness-plan-blocked">{t("extensions.harness.planBlocked")}</span>
        </Callout>
      ) : null}
      {plan.nothingToDo ? (
        <p className="text-[13px] text-foreground" data-testid="harness-plan-nothing">
          {t("extensions.harness.planNothing")}
        </p>
      ) : null}
      {plan.sharedSkills ? (
        <Callout tone="info" icon={<IconInfo size={14} />}>
          <span data-testid="harness-plan-shared">{t("extensions.harness.sharedSkills")}</span>
        </Callout>
      ) : null}
      {plan.skippedTools.length > 0 ? (
        <p className="text-[12.5px] text-muted-foreground" data-testid="harness-plan-skipped">
          {t("extensions.harness.skipped", { tools: plan.skippedTools.map((tool) => TOOL_NAMES[tool]).join(", ") })}
        </p>
      ) : null}
      {files.length > 0 ? (
        <ul className="flex max-h-[52vh] flex-col gap-1.5 overflow-y-auto pr-1" data-testid="harness-plan-files">
          {files.map((file) => (
            <PlannedFileRow key={`${file.tool}:${file.path}`} file={file} />
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function PlannedFileRow({ file }: { file: HarnessPlannedFile }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const body = file.action === "update" ? file.diff : file.content;
  const reason = file.reason?.startsWith("other-pack:")
    ? t("extensions.harness.reasons.otherPack", { pack: file.reason.slice("other-pack:".length) })
    : file.reason
      ? t(`extensions.harness.reasons.${file.reason}`, { defaultValue: file.reason })
      : null;
  return (
    <li
      className={cx("rounded-lg border border-border/70 bg-background/60 px-3 py-2", file.action === "conflict" && "border-destructive/40")}
      data-testid={`harness-file-${file.action}`}
    >
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <ToolLogo tool={file.tool} size={14} />
        <span className="rounded bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
          {t(`extensions.harness.types.${file.type}`)}
        </span>
        <span className="font-mono text-[12.5px] text-foreground">
          {file.type === "skill" || file.type === "command" ? `/${file.name}` : file.name}
        </span>
        <span className={cx("ml-auto rounded-full px-2 py-0.5 text-[11px] font-medium", ACTION_TONE[file.action])}>
          {t(`extensions.harness.actions.${file.action}`)}
        </span>
      </div>
      <div className="mt-1 flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-muted-foreground" title={file.path}>
          {file.display}
        </span>
        {body ? (
          <button
            type="button"
            className="shrink-0 text-[11.5px] font-medium text-primary hover:underline"
            data-testid="harness-file-toggle"
            onClick={() => setOpen((value) => !value)}
          >
            {open ? t("extensions.harness.hideText") : t("extensions.harness.showText")}
          </button>
        ) : null}
      </div>
      {reason ? <p className="mt-1 text-[12px] text-destructive">{reason}</p> : null}
      {open && body ? (
        <pre
          className="mt-2 max-h-[36vh] overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted/60 p-2.5 font-mono text-[11.5px] leading-relaxed text-foreground"
          data-testid="harness-file-body"
        >
          {body}
        </pre>
      ) : null}
    </li>
  );
}
