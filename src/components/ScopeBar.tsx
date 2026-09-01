import { useTranslation } from "react-i18next";
import type { ScopeId } from "../types";
import { scopeNeedsProjectDir } from "../lib/tools";
import { shortPath } from "../lib/format";
import { Button, Input, Segmented, Select, cx } from "./ui";
import { IconFolder } from "./icons";

/**
 * Scope selector plus the project directory picker that project / local need.
 * Only scopes the adapter actually reports are rendered, so non-Claude tools
 * never show a fabricated project / local entry.
 */
export function ScopeBar({
  scope,
  supportedScopes,
  projectDir,
  recentProjectDirs,
  disabled,
  onScopeChange,
  onProjectDirChange,
  onBrowse,
}: {
  scope: ScopeId;
  supportedScopes: ScopeId[];
  projectDir: string;
  recentProjectDirs: string[];
  disabled: boolean;
  onScopeChange: (scope: ScopeId) => void;
  onProjectDirChange: (dir: string) => void;
  onBrowse: () => void;
}) {
  const { t } = useTranslation();
  const needsProject = scopeNeedsProjectDir(scope);
  const projectMissing = needsProject && !projectDir.trim();

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-2xl border border-border bg-card px-3 py-2.5 sm:px-4">
      <span className="text-[14px] font-medium text-muted-foreground">{t("scope.label")}</span>

      {supportedScopes.length > 1 ? (
        <Segmented
          value={scope}
          ariaLabel={t("scope.label")}
          disabled={disabled}
          onChange={(next) => onScopeChange(next)}
          options={supportedScopes.map((item) => ({
            value: item,
            label: t(`scope.${item}`),
          }))}
        />
      ) : (
        <span
          className="inline-flex h-9 items-center rounded-xl border border-border bg-muted px-3 text-[15px] font-medium text-foreground"
          data-testid="scope-single"
        >
          {t(`scope.${supportedScopes[0] ?? "user"}`)}
        </span>
      )}

      {needsProject ? (
        <div className="flex min-w-[240px] flex-1 flex-wrap items-center gap-1.5">
          <div className="relative min-w-[180px] flex-1">
            <Input
              value={projectDir}
              disabled={disabled}
              aria-label={t("scope.projectDir")}
              aria-invalid={projectMissing || undefined}
              placeholder={t("scope.projectDirPlaceholder")}
              title={projectDir || undefined}
              onChange={(event) => onProjectDirChange(event.target.value)}
              className={cx(projectMissing && "border-amber-600/60")}
              data-testid="scope-project-dir"
            />
          </div>
          <Button
            size="md"
            disabled={disabled}
            onClick={onBrowse}
            data-testid="scope-browse"
            title={t("scope.browse")}
          >
            <IconFolder />
            <span className="hidden sm:inline">{t("scope.browse")}</span>
          </Button>
          {recentProjectDirs.length > 0 ? (
            <Select
              value=""
              disabled={disabled}
              aria-label={t("scope.recent")}
              className="w-auto max-w-[180px]"
              onChange={(event) => {
                if (event.target.value) onProjectDirChange(event.target.value);
              }}
              data-testid="scope-recent"
            >
              <option value="">{t("scope.recent")}</option>
              {recentProjectDirs.map((dir) => (
                <option key={dir} value={dir}>
                  {shortPath(dir, 40)}
                </option>
              ))}
            </Select>
          ) : null}
        </div>
      ) : null}

      {projectMissing ? (
        <p className="w-full text-[14px] text-amber-600 dark:text-amber-500" data-testid="scope-project-required">
          {t("scope.needsProjectDir")}
        </p>
      ) : null}
    </div>
  );
}
