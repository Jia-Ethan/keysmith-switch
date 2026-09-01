import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { Envelope, ToolId, ToolInfo } from "../types";
import { shortPath } from "../lib/format";
import { StatusBadge } from "./StatusBadge";
import { Button, Disclosure, IconButton, Mono } from "./ui";
import { IconAlert, IconRefresh } from "./icons";
import { ToolLogo } from "./ToolLogos";

/**
 * 紧凑状态条。正常状态只显示工具和状态；CLI、版本、指纹和写入路径收入 advanced 展开。
 */
export function ToolStatusBar({
  tool,
  toolInfo,
  status,
  doctorOk,
  recovery,
  unavailable,
  busy,
  onRefresh,
  onRecover,
  advancedEnabled,
}: {
  tool: ToolId;
  toolInfo: ToolInfo;
  status: Envelope | null;
  doctorOk: boolean | undefined;
  recovery: boolean;
  unavailable: boolean;
  busy: boolean;
  onRefresh: () => void;
  onRecover: () => void;
  advancedEnabled: boolean;
}) {
  const { t } = useTranslation();
  const adapterVersion = toolInfo.adapterVersion || status?.adapterVersion || "—";
  const writePaths = status?.targetPaths ?? [];

  return (
    <div className="rounded-2xl border border-border bg-card">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 px-4 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <ToolLogo tool={tool} size={20} />
          <h1 className="truncate text-[20px] font-semibold text-foreground">{toolInfo.name}</h1>
        </div>

        <StatusBadge status={status?.status ?? (unavailable ? "unavailable" : null)} />

        {doctorOk === false ? (
          <span className="inline-flex items-center gap-1 text-[14px] text-amber-600 dark:text-amber-500">
            <IconAlert size={14} />
            {t("tool.doctorFail")}
          </span>
        ) : null}

        <div className="ml-auto flex shrink-0 items-center gap-1">
          {recovery ? (
            <Button
              size="sm"
              variant="primary"
              disabled={busy || unavailable}
              data-testid="tool-recover"
              onClick={onRecover}
            >
              {t("tool.recoveryAction")}
            </Button>
          ) : null}
          <IconButton
            label={t("tool.refreshStatus")}
            data-testid="tool-refresh"
            disabled={busy}
            onClick={onRefresh}
          >
            <IconRefresh />
          </IconButton>
        </div>
      </div>

      {recovery ? (
        <p
          className="flex items-start gap-2 border-t border-border bg-amber-500/10 px-4 py-2.5 text-[14px] text-amber-700 dark:text-amber-400"
          data-testid="tool-recovery-notice"
        >
          <IconAlert size={14} className="mt-px shrink-0" />
          <span className="min-w-0">{t("tool.recoveryRequired")}</span>
        </p>
      ) : null}

      {advancedEnabled ? (
        <div className="border-t border-border p-2">
          <Disclosure title={t("tool.diagnostics")} testId="tool-diagnostics">
            <dl className="space-y-1.5 text-[13px]">
              <Row label={t("tool.adapterVersion")}>
                <span>v{adapterVersion}</span>
              </Row>
              <Row label={t("tool.cliPath")}>
                <Mono>{status?.cliPath ? shortPath(status.cliPath, 72) : "—"}</Mono>
              </Row>
              <Row label={t("tool.doctor")}>
                <span>
                  {doctorOk === undefined
                    ? t("common.unknown")
                    : doctorOk
                      ? t("tool.doctorOk")
                      : t("tool.doctorFail")}
                </span>
              </Row>
              <Row label={t("plan.fingerprintCurrent")}>
                <Mono>{status?.currentFingerprint ?? "—"}</Mono>
              </Row>
              <Row label={t("tool.writePaths")}>
                {writePaths.length === 0 ? (
                  <span className="text-muted-foreground">{t("common.none")}</span>
                ) : (
                  <ul className="space-y-0.5">
                    {writePaths.map((item, index) => (
                      <li key={index} className="break-all">
                        <span className="text-muted-foreground">{item.role}</span>{" "}
                        <Mono>{item.path}</Mono>
                        {item.exists ? "" : " · ∅"}
                      </li>
                    ))}
                  </ul>
                )}
              </Row>
            </dl>
          </Disclosure>
        </div>
      ) : null}
    </div>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap gap-x-2">
      <dt className="w-[92px] shrink-0 text-muted-foreground">{label}</dt>
      <dd className="min-w-0 flex-1">{children}</dd>
    </div>
  );
}
