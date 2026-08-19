import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ErrorBanner } from "../components/ErrorBanner";
import { PlanPreview } from "../components/PlanPreview";
import { PromptList } from "../components/PromptList";
import { StatusBadge } from "../components/StatusBadge";
import { ZCodeBanner } from "../components/ZCodeBanner";
import { Button, Card, Field, Input, Select, Textarea } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { toastSafeMessage } from "../lib/redact";
import { canConfirmPlan } from "../lib/planGate";
import { pickDirectory } from "../lib/runtime";
import { shortPath } from "../lib/format";
import {
  defaultScopeFor,
  isRecoveryState,
  mergeTools,
  scopeNeedsProjectDir,
  scopesForTool,
  scopesFromEnvelope,
} from "../lib/tools";
import { isZcodeUnavailable } from "../lib/zcode";
import type {
  Activation,
  Envelope,
  Operation,
  PlanResult,
  PromptDetail,
  PromptSort,
  PromptSummary,
  PromptVersion,
  ScopeId,
  Settings,
  ToolId,
  ToolInfo,
} from "../types";

export function ToolPage({
  tool,
  settings,
  toast,
  onRememberProject,
}: {
  tool: ToolId;
  settings: Settings;
  toast: ToastApi;
  onRememberProject: (dir: string) => void;
}) {
  const { t } = useTranslation();
  const [toolInfo, setToolInfo] = useState<ToolInfo>(() => mergeTools(null).find((item) => item.id === tool)!);
  const [status, setStatus] = useState<Envelope | null>(null);
  const [doctorEnv, setDoctorEnv] = useState<Envelope | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [prompts, setPrompts] = useState<PromptSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<PromptDetail | null>(null);
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState("");
  const [sort, setSort] = useState<PromptSort>("lastUsed");
  const [scope, setScope] = useState<ScopeId>("user");
  const [projectDir, setProjectDir] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState(false);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState({ title: "", content: "", tags: "" });
  const [plan, setPlan] = useState<{ kind: "activate" | "deactivate"; result: PlanResult } | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [versions, setVersions] = useState<PromptVersion[]>([]);
  const [diff, setDiff] = useState<string>("");
  const [operations, setOperations] = useState<Operation[]>([]);
  const [activations, setActivations] = useState<Activation[]>([]);

  const supportedScopes = useMemo(() => {
    const fromStatus = scopesFromEnvelope(status, tool);
    if (fromStatus.length > 0) return fromStatus;
    return scopesForTool(tool, toolInfo.supportedScopes);
  }, [status, tool, toolInfo.supportedScopes]);

  const unavailable = isZcodeUnavailable(toolInfo) || status?.available === false;

  const loadStatus = useCallback(async () => {
    const args: { tool: ToolId; scope?: ScopeId; projectDir?: string } = { tool, scope };
    if (scopeNeedsProjectDir(scope) && projectDir) args.projectDir = projectDir;
    try {
      const envelope = await api.toolStatus(args);
      setStatus(envelope);
      if (envelope.available === false && tool === "zcode") {
        setToolInfo((current) => ({
          ...current,
          available: false,
          unavailableReason: envelope.unavailableReason,
        }));
      }
    } catch (err) {
      setStatus(null);
      setError(toastSafeMessage(err) || t("errors.loadFailed"));
    }
    try {
      setDoctorEnv(await api.doctor(tool));
    } catch {
      setDoctorEnv(null);
    }
  }, [projectDir, scope, t, tool]);

  const loadPrompts = useCallback(async () => {
    try {
      const result = await api.listPrompts({
        tool,
        query: query || undefined,
        tag: tag || undefined,
        sort,
      });
      setPrompts(result.prompts ?? []);
      setError(null);
    } catch (err) {
      setPrompts([]);
      setError(toastSafeMessage(err) || t("errors.apiUnavailable"));
    }
  }, [query, sort, t, tag, tool]);

  const loadOps = useCallback(async () => {
    try {
      const ops = await api.listOperations(tool);
      setOperations(ops.operations ?? []);
    } catch {
      setOperations([]);
    }
    try {
      const acts = await api.listActivations(tool);
      setActivations(acts.activations ?? []);
    } catch {
      setActivations([]);
    }
  }, [tool]);

  const refreshMeta = useCallback(async () => {
    try {
      const listed = await api.listTools();
      const merged = mergeTools(listed.tools).find((item) => item.id === tool);
      if (merged) setToolInfo(merged);
    } catch {
      setToolInfo(mergeTools(null).find((item) => item.id === tool)!);
    }
    await Promise.all([loadStatus(), loadOps()]);
  }, [loadOps, loadStatus, tool]);

  const refreshAll = useCallback(async () => {
    await Promise.all([refreshMeta(), loadPrompts()]);
  }, [loadPrompts, refreshMeta]);

  useEffect(() => {
    setSelectedId(null);
    setDetail(null);
    setCreating(false);
    setEditing(false);
    setPlan(null);
    setPlanError(null);
    setQuery("");
    setTag("");
    setVersions([]);
    setDiff("");
    setPrompts([]);
    const nextScope = defaultScopeFor(tool, scopesForTool(tool), settings.defaultClaudeScope);
    setScope(nextScope);
    setProjectDir("");
    setToolInfo(mergeTools(null).find((item) => item.id === tool)!);
  }, [settings.defaultClaudeScope, tool]);

  useEffect(() => {
    void refreshMeta();
  }, [refreshMeta]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      void loadPrompts();
    }, 180);
    return () => window.clearTimeout(handle);
  }, [loadPrompts]);

  const selectPrompt = async (id: string) => {
    setSelectedId(id);
    setCreating(false);
    setEditing(false);
    setDiff("");
    try {
      const next = await api.getPrompt(id);
      setDetail(next);
      setDraft({ title: next.title, content: next.content, tags: next.tags.join(", ") });
      const history = await api.promptHistory(id);
      setVersions(history.versions ?? []);
    } catch (err) {
      toast.err(err);
    }
  };

  const tagsOf = (value: string) =>
    value
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);

  const requireProject = scopeNeedsProjectDir(scope);
  const projectReady = !requireProject || Boolean(projectDir.trim());

  const startCreate = () => {
    setCreating(true);
    setEditing(true);
    setSelectedId(null);
    setDetail(null);
    setDraft({ title: "", content: "", tags: "" });
    setVersions([]);
  };

  const saveDraft = async () => {
    if (!draft.title.trim()) {
      toast.err(t("errors.validation"));
      return;
    }
    setBusy(true);
    try {
      if (creating) {
        const created = await api.createPrompt({
          tool,
          title: draft.title.trim(),
          content: draft.content,
          tags: tagsOf(draft.tags),
        });
        toast.ok(t("prompts.created"));
        setCreating(false);
        setEditing(false);
        await loadPrompts();
        await selectPrompt(created.id);
      } else if (detail) {
        const updated = await api.updatePrompt({
          id: detail.id,
          title: draft.title.trim(),
          content: draft.content,
          tags: tagsOf(draft.tags),
        });
        toast.ok(t("prompts.updatedOk"));
        setDetail(updated);
        setEditing(false);
        await loadPrompts();
      }
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const duplicateSameTool = async () => {
    if (!detail) return;
    setBusy(true);
    try {
      const created = await api.createPrompt({
        tool,
        title: `${detail.title} copy`,
        content: detail.content,
        tags: detail.tags,
      });
      toast.ok(t("prompts.created"));
      await loadPrompts();
      await selectPrompt(created.id);
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const copyTo = async (targetTool: ToolId) => {
    if (!detail) return;
    setBusy(true);
    try {
      await api.copyPrompt(detail.id, targetTool);
      toast.ok(t("prompts.copied"));
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const removePrompt = async () => {
    if (!detail) return;
    if (!window.confirm(t("prompts.deleteConfirm"))) return;
    setBusy(true);
    try {
      await api.deletePrompt(detail.id);
      toast.ok(t("prompts.deleted"));
      setDetail(null);
      setSelectedId(null);
      await loadPrompts();
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const openPlan = async (kind: "activate" | "deactivate") => {
    if (unavailable) return;
    if (!projectReady) {
      toast.err(t("scope.needsProjectDir"));
      return;
    }
    if (kind === "activate" && !detail) {
      toast.err(t("errors.validation"));
      return;
    }
    setBusy(true);
    setPlanError(null);
    try {
      const result =
        kind === "activate"
          ? await api.planActivate({
              promptId: detail!.id,
              scope,
              projectDir: requireProject ? projectDir : undefined,
            })
          : await api.planDeactivate({
              promptId: detail?.id,
              tool,
              scope,
              projectDir: requireProject ? projectDir : undefined,
            });
      setPlan({ kind, result });
      if (requireProject && projectDir) onRememberProject(projectDir);
    } catch (err) {
      setPlan(null);
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const confirmPlan = async () => {
    if (!plan || !canConfirmPlan(plan.result.envelope) || isRecoveryState(plan.result.envelope)) return;
    setBusy(true);
    setPlanError(null);
    try {
      const result =
        plan.kind === "activate"
          ? await api.activate(plan.result.operationId)
          : await api.deactivate(plan.result.operationId);
      if (!result.envelope.ok || result.envelope.exitCode !== 0) {
        const reason = toastSafeMessage(result.envelope.error || t("plan.failed"));
        setPlanError(reason);
        toast.err(reason);
      } else {
        toast.ok(t("plan.success"));
        setPlan(null);
      }
      await Promise.all([loadStatus(), loadPrompts(), loadOps()]);
      if (detail) await selectPrompt(detail.id);
    } catch (err) {
      const reason = toastSafeMessage(err);
      setPlanError(reason);
      toast.err(reason);
    } finally {
      setBusy(false);
    }
  };

  const recover = async () => {
    setBusy(true);
    try {
      await api.recoverTool({
        tool,
        scope,
        projectDir: requireProject ? projectDir || undefined : undefined,
      });
      toast.ok(t("operations.recover"));
      await Promise.all([loadStatus(), loadPrompts(), loadOps()]);
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const restoreVersion = async (version: number) => {
    if (!detail) return;
    setBusy(true);
    try {
      const restored = await api.restorePromptVersion(detail.id, version);
      toast.ok(t("prompts.restored"));
      setDetail(restored);
      setDraft({ title: restored.title, content: restored.content, tags: restored.tags.join(", ") });
      await loadPrompts();
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const showDiff = async (fromVersion: number, toVersion: number) => {
    if (!detail) return;
    try {
      const result = await api.promptDiff(detail.id, fromVersion, toVersion);
      setDiff(result.unified || result.summary || "");
    } catch (err) {
      toast.err(err);
    }
  };

  const tags = useMemo(() => {
    const set = new Set<string>();
    for (const item of prompts) for (const value of item.tags) set.add(value);
    return [...set].sort();
  }, [prompts]);

  const recovery = isRecoveryState(status);
  const doctorOk = doctorEnv?.doctor?.ok ?? status?.doctor?.ok;
  const writePaths = status?.targetPaths ?? [];

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {error ? <ErrorBanner message={error} onRetry={() => void refreshAll()} retryLabel={t("common.retry")} /> : null}
      <ZCodeBanner tool={toolInfo} />

      <Card>
        <div className="flex flex-wrap items-center gap-2">
          <StatusBadge status={status?.status ?? (unavailable ? "unavailable" : null)} />
          <span className="text-[12px] text-ink-200">
            {t("tool.adapterVersion")} {toolInfo.adapterVersion || status?.adapterVersion || "—"}
          </span>
          <span className="text-[12px] text-ink-200">
            {t("tool.doctor")}: {doctorOk ? t("tool.doctorOk") : t("tool.doctorFail")}
          </span>
          {status?.cliPath ? (
            <span className="font-mono text-[11px] text-ink-200" title={status.cliPath}>
              {t("tool.cliPath")} {shortPath(status.cliPath)}
            </span>
          ) : null}
          <div className="ml-auto flex gap-1">
            <Button onClick={() => void refreshAll()}>{t("tool.refreshStatus")}</Button>
            {recovery ? (
              <Button variant="primary" onClick={() => void recover()} disabled={busy || unavailable}>
                {t("tool.recoveryAction")}
              </Button>
            ) : null}
          </div>
        </div>
        {recovery ? <p className="mt-2 text-[12px] text-amber-200">{t("tool.recoveryRequired")}</p> : null}

        <div className="mt-3 grid gap-2 md:grid-cols-[220px_1fr]">
          <Field label={t("scope.label")}>
            <Select
              value={scope}
              onChange={(event) => setScope(event.target.value as ScopeId)}
              disabled={unavailable}
            >
              {supportedScopes.map((item) => (
                <option key={item} value={item}>
                  {t(`scope.${item}`)}
                </option>
              ))}
            </Select>
          </Field>
          {requireProject ? (
            <Field label={t("scope.projectDir")}>
              <div className="flex gap-1">
                <Input
                  value={projectDir}
                  placeholder={t("scope.projectDirPlaceholder")}
                  onChange={(event) => setProjectDir(event.target.value)}
                />
                <Button
                  onClick={async () => {
                    const picked = await pickDirectory();
                    if (picked) {
                      setProjectDir(picked);
                      onRememberProject(picked);
                    }
                  }}
                >
                  {t("scope.browse")}
                </Button>
              </div>
              {settings.recentProjectDirs.length > 0 ? (
                <Select
                  className="mt-1 w-full"
                  value=""
                  onChange={(event) => {
                    if (event.target.value) setProjectDir(event.target.value);
                  }}
                >
                  <option value="">{t("scope.recent")}</option>
                  {settings.recentProjectDirs.map((dir) => (
                    <option key={dir} value={dir}>
                      {dir}
                    </option>
                  ))}
                </Select>
              ) : null}
            </Field>
          ) : (
            <div />
          )}
        </div>

        <div className="mt-3">
          <h3 className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-ink-200">
            {t("tool.writePaths")}
          </h3>
          {writePaths.length === 0 ? (
            <p className="text-[12px] text-ink-200">{t("common.none")}</p>
          ) : (
            <ul className="space-y-0.5 font-mono text-[11px]">
              {writePaths.map((item) => (
                <li key={item.path}>
                  <span className="text-ink-200">{item.role}</span> {item.path}
                  {item.exists ? "" : " · ∅"}
                </li>
              ))}
            </ul>
          )}
        </div>
      </Card>

      <div className="grid min-h-0 flex-1 gap-3 lg:grid-cols-[280px_1fr]">
        <Card className="flex min-h-0 flex-col">
          <div className="mb-2 flex gap-1">
            <Input
              value={query}
              placeholder={t("common.search")}
              onChange={(event) => setQuery(event.target.value)}
            />
            <Button variant="primary" onClick={startCreate} disabled={unavailable}>
              {t("prompts.new")}
            </Button>
          </div>
          <div className="mb-2 flex gap-1">
            <Select value={tag} onChange={(event) => setTag(event.target.value)}>
              <option value="">{t("prompts.allTags")}</option>
              {tags.map((item) => (
                <option key={item} value={item}>
                  {item}
                </option>
              ))}
            </Select>
            <Select value={sort} onChange={(event) => setSort(event.target.value as PromptSort)}>
              <option value="lastUsed">{t("prompts.sortLastUsed")}</option>
              <option value="updated">{t("prompts.sortUpdated")}</option>
              <option value="title">{t("prompts.sortTitle")}</option>
              <option value="created">{t("prompts.sortCreated")}</option>
            </Select>
          </div>
          <div className="min-h-0 flex-1 overflow-auto">
            <PromptList prompts={prompts} selectedId={selectedId} onSelect={(id) => void selectPrompt(id)} />
          </div>
        </Card>

        <Card className="min-h-0 overflow-auto">
          {creating || detail ? (
            <div className="space-y-3">
              <div className="flex flex-wrap gap-1">
                {!creating && detail ? (
                  <>
                    <Button onClick={() => setEditing((value) => !value)}>{t("prompts.edit")}</Button>
                    <Button onClick={() => void duplicateSameTool()}>{t("prompts.copy")}</Button>
                    <Select
                      defaultValue=""
                      onChange={(event) => {
                        const value = event.target.value as ToolId | "";
                        event.target.value = "";
                        if (value) void copyTo(value);
                      }}
                    >
                      <option value="">{t("prompts.copyTo")}</option>
                      {(["claude", "codex", "grok", "zcode"] as ToolId[])
                        .filter((item) => item !== tool)
                        .map((item) => (
                          <option key={item} value={item}>
                            {t(`nav.${item}`)}
                          </option>
                        ))}
                    </Select>
                    <Button variant="danger" onClick={() => void removePrompt()}>
                      {t("prompts.delete")}
                    </Button>
                    <Button
                      variant="primary"
                      disabled={unavailable || !projectReady || busy}
                      onClick={() => void openPlan("activate")}
                    >
                      {t("prompts.activate")}
                    </Button>
                    <Button
                      disabled={unavailable || !projectReady || busy}
                      onClick={() => void openPlan("deactivate")}
                    >
                      {t("prompts.deactivate")}
                    </Button>
                  </>
                ) : null}
                {(creating || editing) && (
                  <Button variant="primary" disabled={busy} onClick={() => void saveDraft()}>
                    {t("prompts.save")}
                  </Button>
                )}
              </div>

              <Field label={t("prompts.title")}>
                <Input
                  value={draft.title}
                  disabled={!creating && !editing}
                  onChange={(event) => setDraft((current) => ({ ...current, title: event.target.value }))}
                />
              </Field>
              <Field label={t("prompts.tags")}>
                <Input
                  value={draft.tags}
                  disabled={!creating && !editing}
                  placeholder={t("prompts.tagsPlaceholder")}
                  onChange={(event) => setDraft((current) => ({ ...current, tags: event.target.value }))}
                />
              </Field>
              <Field label={t("prompts.content")}>
                <Textarea
                  value={draft.content}
                  disabled={!creating && !editing}
                  onChange={(event) => setDraft((current) => ({ ...current, content: event.target.value }))}
                />
              </Field>

              {!creating && versions.length > 0 ? (
                <section>
                  <h3 className="mb-1 text-[12px] font-semibold">{t("history.title")}</h3>
                  <ul className="space-y-1">
                    {versions.map((item, index) => (
                      <li key={item.version} className="flex items-center gap-2 text-[12px]">
                        <span className="font-mono">
                          {t("history.version")} {item.version}
                        </span>
                        <span className="text-ink-200">{item.createdAt}</span>
                        <Button onClick={() => void restoreVersion(item.version)}>{t("prompts.restore")}</Button>
                        {index < versions.length - 1 ? (
                          <Button onClick={() => void showDiff(item.version, versions[index + 1]!.version)}>
                            {t("history.diff")}
                          </Button>
                        ) : null}
                      </li>
                    ))}
                  </ul>
                  {diff ? <pre className="mt-2 overflow-auto rounded bg-ink-900 p-2 font-mono text-[11px]">{diff}</pre> : null}
                </section>
              ) : null}
            </div>
          ) : (
            <p className="text-[12px] text-ink-200">{t("prompts.emptyHint")}</p>
          )}
        </Card>
      </div>

      <Card title={t("operations.title")}>
        {activations.length > 0 ? (
          <ul className="mb-2 space-y-1 text-[12px]">
            {activations.map((item) => (
              <li key={item.id} className="flex flex-wrap gap-2">
                <span>{item.promptTitle || item.promptId}</span>
                <span className="text-ink-200">{t(`scope.${item.scope}`)}</span>
                {item.projectDir ? <span className="font-mono text-[11px]">{shortPath(item.projectDir)}</span> : null}
                <span className="text-ink-200">{item.active ? t("status.active") : t("status.inactive")}</span>
              </li>
            ))}
          </ul>
        ) : null}
        {operations.length === 0 ? (
          <p className="text-[12px] text-ink-200">{t("operations.empty")}</p>
        ) : (
          <ul className="space-y-1 text-[12px]">
            {operations.map((item) => (
              <li key={item.id} className="flex flex-wrap items-center gap-2">
                <span className="font-mono text-[11px]">{item.kind}</span>
                <span>{item.status}</span>
                {item.error ? <span className="text-rose-200">{item.error}</span> : null}
                {item.recoverAvailable ? (
                  <Button onClick={() => void recover()}>{t("operations.restoreEntry")}</Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </Card>

      <ConfirmDialog
        open={Boolean(plan)}
        title={plan?.kind === "deactivate" ? t("plan.titleDeactivate") : t("plan.titleActivate")}
        confirmLabel={plan?.kind === "deactivate" ? t("plan.confirmDeactivate") : t("plan.confirmActivate")}
        cancelLabel={t("common.cancel")}
        confirmDisabled={
          !plan ||
          !canConfirmPlan(plan.result.envelope) ||
          isRecoveryState(plan.result.envelope) ||
          busy
        }
        confirmTestId="plan-confirm"
        onClose={() => {
          setPlan(null);
          setPlanError(null);
        }}
        onConfirm={() => void confirmPlan()}
      >
        {plan ? <PlanPreview envelope={plan.result.envelope} /> : null}
        {planError ? (
          <div className="rounded border border-rose-800/50 bg-rose-950/40 px-2 py-1.5 text-rose-100">
            <p>{t("plan.failed")}</p>
            <p>{planError}</p>
            <Button className="mt-2" onClick={() => void recover()}>
              {t("operations.restoreEntry")}
            </Button>
          </div>
        ) : null}
      </ConfirmDialog>
    </div>
  );
}
